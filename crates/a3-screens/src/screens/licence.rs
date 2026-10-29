//! The association's conditions, the Finanzen screen's picture button 1619
//! (dialog 64, `CFinanzpruefung`; FINANZUEBERSICHT.md §4.3 in the RE repo):
//! what the licence for the new season asks of the club, and how its
//! chances stand ([`a3_core::licence`]). The screen refuses with text 3108
//! where the chance is gone.

use std::collections::HashMap;

use a3_core::licence::{self, DEBT, NO_BUILDING, NO_CAMPS, SALARIES, SPECTATORS};
use a3_core::licence::{TRANSFER_BUDGET, TRANSFER_SURPLUS};
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::ControlState;
use crate::pointer::Target;
use crate::view::View;

/// The dialog, which the Finanzen screen opens.
pub(crate) const LICENCE: u16 = 64;
/// The text the screen refuses with, the chance gone.
pub(crate) const NO_LICENCE: u16 = 3108;

/// The conditions, the outlook below them, and OK.
const CONDITIONS: u16 = 1637;
const OUTLOOK: u16 = 1636;
const OK: u16 = 1638;
/// The outlook's ink on its bar (COLORREF 0xdcffff).
const OUTLOOK_INK: [u8; 3] = [0xff, 0xff, 0xdc];

/// The conditions' texts, each on a line of its own: the most debt, the
/// spectators, the salaries, a transfer surplus of a sum, a transfer
/// budget, a surplus of any sum, no building, no camps, none at all.
const DEBT_TEXT: u16 = 2811;
const SPECTATORS_TEXT: u16 = 2438;
const SALARIES_TEXT: u16 = 2439;
const SURPLUS_SUM_TEXT: u16 = 3297;
const BUDGET_TEXT: u16 = 2440;
const SURPLUS_TEXT: u16 = 2443;
const NO_BUILDING_TEXT: u16 = 2441;
const NO_CAMPS_TEXT: u16 = 2442;
const NONE_TEXT: u16 = 2444;
/// The outlook while the season runs: troubled or fine, and the breach,
/// slight, plain or grave (2816 to 2818 after the first, 2820 to 2822
/// after the second); past the 84th half week the verdict: down to the
/// amateurs, refused, granted.
const TROUBLED: u16 = 2815;
const FINE: u16 = 2819;
const BREACH_AFTER_TROUBLED: u16 = 2816;
const BREACH_AFTER_FINE: u16 = 2820;
const VERDICT_DOWN: u16 = 2812;
const VERDICT_REFUSED: u16 = 2813;
const VERDICT_GRANTED: u16 = 2814;
const VERDICT_FROM: u16 = 84;

#[derive(Default)]
pub struct Licence;

impl Screen for Licence {
    fn ids(&self) -> &[u16] {
        &[LICENCE]
    }

    /// `FUN_004a07de`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let name = cx.state.season.as_ref().and_then(|s| s.manager()).map_or_else(String::new, |m| {
            format!(" {} {}", m.first_name, m.last_name)
        });
        set(view, dialog, CAPTION_ID, format!("{}{name}", dialog.title));
        let (Some(records), Some(team)) = (cx.state.records.as_ref(), super::squad::own_team(cx))
        else {
            return;
        };
        let money = |v: i64| super::playerinfo::money(game, v);
        let with = |text: u16, fill: &str| game.text(text).replacen("%s", fill, 1).replacen("%d", fill, 1);
        let flags = licence::conditions(team);
        let mut lines = Vec::new();
        if flags & DEBT != 0 {
            lines.push(with(DEBT_TEXT, &money(licence::most_debt(team))));
        }
        let spectators = licence::most_spectators(team);
        if flags & SPECTATORS != 0 && spectators != 0 {
            lines.push(with(SPECTATORS_TEXT, &spectators.to_string()));
        }
        if flags & SALARIES != 0 {
            lines.push(with(SALARIES_TEXT, &money(licence::most_salaries(team))));
        }
        let sum = licence::transfer_sum(team);
        if flags & TRANSFER_SURPLUS != 0 {
            lines.push(with(SURPLUS_SUM_TEXT, &money(sum)));
        } else if flags & TRANSFER_BUDGET != 0 {
            let text = if sum == 0 { game.text(SURPLUS_TEXT).to_owned() } else { with(BUDGET_TEXT, &money(sum)) };
            lines.push(text);
        }
        if flags & NO_BUILDING != 0 {
            lines.push(game.text(NO_BUILDING_TEXT).to_owned());
        }
        if flags & NO_CAMPS != 0 {
            lines.push(game.text(NO_CAMPS_TEXT).to_owned());
        }
        let any = !lines.is_empty();
        if !any {
            lines.push(game.text(NONE_TEXT).to_owned());
        }
        set(view, dialog, CONDITIONS, lines.iter().map(|l| format!("{l}\n")).collect());
        // The outlook (+0x6a8): nothing without conditions; while the
        // season runs the chance and the breach, then the verdict.
        let outlook = if !any {
            String::new()
        } else if records.globals.half_weeks() < VERDICT_FROM {
            let breach = licence::breach(records, team);
            let size = (breach < 0).then(|| licence::breach_size(breach) as u16);
            let (head, after) = if licence::chance(records, team) < 100 {
                (TROUBLED, BREACH_AFTER_TROUBLED)
            } else {
                (FINE, BREACH_AFTER_FINE)
            };
            let tail = size.map_or("", |s| game.text(after + s));
            format!("{}{tail}", game.text(head))
        } else if !licence::refused(team) {
            game.text(VERDICT_GRANTED).to_owned()
        } else if licence::lower_league(team) {
            game.text(VERDICT_DOWN).to_owned()
        } else {
            game.text(VERDICT_REFUSED).to_owned()
        };
        set(view, dialog, OUTLOOK, outlook);
        if let Some(ControlState::Static(s)) = control(dialog, OUTLOOK).map(|c| &mut view.controls[c].state)
        {
            s.ink = Some(OUTLOOK_INK);
        }
        mark_inert(view, dialog, |target| match target {
            Target::Button(c) => dialog.controls[c].id == Some(OK),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(OK) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// Puts the dialog's caption in its frame.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&LICENCE) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}
