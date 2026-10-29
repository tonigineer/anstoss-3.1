//! The year calendar, the menu bar's seventh symbol (dialog 348,
//! `CJahreskalender`; JAHRESKALENDER.md in the RE repo): a month at a time,
//! a row a day with its weekday, its week, and what it holds — a training
//! camp, a tournament, a friendly, a match and its result, a party or an
//! event. A party or an event is booked by dragging its label onto a day,
//! and gone again by a click on its row; a click in the camp column moves
//! the camp's end. The camp, the tournaments and the friendly need screens
//! not built yet.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};

use a3_core::calendar;
use a3_core::cycle::dialogs::NOTICE;
use a3_core::records::{Planner, Records, Team};
use a3_core::season::Rules;
use a3_core::year::{self, CAMP, EVENT, FRIENDLY, LAST_BOOKING, MATCH, PARTY};
use a3_files::rch::Dialog;
use image::Rgba;

use super::hub::{self, YEAR_CALENDAR, YEAR_CALENDAR_SYMBOL};
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::controls::list::Bar;
use crate::pointer::{Grip, Target};
use crate::view::View;

/// Controls by their ids in the dialog file.
const DAYS: u16 = 4459;
const MONTHS: u16 = 4776;
const EVENT_LABEL: u16 = 5133;
const EVENT_SPIN: u16 = 5134;
const PARTY_LABEL: u16 = 5135;
const PARTY_SPIN: u16 = 5136;
/// The training camp's label, dragged as its button's picture.
const CAMP_LABEL: u16 = 5038;
const CAMP_PICTURE: u16 = 2613;
/// The OK, whose place the bar takes.
const OK: u16 = 1;
const CAPTION_ID: u16 = 9931;

/// What a dragged label looks like: the event's, the party's.
const EVENT_PICTURE: u16 = 2618;
const PARTY_PICTURE: u16 = 2617;
/// A camp day's red dot, and where it goes in its row.
const CAMP_DOT: u16 = 2811;
const CAMP_DOT_AT: f32 = 50.0;
/// The camp column, which moves the camp's end.
const CAMP_COLUMN: usize = 2;
/// The rows' height and where the list starts, for a drop.
const ROW: f32 = 12.0;

/// Texts of the game.
const WEEKDAYS: u16 = 1779;
const ARRIVAL: u16 = 4690;
const DEPARTURE: u16 = 4691;
const UNKNOWN: u16 = 1996;
const TOURNAMENTS: [u16; 3] = [2235, 2237, 4091];
const FRIENDLY_TEXT: u16 = 2234;
const TOO_EARLY: u16 = 4359;
const TOO_LATE: u16 = 4360;
const IN_CAMP: u16 = 4969;

/// The rows' colours: Sundays, days with something on, today's band.
const SUNDAY: [u8; 3] = [14, 160, 40];
const BUSY: [u8; 3] = [182, 10, 26];
const TODAY: Rgba<u8> = Rgba([200, 200, 200, 255]);
const TODAY_WIDTH: i32 = 422;
const WEEK_DAYS: i64 = 7;
/// The first month of a season.
const JULY: u32 = 7;
/// The events and parties offered only sometimes: the youth day to a club
/// that is no other's second team, the champions' party in the season's
/// last weeks, Christmas in December, the season's end in June.
const YOUTH_DAY: usize = 1;
const CHAMPIONS: usize = 5;
const CHAMPIONS_WEEKS: std::ops::RangeInclusive<u16> = 88..=94;
const CHRISTMAS: usize = 3;
const SEASON_END: usize = 4;

/// The bar takes the OK's place; the caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&YEAR_CALENDAR) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
    hub::add_bar(dialog, rules);
}

#[derive(Default)]
pub struct YearCalendar {
    /// The month shown, 1 to 12, and the day of its first row.
    month: u32,
    first: i64,
    /// The events and parties the labels offer, by their values, and the
    /// ones chosen.
    events: Vec<usize>,
    parties: Vec<usize>,
    event: usize,
    party: usize,
    /// Whether the whistle asks.
    whistle: bool,
    /// The day a camp was dropped on, while the world map is open.
    camp_day: Option<usize>,
}

