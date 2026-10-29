//! Donations, the Finanzen screen's picture button 2222 (dialog 63,
//! `CSpenden`; FINANZUEBERSICHT.md §4.3 in the RE repo): an amount on a
//! slider, with the bank's money and what may still be spent beside it,
//! given to good causes or to another human's club. OK gives it
//! ([`a3_core::finances::donate`]).

use std::collections::HashMap;

use a3_core::finances::{
    self, Balance, DONATION_MOST, DONATION_OFFER, DONATION_STEPS, DONATION_UNIT,
};
use a3_core::season::Donation;
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::finances::money64;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

/// The dialog, which the Finanzen screen opens.
pub(crate) const DONATIONS: u16 = 63;

/// The amount, the bank's money, what may still be spent, the slider and
/// the recipients: good causes first, then the other humans' clubs.
const AMOUNT: u16 = 1628;
const BANK: u16 = 1629;
const INVESTABLE: u16 = 1630;
const SLIDER: u16 = 1631;
const RECIPIENTS: u16 = 6022;
const OK: u16 = 1;
const CANCEL: u16 = 2;
/// What a recipient's row shows where there is no human (0x887d48).
const NOBODY: &str = "---";
const HUMAN_ROWS: usize = 3;

#[derive(Default)]
pub struct Donations {
    /// The amount, and the clubs of the other humans by row.
    amount: i64,
    humans: Vec<(u8, i16)>,
    /// The chosen recipient: none for good causes, else a human's row.
    recipient: Option<usize>,
    /// With nothing in the bank the slider is disabled.
    still: bool,
}

impl Screen for Donations {
    fn ids(&self) -> &[u16] {
        &[DONATIONS]
    }

    /// `FUN_0054148b`: what the bank holds, up to 100.000, offered at
    /// first, in hundreds; with nothing in the bank the slider stands at
    /// nought and still.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(team) = super::squad::own_team(cx) else { return };
        let balance = Balance::of(team, records.globals.fund_prices(), &records.agreed());
        let bank = balance.bank();
        set(view, dialog, BANK, money64(game, bank));
        set(view, dialog, INVESTABLE, money64(game, balance.investable()));
        let slider = control(dialog, SLIDER);
        if bank < 1 {
            if let Some(value) = slider.and_then(|c| view.value_mut(c)) {
                value.range = (0, 1);
                value.set(0);
            }
            self.still = true;
        } else {
            self.amount = DONATION_OFFER.min(bank / DONATION_UNIT * DONATION_UNIT);
            let most = finances::step_of(&DONATION_STEPS, bank.min(DONATION_MOST) / DONATION_UNIT);
            if let Some(value) = slider.and_then(|c| view.value_mut(c)) {
                value.range = (0, most);
                value.set(finances::step_of(&DONATION_STEPS, self.amount / DONATION_UNIT));
            }
        }
        self.show_amount(cx, view);
        // The other humans, by their managers' names (`FUN_0054193d`).
        let own = cx.state.season.as_ref().and_then(|s| records.human_slot(s.current));
        let slots = records.globals.slots();
        let others: Vec<(usize, &a3_core::records::Manager)> = records
            .managers
            .iter()
            .enumerate()
            .filter(|&(slot, _)| slots.get(slot).copied().unwrap_or(0) > 0 && Some(slot) != own)
            .collect();
        self.humans = others
            .iter()
            .filter_map(|(_, m)| {
                let t = records.teams.get(usize::from(m.team()))?;
                Some((t.country(), i16::from(t.number())))
            })
            .collect();
        if let Some(rows) = control(dialog, RECIPIENTS).and_then(|c| view.options_mut(c)) {
            for row in 0..HUMAN_ROWS {
                let name = others.get(row).map(|(_, m)| format!("{} {}", m.first_name(), m.last_name()));
                if let Some(label) = rows.labels.get_mut(row + 1) {
                    *label = name.clone().unwrap_or_else(|| NOBODY.to_owned());
                }
                if let Some(dimmed) = rows.dimmed.get_mut(row + 1) {
                    *dimmed = name.is_none();
                }
            }
            rows.chosen = 0;
        }
        mark_inert(view, dialog, |target| match target {
            Target::Button(c) => matches!(dialog.controls[c].id, Some(OK | CANCEL)),
            Target::Option { control: c, .. } => dialog.controls[c].id == Some(RECIPIENTS),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            // The amount at the slider's step, in hundreds (`FUN_005418d5`).
            Target::SliderStep { control: c, .. }
            | Target::SliderAt { control: c, .. }
            | Target::SliderTrack(c)
                if id(c) == SLIDER && !self.still =>
            {
                let position = view.value(c).map_or(0, |v| v.value);
                self.amount = finances::amount_at(&DONATION_STEPS, position) * DONATION_UNIT;
                self.show_amount(cx, view);
            }
            // A recipient: good causes, or a human's club
            // (`OnClicked_IDC_SPENDEN`).
            Target::Option { control: c, item } if id(c) == RECIPIENTS => {
                self.recipient = item.checked_sub(1);
            }
            Target::Button(c) if id(c) == OK => {
                self.give(cx);
                return Outcome::Close(Reply::Ok);
            }
            Target::Button(c) if id(c) == CANCEL => return Outcome::Close(Reply::Cancel),
            _ => {}
        }
        Outcome::Stay
    }
}

impl Donations {
    fn show_amount(&self, cx: &Cx, view: &mut View) {
        set(view, cx.dialog, AMOUNT, super::playerinfo::money(cx.game, self.amount));
    }

    /// OK (`FUN_005419e5`): the donation's effects on the club and its
    /// manager; one to a human's club is kept for the week's end.
    fn give(&self, cx: &mut Cx) {
        let Some(team) = super::squad::own_team(cx).map(|t| (t.country(), t.number())) else {
            return;
        };
        let state = &mut *cx.state;
        let current = state.season.as_ref().map(|s| s.current);
        let Some(records) = state.records.as_mut() else { return };
        let Some(slot) = current.and_then(|c| records.human_slot(c)) else { return };
        let Some(index) = records.teams.iter().position(|t| (t.country(), t.number()) == team) else {
            return;
        };
        let mut manager = records.managers[slot].clone();
        finances::donate(&mut records.teams[index], &mut manager, self.amount);
        records.managers[slot] = manager;
        if self.amount != 0
            && let Some(club) = self.recipient.and_then(|r| self.humans.get(r).copied())
            && let (Some(season), Some(current)) = (state.season.as_mut(), current)
        {
            season.donations.push(Donation { manager: current, club, amount: self.amount as i32 });
        }
    }
}

/// Whether the manager has given to another human this week: then the
/// Finanzen screen refuses (`FUN_006eecd7`, text 4602).
pub(crate) fn given(cx: &Cx) -> bool {
    cx.state
        .season
        .as_ref()
        .is_some_and(|s| s.donations.iter().any(|d| d.manager == s.current && d.amount > 0))
}

/// Puts the dialog's caption in its frame.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&DONATIONS) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}
