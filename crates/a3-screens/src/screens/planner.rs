//! The week planner to book an appointment in (dialog 412,
//! `CSummeTerminplTerminAuswahl`): the days of this week, a row each with
//! what is booked for it. From the line-up's menu it books a talk with a
//! player (`Einzelgespräch`, kind 0x19), from the medical department a
//! sick visit (`Krankenbesuch`, 0x0b), on a free day from today on
//! (`FUN_0076640b`); a day gone or taken does not answer.

use a3_core::records::{Appointment, Records};

use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel, set};
use crate::pointer::Target;
use crate::view::View;

pub const PLANNER: u16 = 412;
const WEEK_TEXT: u16 = 3811;
const DAYS: u16 = 5034;
const PROMPT: u16 = 5037;
/// The notepad in the corner of the planner's picture.
const NOTEPAD: u16 = 5036;
/// The hidden OK; a day chosen closes it.
const OK: u16 = 1;
const WEEK: usize = 7;

/// A talk with a player, as the planner books it.
pub const TALK: u8 = 0x19;
/// A single day's appointment.
const ONE_DAY: u8 = 1;

/// Texts of the game.
const CHOOSE: u16 = 3876;
const DAY_MONTH: u16 = 1778;
/// What each kind of appointment is called, and whether it names a player
/// (`FUN_0058e4c9`).
const KINDS: [(u8, u16, bool); 16] = [
    (0x01, 2231, false),
    (0x0b, 2221, true),
    (0x0c, 2224, true),
    (0x0d, 2220, true),
    (0x0e, 2220, true),
    (0x0f, 1676, true),
    (0x10, 2220, true),
    (0x11, 2220, true),
    (0x12, 2229, false),
    (0x13, 2228, false),
    (0x14, 2220, false),
    (0x15, 2222, false),
    (0x16, 1730, false),
    (0x17, 2238, false),
    (0x18, 3661, false),
    (TALK, 4230, true),
];
/// The kinds 2 to 10 the original shows as missing.
const MISSING: &str = " Fehlt !!!! ";

/// The planner, booking an appointment with a player.
#[derive(Default)]
pub struct Planner {
    /// What is booked, with whom, and which days can be booked.
    kind: u8,
    player: u16,
    free: [bool; WEEK],
}

/// The human manager's slot and his week: its first day and today.
fn week(cx: &Cx) -> Option<(usize, usize, usize)> {
    let season = cx.state.season.as_ref()?;
    let records = cx.state.records.as_ref()?;
    let slot = records.human_slot(season.current)?;
    let start = usize::try_from(records.globals.week_start()).ok()?;
    Some((slot, start, usize::from(records.globals.day())))
}

/// A day of the planner as its row shows it: what kind of appointment, and
/// the player it is about.
fn label(cx: &Cx, records: &Records, a: Appointment) -> String {
    if a.kind == 0 {
        return "\n".to_owned();
    }
    if (0x02..=0x0a).contains(&a.kind) {
        return format!("{MISSING}\n");
    }
    let Some(&(_, text, named)) = KINDS.iter().find(|(kind, ..)| *kind == a.kind) else {
        return "\n".to_owned();
    };
    let name = if named { records.known_as(a.player).unwrap_or_default() } else { String::new() };
    format!("{}\n{name}", cx.game.text(text))
}

impl Screen for Planner {
    fn ids(&self) -> &[u16] {
        &[PLANNER]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        (self.kind, self.player) = match args {
            Args::Record(player) => (TALK, player),
            Args::Book(kind, player) => (kind, player),
            _ => return,
        };
        if let Some(ok) = control(cx.dialog, OK) {
            view.controls[ok].hidden = true;
        }
        set(view, cx.dialog, PROMPT, cx.game.text(CHOOSE).to_owned());
        // The notepad does nothing yet.
        let dialog = cx.dialog;
        let notepad = |t| matches!(t, Target::Button(b) if dialog.controls[b].id == Some(NOTEPAD));
        super::mark_inert(view, dialog, |t| !notepad(t));
        let (Some((slot, start, today)), Some(records)) = (week(cx), cx.state.records.as_ref())
        else {
            return;
        };
        let date = |day: usize| {
            records.globals.date_of(day as i16).map_or_else(String::new, |d| {
                use chrono::Datelike;
                cx.game
                    .text(DAY_MONTH)
                    .replace("%1", &d.day().to_string())
                    .replace("%2", &d.month().to_string())
            })
        };
        let template = control(cx.dialog, WEEK_TEXT)
            .map_or_else(String::new, |c| cx.dialog.controls[c].text.clone());
        let week_text =
            template.replacen("%s", &date(start), 1).replacen("%s", &date(start + 6), 1);
        set(view, cx.dialog, WEEK_TEXT, week_text);
        let planner = records.planners.get(slot);
        let days: Vec<Appointment> = (0..WEEK)
            .map(|i| planner.map_or_else(Appointment::default, |p| p.day(start + i)))
            .collect();
        let labels: Vec<String> = days.iter().map(|&a| label(cx, records, a)).collect();
        for (i, a) in days.iter().enumerate() {
            self.free[i] = a.kind == 0 && start + i >= today;
        }
        if let Some(rows) = control(cx.dialog, DAYS).and_then(|c| view.options_mut(c)) {
            rows.labels = labels;
            rows.actions = true;
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        if ok_or_cancel(cx.dialog, target).1 {
            return Outcome::Close(Reply::Cancel);
        }
        let Target::Option { control: c, item } = target else { return Outcome::Stay };
        if cx.dialog.controls[c].id != Some(DAYS) || !self.free.get(item).copied().unwrap_or(false)
        {
            return Outcome::Stay;
        }
        let Some((slot, start, _)) = week(cx) else { return Outcome::Stay };
        let Some(planner) = cx.state.records.as_mut().and_then(|r| r.planners.get_mut(slot)) else {
            return Outcome::Stay;
        };
        planner.book(
            start + item,
            Appointment { kind: self.kind, flag: ONE_DAY, player: self.player },
        );
        Outcome::Close(Reply::Ok)
    }
}
