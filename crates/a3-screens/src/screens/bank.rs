//! The bank behind the balance sheet: the deposits (dialog 16,
//! `CAnlagen`; BILANZ.md §4.2 in the RE repo) — an amount set by the
//! slider, put with one of five partners, the deposits listed with what
//! they cost and are worth, and dissolved again.

use std::collections::HashMap;

use a3_core::finances::{self, Balance as Figures, LEAST_DEPOSIT, STEP_UNIT};
use a3_files::rch::Dialog;

use super::finances::{money64, own_index};
use super::{Args, Cx, Outcome, Reply, Screen, control, set};
use crate::pointer::Target;
use crate::view::View;

pub use a3_core::cycle::dialogs::INVESTMENTS;

/// The amount, the bank's money, what may still be invested, what all
/// deposits cost.
const AMOUNT: u16 = 1267;
const BANK: u16 = 1268;
const INVESTABLE: u16 = 1269;
const TOTAL: u16 = 1275;
const SLIDER: u16 = 1278;
const INVEST: u16 = 1276;
const DISSOLVE: u16 = 1277;
const LIST: u16 = 1266;
const PARTNERS: u16 = 6050;
const OK: u16 = 1;
const CAPTION_ID: u16 = 9934;
/// The partners' kinds of deposit, by partner (`DAT_008791c0`).
const KINDS: [u16; 5] = [2804, 2805, 2806, 2807, 2808];
/// A club the bank trusts less may only put call money; the partners it
/// may not use.
const ONLY_CALL_MONEY: u8 = 0x80;
const RISKY_PARTNERS: [usize; 3] = [2, 3, 4];
/// The amount a deposit starts at, in the slider's steps.
const FIRST_STEP: i64 = LEAST_DEPOSIT / STEP_UNIT;

/// The dialogs' titles in their captions.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    if let Some(dialog) = dialogs.get_mut(&INVESTMENTS) {
        let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
        dialog.controls.insert(0, caption);
    }
}

/// The deposits.
#[derive(Default)]
pub struct Investments {
    /// The amount, the partner chosen, the slot the next deposit takes.
    amount: i64,
    partner: usize,
    slot: Option<usize>,
}

impl Screen for Investments {
    fn ids(&self) -> &[u16] {
        &[INVESTMENTS]
    }

    /// The call money's rate in its partner's name; the risky partners off
    /// for a club the bank trusts less (`FUN_00443c25`).
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        self.partner = 0;
        let Some((records, index)) = own_index(cx.game, cx) else { return };
        let rate = records.globals.deposit_rate();
        let rate = format!("{}.{:02}", rate / 100, rate % 100);
        let trusted = records.teams[index].standing() & ONLY_CALL_MONEY == 0;
        if let Some(options) = control(dialog, PARTNERS).and_then(|c| view.options_mut(c)) {
            if let Some(first) = options.labels.first_mut() {
                *first = first.replacen("%s", &rate, 1).replace("%%", "%");
            }
            if !trusted {
                options.off.resize(options.labels.len(), false);
                RISKY_PARTNERS.iter().for_each(|&p| options.off[p] = true);
            }
        }
        self.refresh(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if id(c) == Some(OK) => return Outcome::Close(Reply::Ok),
            Target::Button(c) if id(c) == Some(INVEST) => self.invest(cx, view),
            Target::Button(c) if id(c) == Some(DISSOLVE) => self.dissolve(cx, view),
            Target::Option { control: c, item } if id(c) == Some(PARTNERS) => self.partner = item,
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. }
                if id(c) == Some(SLIDER) =>
            {
                let position = view.value(c).map_or(0, |v| v.value);
                self.amount = finances::step_amount(position) * STEP_UNIT;
                set(view, dialog, AMOUNT, money64(cx.game, self.amount));
            }
            _ => {}
        }
        Outcome::Stay
    }
}