impl Screen for YearCalendar {
    fn ids(&self) -> &[u16] {
        &[YEAR_CALENDAR]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        hub::open_bar(cx, view, YEAR_CALENDAR_SYMBOL);
        let Some(season) = cx.state.season.as_ref() else { return };
        set(view, dialog, CAPTION_ID, hub::manager_title_of(game, season, &dialog.title));
        self.month = season.date().month();
        if let Some(c) = control(dialog, MONTHS) {
            view.set_chosen(c, self.month as usize - 1);
        }
        if let Some(list) = control(dialog, DAYS).and_then(|c| view.list_mut(c)) {
            list.by_column = true;
            list.keyed = vec![CAMP_DOT];
        }
        for (label, picture) in
            [(EVENT_LABEL, EVENT_PICTURE), (PARTY_LABEL, PARTY_PICTURE), (CAMP_LABEL, CAMP_PICTURE)]
        {
            let Some(c) = control(dialog, label) else { continue };
            view.set_clickable(c, true);
            if let Some(crate::controls::ControlState::Static(s)) =
                view.controls.get_mut(c).map(|v| &mut v.state)
            {
                s.quiet = true;
            }
            view.drag_pictures.push((c, picture));
        }
        self.fill(cx, view);
        self.offer(cx, view);
        let works = |target: Target| match target {
            Target::Option { control: c, .. } if dialog.controls[c].id == Some(MONTHS) => true,
            _ => hub::on_bar(dialog, target).is_some_and(hub::bar_works),
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        if let Some(item) = hub::on_bar(dialog, target) {
            let outcome = hub::bar_click(cx, view, item, YEAR_CALENDAR_SYMBOL);
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        match target {
            Target::Option { control: c, item } if id(c) == MONTHS => {
                self.month = item as u32 + 1;
                self.fill(cx, view);
                self.offer(cx, view);
            }
            Target::Spin { control: c, .. } if [EVENT_SPIN, PARTY_SPIN].contains(&id(c)) => {
                let chosen = view.value(c).map_or(0, |v| v.value.max(0) as usize);
                if id(c) == EVENT_SPIN {
                    self.event = chosen;
                } else {
                    self.party = chosen;
                }
                self.label(cx, view);
            }
            // A day's party or event goes; a click in the camp column moves
            // the camp's end there (`0x4d05c0`, `OnUserC000_004d072f`).
            Target::Cell { control: c, row, column } if id(c) == DAYS => {
                let scroll = view.list(c).map_or(0, |l| l.scroll);
                let Some(day) = self.day_of(scroll + row) else { return Outcome::Stay };
                let tables = &cx.game.rules.year;
                let kind = calendar_of(cx).map_or(0, |c| year::day(c, day).kind());
                let today = today(cx);
                let changed = if tables.deletable(kind) {
                    change_calendar(cx, |c| year::clear(c, day));
                    true
                } else if column == CAMP_COLUMN {
                    let mut moved = false;
                    change_calendar(cx, |c| moved = year::move_camp_end(c, day, today));
                    moved
                } else {
                    false
                };
                if changed {
                    self.fill(cx, view);
                }
            }
            Target::Drop { from: Grip::Control(c), moved, .. }
                if [EVENT_LABEL, PARTY_LABEL, CAMP_LABEL].contains(&id(c)) =>
            {
                let at = dialog.controls[c].rect.min
                    + bevy_math::Vec2::new(f32::from(moved.0), f32::from(moved.1));
                return self.drop(cx, view, id(c), at.max(bevy_math::Vec2::ZERO));
            }
            _ => {}
        }
        Outcome::Stay
    }

    /// The whistle's question: a yes ends the turn. The world map's place
    /// books the camp from its day; either way the month is shown anew.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            return Outcome::Close(Reply::Ok);
        }
        if let Some(day) = self.camp_day.take() {
            if let Reply::Choice(place) = reply {
                change_calendar(cx, |c| year::book_camp(c, day, place as i16));
            }
            self.fill(cx, view);
        }
        Outcome::Stay
    }
}

