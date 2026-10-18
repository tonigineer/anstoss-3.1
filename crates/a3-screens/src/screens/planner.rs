//! The week planner to book an appointment in (dialog 412,
//! `CSummeTerminplTerminAuswahl`): the days of this week, a row each with
//! what is booked for it. From the line-up's menu it books a talk with a
//! player (`Einzelgespräch`, kind 0x19), from the medical department a
//! sick visit (`Krankenbesuch`, 0x0b), on a free day from today on
//! (`FUN_0076640b`); a day gone or taken does not answer.

use std::collections::HashMap;

use a3_core::records::{Appointment, Records};
use a3_core::season::Rules;
use a3_files::rch::Dialog;

use super::hub::{self, WEEK_PLANNER, WEEK_PLANNER_SYMBOL};
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
const CAPTION_ID: u16 = 9932;

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
/// A negotiation with another human's club, which names the club: its
/// country in the entry's player field, its number after.
const WITH_CLUB: u8 = 0x14;
const NEGOTIATION: u16 = 2220;
/// The kinds 2 to 10 the original shows as missing.
const MISSING: &str = " Fehlt !!!! ";

/// The week planner of the menu bar (dialog 413, `CTerminplanerNormal`;
/// TERMINPLANER.md in the RE repo): the bar takes the OK's place.
/// Its button for the next match's announcement and its notepad lead to
/// screens not built yet.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&WEEK_PLANNER) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
    hub::add_bar(dialog, rules);
}

/// The week planner of the menu bar: the week's days with what is booked
/// for each, which only shows; a click lights a day.
#[derive(Default)]
pub struct WeekPlanner {
    /// Whether the whistle asks.
    whistle: bool,
}

impl Screen for WeekPlanner {
    fn ids(&self) -> &[u16] {
        &[WEEK_PLANNER]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        hub::open_bar(cx, view, WEEK_PLANNER_SYMBOL);
        // On the week's last day the short-term training closes too; the
        // planner's own symbol without a next opponent (`FUN_0058de0f`).
        if hub::last_day(cx) {
            hub::switch_off(cx, view, hub::SHORT_TRAINING_SYMBOL);
        }
        if !next_opponent(cx) {
            hub::switch_off(cx, view, WEEK_PLANNER_SYMBOL);
        }
        if let Some(prompt) = control(dialog, PROMPT) {
            view.controls[prompt].hidden = true;
        }
        show_week(cx, view);
        if let Some(rows) = control(dialog, DAYS) {
            if let Some(options) = view.options_mut(rows) {
                options.actions = false;
            }
            view.set_chosen(rows, super::super::controls::options::NONE);
        }
        let works = |target: Target| {
            hub::on_bar(dialog, target).is_none_or(hub::bar_works)
                && !matches!(target, Target::Button(_))
        };
        super::mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if let Some(item) = hub::on_bar(cx.dialog, target) {
            let outcome = hub::bar_click(cx, view, item, WEEK_PLANNER_SYMBOL);
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        Outcome::Stay
    }

    /// The whistle's question: a yes ends the turn.
    fn answer(&mut self, _: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}

/// Whether the manager's club has a next match to play.
fn next_opponent(cx: &Cx) -> bool {
    let Some(season) = cx.state.season.as_ref() else { return false };
    let Some(key) = season.manager().and_then(|m| m.club) else { return false };
    if let Some(records) = cx.state.records.as_ref() {
        let nation = cx.game.world.country(key.0).map(|c| c.nation);
        let team = nation.zip(u16::try_from(key.1).ok()).and_then(|(n, c)| records.team(n, c));
        return team.is_some_and(|t| t.next_country() != 0);
    }
    season.next_match(&cx.game.world, &cx.game.calendar, key).is_some()
}

/// The week's line and its days, each with what is booked for it; the
/// days which are free from today on.
fn show_week(cx: &Cx, view: &mut View) -> [bool; WEEK] {
    let mut free = [false; WEEK];
    let (Some((slot, start, today)), Some(records)) = (week(cx), cx.state.records.as_ref()) else {
        return free;
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
    let week_text = template.replacen("%s", &date(start), 1).replacen("%s", &date(start + 6), 1);
    set(view, cx.dialog, WEEK_TEXT, week_text);
    let planner = records.planners.get(slot);
    let days: Vec<Appointment> = (0..WEEK)
        .map(|i| planner.map_or_else(Appointment::default, |p| p.day(start + i)))
        .collect();
    let labels: Vec<String> = days.iter().map(|&a| label(cx, records, a)).collect();
    for (i, a) in days.iter().enumerate() {
        free[i] = a.kind == 0 && start + i >= today;
    }
    if let Some(rows) = control(cx.dialog, DAYS).and_then(|c| view.options_mut(c)) {
        rows.labels = labels;
        rows.actions = true;
    }
    free
}

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
    if (0x02..=0x0a).contains(&a.kind) || a.kind > TALK {
        return format!("{MISSING}\n");
    }
    if a.kind == WITH_CLUB {
        let country = (a.player & 0xff) as u8;
        let name = cx.game.club_name(Some(records), country, a.club);
        return format!("{}\n{name}", cx.game.text(NEGOTIATION));
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
        self.free = show_week(cx, view);
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
            Appointment { kind: self.kind, flag: ONE_DAY, player: self.player, club: 0 },
        );
        Outcome::Close(Reply::Ok)
    }
}