impl Investments {
    /// The list, the figures, the slider and the buttons as they now
    /// stand (`FUN_00443d73`, `FUN_00444007`).
    fn refresh(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some((records, index)) = own_index(game, cx) else { return };
        let team = &records.teams[index];
        let prices = records.globals.fund_prices();
        let deposits = finances::deposits_of(team, prices);
        if let Some(list) = control(dialog, LIST).and_then(|c| view.list_mut(c)) {
            list.rows = deposits
                .iter()
                .map(|d| {
                    let kind = game.text(KINDS[usize::from(d.partner).min(KINDS.len() - 1)]);
                    let (cost, value) = (money64(game, d.cost), money64(game, d.value));
                    format!("{}\t{kind}\t{cost}\t{value}", d.slot + 1)
                })
                .collect();
            list.keys = deposits.iter().map(|d| d.slot).collect();
            list.selected = None;
            list.scroll = 0;
        }
        let total = deposits.iter().fold(0i64, |s, d| s.wrapping_add(d.cost));
        set(view, dialog, TOTAL, money64(game, total));
        if let Some(button) = control(dialog, DISSOLVE) {
            view.set_disabled(button, deposits.is_empty());
        }
        let figures = Figures::of(team, prices, &records.agreed());
        let available = finances::available(team, &figures);
        let slider = control(dialog, SLIDER).and_then(|c| view.value_mut(c));
        if available < LEAST_DEPOSIT {
            self.amount = 0;
            self.slot = None;
            if let Some(slider) = slider {
                slider.range = (0, 1);
                slider.value = 0;
            }
        } else {
            self.amount = LEAST_DEPOSIT;
            self.slot = finances::free_deposit(team);
            if let Some(slider) = slider {
                slider.range = (
                    finances::step_position(FIRST_STEP),
                    finances::step_position(available / STEP_UNIT),
                );
                slider.value = finances::step_position(FIRST_STEP);
            }
        }
        set(view, dialog, AMOUNT, money64(game, self.amount));
        set(view, dialog, BANK, money64(game, figures.bank()));
        set(view, dialog, INVESTABLE, money64(game, figures.investable()));
        if let Some(button) = control(dialog, INVEST) {
            view.set_disabled(button, self.slot.is_none());
        }
        if let Some(c) = control(dialog, PARTNERS) {
            view.set_chosen(c, self.partner);
        }
    }

    /// The amount put with the partner in the free slot
    /// (`OnClicked_IDC_ANLAGEN_ANLEGEN_00444359`).
    fn invest(&mut self, cx: &mut Cx, view: &mut View) {
        let Some(slot) = self.slot else { return };
        let Some((_, index)) = own_index(cx.game, cx) else { return };
        let Some(records) = cx.state.records.as_mut() else { return };
        let prices = records.globals.fund_prices();
        let [day, month, _] = records.globals.date();
        let day = finances::bank_day(day, month);
        if let Some(team) = records.teams.get_mut(index) {
            finances::invest(team, slot, self.amount, self.partner as u8, prices, day);
        }
        self.refresh(cx, view);
    }

    /// The deposit of the row chosen dissolved
    /// (`OnClicked_IDC_ANLAGEN_AUFLOESEN_0044440f`).
    fn dissolve(&mut self, cx: &mut Cx, view: &mut View) {
        let chosen = control(cx.dialog, LIST)
            .and_then(|c| view.list(c))
            .and_then(|l| Some(*l.keys.get(l.selected?)?));
        let Some(slot) = chosen else { return };
        let Some((_, index)) = own_index(cx.game, cx) else { return };
        let Some(records) = cx.state.records.as_mut() else { return };
        let prices = records.globals.fund_prices();
        if let Some(team) = records.teams.get_mut(index) {
            let deposit = finances::deposits_of(team, prices).into_iter().find(|d| d.slot == slot);
            if let Some(deposit) = deposit {
                finances::dissolve(team, &deposit);
            }
        }
        self.refresh(cx, view);
    }
}