impl YearCalendar {
    /// The month's days, a row each (`FUN_004cf81b`): the weekday, the
    /// day of the month, the week on its first row and on Sundays, what
    /// the day holds; Sundays green, days with something on red, today on
    /// a grey band, camp days with a red dot.
    fn fill(&mut self, cx: &Cx, view: &mut View) {
        let game = cx.game;
        let Some(season) = cx.state.season.as_ref() else { return };
        let start = calendar::date(season.year, 0);
        let year = start.year() + i32::from(self.month < JULY);
        let first_of_month = NaiveDate::from_ymd_opt(year, self.month, 1).unwrap_or(start);
        self.first = (first_of_month - start).num_days();
        let days = days_in(year, self.month);
        let calendar = calendar_of(cx);
        let records = cx.state.records.as_ref();
        let own = own_team(cx);
        let tables = &game.rules.year;
        let today = today(cx) as i64;
        let (mut rows, mut inks, mut icons, mut bars) = (vec![], vec![], vec![], vec![]);
        for i in 0..days {
            let day = self.first + i;
            if day < 0 {
                rows.push(String::new());
                inks.push(None);
                icons.push(Vec::new());
                bars.push(None);
                continue;
            }
            let d = day as usize;
            let weekday = game.text(WEEKDAYS + (day % WEEK_DAYS) as u16);
            let week = if i == (-self.first).max(0) || day % WEEK_DAYS == 0 {
                (day / WEEK_DAYS + 1).to_string()
            } else {
                String::new()
            };
            let entry = calendar.map(|c| year::day(c, d)).filter(|e| tables.shown(e.kind()));
            let sunday = day % WEEK_DAYS == 0;
            if let Some(entry) = entry {
                let (art, termin) = texts(cx, records, own, &entry);
                rows.push(format!("{weekday}\t{}\t\t{week}\t{art}\t{termin}", i + 1));
                inks.push(Some(if sunday { SUNDAY } else { BUSY }));
            } else {
                rows.push(format!("{weekday}\t{}\t\t{week}", i + 1));
                inks.push(sunday.then_some(SUNDAY));
            }
            let camp = calendar.is_some_and(|c| year::in_camp(c, d));
            icons.push(if camp { vec![(CAMP_DOT_AT, CAMP_DOT)] } else { Vec::new() });
            bars.push((day == today).then_some(Bar {
                left: 0,
                width: TODAY_WIDTH,
                value: 100,
                fill: TODAY,
            }));
        }
        let Some(list) = control(cx.dialog, DAYS).and_then(|c| view.list_mut(c)) else { return };
        list.rows = rows;
        list.inks = inks;
        list.row_icons = icons;
        list.bars = bars;
        list.selected = None;
        list.scroll = 0;
    }

    /// The events and parties the labels offer this month, the choices
    /// kept where they still are (`FUN_004cfe6b`).
    fn offer(&mut self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        let party = self.parties.get(self.party).copied();
        let mut parties = vec![0, 1, 2];
        if self.month == 12 {
            parties.push(CHRISTMAS);
        }
        if self.month == 6 {
            parties.push(SEASON_END);
        }
        parties.push(5);
        self.party = party.and_then(|p| parties.iter().position(|&q| q == p)).unwrap_or(0);
        self.parties = parties;
        let second_team = own_team(cx).is_some_and(|t| t.linked()[0] >= 0);
        let weeks = cx.state.records.as_ref().map_or(0, |r| r.globals.half_weeks());
        self.events = (0..7)
            .filter(|&e| e != YOUTH_DAY || !second_team)
            .filter(|&e| e != CHAMPIONS || CHAMPIONS_WEEKS.contains(&weeks))
            .collect();
        self.event = self.event.min(self.events.len() - 1);
        for (spin, chosen, count) in [
            (EVENT_SPIN, self.event, self.events.len()),
            (PARTY_SPIN, self.party, self.parties.len()),
        ] {
            if let Some(value) = control(dialog, spin).and_then(|c| view.value_mut(c)) {
                value.range = (0, count as i32 - 1);
                value.value = chosen as i32;
            }
        }
        self.label(cx, view);
    }

    /// The labels name the event and the party chosen.
    fn label(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let tables = &game.rules.year;
        let event = self.events.get(self.event).and_then(|&e| tables.event_texts.get(e));
        let party = self.parties.get(self.party).and_then(|&p| tables.party_texts.get(p));
        set(view, dialog, EVENT_LABEL, game.text(event.copied().unwrap_or(0)).to_owned());
        set(view, dialog, PARTY_LABEL, game.text(party.copied().unwrap_or(0)).to_owned());
    }

    /// The day of a row, if it has one.
    fn day_of(&self, row: usize) -> Option<usize> {
        usize::try_from(self.first + row as i64).ok()
    }

    /// A label let go of with its picture's corner at `at`: on a free day
    /// from today on the party or event is booked (`FUN_004d0bbf`).
    fn drop(&mut self, cx: &mut Cx, view: &mut View, label: u16, at: bevy_math::Vec2) -> Outcome {
        let Some(list) = control(cx.dialog, DAYS).map(|c| cx.dialog.controls[c].rect) else {
            return Outcome::Stay;
        };
        if !list.contains(at) {
            return Outcome::Stay;
        }
        let row = ((at.y - list.min.y) / ROW) as usize;
        let rows = view.list(control(cx.dialog, DAYS).unwrap_or(0)).map_or(0, |l| l.rows.len());
        let Some(day) = self.day_of(row).filter(|_| row < rows) else { return Outcome::Stay };
        let today = today(cx);
        if day < today {
            return Outcome::Popup(Request::with(NOTICE, Args::TextForDate(TOO_EARLY, today)));
        }
        if day > LAST_BOOKING {
            return Outcome::Popup(Request::with(NOTICE, Args::Text(TOO_LATE)));
        }
        if label == CAMP_LABEL {
            let tables = &cx.game.rules.year;
            let Some(calendar) = calendar_of(cx) else { return Outcome::Stay };
            if !tables.free(calendar, day, CAMP) {
                return Outcome::Stay;
            }
            if year::in_camp(calendar, day) && !tables.allowed_in_camp(CAMP) {
                return Outcome::Popup(Request::with(NOTICE, Args::Text(IN_CAMP)));
            }
            self.camp_day = Some(day);
            return Outcome::Popup(Request::with(super::camp::WORLD_MAP, Args::CampDay(day)));
        }
        let (kind, value) = if label == EVENT_LABEL {
            (EVENT, self.events.get(self.event).copied().unwrap_or(0))
        } else {
            (PARTY, self.parties.get(self.party).copied().unwrap_or(0))
        };
        let tables = &cx.game.rules.year;
        let Some(calendar) = calendar_of(cx) else { return Outcome::Stay };
        if !tables.free(calendar, day, kind) {
            return Outcome::Stay;
        }
        if year::in_camp(calendar, day) && !tables.allowed_in_camp(kind) {
            return Outcome::Popup(Request::with(NOTICE, Args::Text(IN_CAMP)));
        }
        change_calendar(cx, |c| year::book(c, day, kind, value as i16));
        self.fill(cx, view);
        Outcome::Stay
    }
}

/// What a day's entry says in the two text columns (`FUN_004cece7`).
fn texts(
    cx: &Cx,
    records: Option<&Records>,
    own: Option<&Team>,
    entry: &year::Day,
) -> (String, String) {
    let game = cx.game;
    let tables = &game.rules.year;
    let text = |id: u16| game.text(id).to_owned();
    let club = |(country, number): (u8, i16)| {
        if country == 0 { String::new() } else { game.club_name(records, country, number) }
    };
    match entry.kind() {
        CAMP if entry.camp_start() => {
            let place = usize::try_from(entry.place() - 1).ok();
            let name = place.and_then(|p| tables.place_texts.get(p)).copied().unwrap_or(UNKNOWN);
            (text(ARRIVAL), text(name))
        }
        CAMP => (text(DEPARTURE), String::new()),
        kind @ 2..=4 => {
            let name = TOURNAMENTS[usize::from(kind - 2)];
            (String::new(), format!("{} ({})", text(name), club(entry.club())))
        }
        FRIENDLY => (String::new(), format!("{} ({})", text(FRIENDLY_TEXT), club(entry.club()))),
        MATCH => {
            let country = own.map_or(0, |t| t.country());
            let start_year = records.map_or(0, |r| i32::from(r.globals.start_year()));
            let art = competition(cx, entry.match_code(), country, start_year);
            let [home, away] = entry.teams();
            let mut termin = String::new();
            if home.0 != 0 {
                termin = format!("{} - {}", club(home), club(away));
                if let Some((h, a)) = entry.result() {
                    termin.push_str(&format!(" {h}:{a}"));
                }
            }
            (art, termin)
        }
        PARTY => {
            let name = usize::try_from(entry.value()).ok().and_then(|v| tables.party_texts.get(v));
            (String::new(), text(name.copied().unwrap_or(0)))
        }
        EVENT => {
            let name = usize::try_from(entry.value()).ok().and_then(|v| tables.event_texts.get(v));
            (String::new(), text(name.copied().unwrap_or(0)))
        }
        _ => (String::new(), String::new()),
    }
}

/// A match's competition, by its code (`FUN_004cea5a`): the league with
/// its matchday, the cup, the European cups …
fn competition(cx: &Cx, code: u32, country: u8, start_year: i32) -> String {
    const GERMANY: u8 = 14;
    const MATCHDAY: u32 = 0x3f;
    let game = cx.game;
    let world_cup_year = matches!(start_year % 4, 0 | 1);
    let id = match code & 0x7f00_0000 {
        0 => return game.text(3454).replacen("%d", &(code & MATCHDAY).to_string(), 1),
        0x4000_0000 => 3455,
        0x0100_0000 => 3458,
        0x5000_0000 => 3456,
        0x3000_0000 if country == GERMANY => 3458,
        0x3000_0000 => 3457,
        0x1000_0000 => 3459,
        0x7000_0000 => 3460,
        0x6000_0000 => 3461,
        0x2000_0000 => 3462,
        0x0400_0000 => 4692,
        0x0800_0000 if world_cup_year => 4693,
        0x0800_0000 => 5033,
        0x0200_0000 if world_cup_year => 4694,
        0x0200_0000 => 5034,
        _ => return "?".to_owned(),
    };
    game.text(id).to_owned()
}

/// Days in a month.
fn days_in(year: i32, month: u32) -> i64 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    let first = NaiveDate::from_ymd_opt(year, month, 1);
    next.zip(first).map_or(30, |(n, f)| (n - f).num_days())
}

/// Today, counted from the first season's start.
fn today(cx: &Cx) -> usize {
    cx.state.season.as_ref().map_or(0, |s| s.day)
}

/// The manager's club's team record.
fn own_team<'a>(cx: &'a Cx) -> Option<&'a Team> {
    let (slot, number) = cx.state.season.as_ref()?.manager()?.club?;
    let nation = cx.game.world.country(slot)?.nation;
    cx.state.records.as_ref()?.team(nation, u16::try_from(number).ok()?)
}

/// The club's year calendar, by its human slot.
fn calendar_of<'a>(cx: &'a Cx) -> Option<&'a Planner> {
    let slot = usize::from(own_team(cx)?.human()).checked_sub(1)?;
    cx.state.records.as_ref()?.calendars.get(slot)
}

/// Changes the club's year calendar at once.
fn change_calendar(cx: &mut Cx, change: impl FnOnce(&mut Planner)) {
    let Some(slot) = own_team(cx).and_then(|t| usize::from(t.human()).checked_sub(1)) else {
        return;
    };
    if let Some(calendar) = cx.state.records.as_mut().and_then(|r| r.calendars.get_mut(slot)) {
        change(calendar);
    }
}
