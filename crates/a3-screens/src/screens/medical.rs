//! The medical department, the game's `CMedizinischeAbteilung` (dialog
//! 151), which the line-up's "Spielsysteme" buttons open: the sick list —
//! the injured, longest out first, then those who play with a knock — with
//! each one's injury, since when, how many days still, against whom he is
//! fit again and whether he was visited; and what the doctor says of them.
//! A click on a player's status gives him an injection to play with, or
//! takes it back, where his injury allows; a click elsewhere on him books a
//! sick visit in the week planner, on a week's first day. OK operates on
//! who is due (MEDIZIN.md in the RE repo).

use std::collections::HashMap;

use a3_core::medical;
use a3_core::season::{ClubState, Season};
use a3_files::rch::Dialog;

use super::hub::manager_title;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, ok_or_cancel, set};
use super::{duties, planner, popups};
use crate::pointer::Target;
use crate::view::View;

pub const MEDICAL: u16 = 151;
const SICK_LIST: u16 = 2586;
const NOTES: u16 = 2587;
/// The doping department (dialog 394), not built yet.
const DOPING: u16 = 4929;
const CAPTION_ID: u16 = 9926;
const CAPTION: u16 = 2516;
/// The column a click on gives or takes the injection: "Status".
const STATUS_COLUMN: usize = 4;
/// The icons a row shows, the first at x 300, a second at 0: the injection,
/// an operation due.
const ICON_SLOTS: [f32; 2] = [300.0, 0.0];
const INJECTION_ICON: u16 = 1910;
const OPERATION_ICON: u16 = 1911;
/// A sick visit, as the planner books it.
const VISIT_KIND: u8 = 0x0b;
/// The players' ids above it are the managers who play.
const LAST_REAL_ID: u16 = 17_000;
/// What a knock's days show: the address of "?" the original passes to a
/// `%d` (0x8854d2).
const KNOCK_DAYS: &str = "8934610";
const WEEK: usize = 7;
/// The day from which an injury lasts into next season.
const SEASON_END: usize = 360;

/// Texts of the game.
const NOBODY: u16 = 2116;
const YES: u16 = 1028;
const NO: u16 = 1029;
const UNTIL_SEASON_END: u16 = 1705;
const NEXT_SEASON: u16 = 1706;
const TALK_TO_YOURSELF: u16 = 1717;
const TALK_TO_HIM: u16 = 1718;
const NEXT_WEEK: u16 = 3132;

/// Puts the dialog's title in its frame.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&MEDICAL) else { return };
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
}

#[derive(Default)]
pub struct Medical {
    /// The sick list's columns as the dialog file sets them, which an empty
    /// list gives up for one.
    tabs: Vec<f32>,
}

impl Screen for Medical {
    fn ids(&self) -> &[u16] {
        &[MEDICAL]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        if let Some(season) = &cx.state.season {
            set(view, dialog, CAPTION_ID, manager_title(cx.game, season, CAPTION));
        }
        if let Some(list) = control(dialog, SICK_LIST).and_then(|c| view.list_mut(c)) {
            list.by_column = true;
            self.tabs.clone_from(&list.tabs);
        }
        mark_inert(view, dialog, |_| true);
        self.fill(cx, view);
        notes(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if ok_or_cancel(cx.dialog, target).0 {
            operate(cx);
            return Outcome::Close(Reply::Ok);
        }
        // The doping department (`0x518c02`); nothing refreshed after it.
        if matches!(target, Target::Button(b) if cx.dialog.controls[b].id == Some(DOPING)) {
            return Outcome::Popup(Request::new(super::doping::DOPING));
        }
        let (c, row, column) = match target {
            Target::Cell { control, row, column } => (control, row, Some(column)),
            Target::Row { control, row } => (control, row, None),
            _ => return Outcome::Stay,
        };
        if cx.dialog.controls[c].id != Some(SICK_LIST) {
            return Outcome::Stay;
        }
        let Some(list) = view.list(c) else { return Outcome::Stay };
        let Some(&player) = list.keys.get(list.scroll + row) else { return Outcome::Stay };
        if column == Some(STATUS_COLUMN) {
            if toggle_injection(cx, player) {
                if let Some(list) = view.list_mut(c) {
                    list.scroll = 0;
                }
                self.fill(cx, view);
            }
            return Outcome::Stay;
        }
        visit(cx, player)
    }
}

impl Medical {
    /// The sick list (`FUN_00517afe`).
    fn fill(&self, cx: &mut Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(season) = &cx.state.season else { return };
        let Some(key) = season.manager().and_then(|m| m.club) else { return };
        let Some(club) = season.clubs.get(&key) else { return };
        let records = cx.state.records.as_ref();
        let record = |p: usize| records?.player(club.status.get(p)?.id);
        let knocks = |p: usize| record(p).map_or(0, |r| r.knocks());
        let players = medical::sick_list(club, knocks);
        let Some(list) = control(dialog, SICK_LIST).and_then(|c| view.list_mut(c)) else { return };
        list.selected = None;
        list.inks.clear();
        list.row_icons.clear();
        if players.is_empty() {
            // One column, one line saying so.
            list.tabs = vec![0.0];
            list.rows = vec![game.text(NOBODY).to_owned()];
            list.keys.clear();
            return;
        }
        list.tabs.clone_from(&self.tabs);
        let pitch = &game.rules.pitch;
        let today = season.day;
        let mut rows = Vec::new();
        let mut icons = Vec::new();
        for &p in &players {
            let player = &club.players[p];
            let status = club.status.get(p).copied().unwrap_or_default();
            let kind = pitch.abbreviations.get(usize::from(player.positions[0])).copied();
            let position = game.text(kind.unwrap_or(0) as u16).to_owned();
            let strength =
                if status.strength_hidden { "?".to_owned() } else { player.strength.to_string() };
            let name = duties::short_name(cx, player, status.id);
            let r = record(p);
            let row = if status.injured != 0 {
                let injury = r.map_or(0, |r| r.injury_kind());
                let injury = game.rules.words.injuries.get(usize::from(injury)).copied();
                let since = r.map_or_else(|| "-".to_owned(), |r| since(r.injured_on(), today));
                let fit = fit_against(cx, season, key, today + status.injured.max(0) as usize);
                let visited = r.is_some_and(medical::visited);
                [
                    name,
                    position,
                    strength,
                    game.text(injury.unwrap_or(0)).to_owned(),
                    " ".to_owned(),
                    since,
                    status.injured.to_string(),
                    fit,
                    game.text(if visited { YES } else { NO }).to_owned(),
                ]
            } else {
                [
                    name,
                    position,
                    strength,
                    game.text(medical::knock_text(knocks(p))).to_owned(),
                    String::new(),
                    "-".to_owned(),
                    KNOCK_DAYS.to_owned(),
                    "?".to_owned(),
                    String::new(),
                ]
            };
            rows.push(row.join("\t"));
            let k = knocks(p);
            let shown = [
                (k & medical::INJECTION != 0).then_some(INJECTION_ICON),
                (k & 0xc000 != 0 && k & medical::OPERATION_DUE != 0).then_some(OPERATION_ICON),
            ];
            icons.push(ICON_SLOTS.into_iter().zip(shown.into_iter().flatten()).collect());
        }
        list.rows = rows;
        list.row_icons = icons;
        list.keyed = vec![INJECTION_ICON, OPERATION_ICON];
        list.keys = players;
    }
}

/// How long ago an injury began, by the season's day it began on.
fn since(began: u16, today: usize) -> String {
    let (began, today) = (i32::from(began), today as i32);
    if began > today {
        (today - began + 364).to_string()
    } else if began <= 0 {
        "-".to_owned()
    } else {
        (today - began).to_string()
    }
}

/// Against whom a player out until `day` is fit again: the club's first
/// league opponent from then on (`FUN_0062b402`), or the season's end.
fn fit_against(cx: &Cx, season: &Season, key: (usize, usize), day: usize) -> String {
    let (game, world) = (cx.game, &cx.game.world);
    let next = season.next_match_from(world, &game.calendar, key, day);
    match next {
        Some(m) => {
            let other = if m.home == key { m.away } else { m.home };
            let club = world.country(other.0).and_then(|c| c.clubs.get(other.1));
            club.map_or_else(String::new, |c| c.name.clone())
        }
        None if day < SEASON_END => game.text(UNTIL_SEASON_END).to_owned(),
        None => game.text(NEXT_SEASON).to_owned(),
    }
}

/// What the doctor says (`FUN_00518783`): of the injured, in the list's
/// order, the managers left out.
fn notes(cx: &Cx, view: &mut View) {
    let Some(season) = &cx.state.season else { return };
    let Some(club) = season.manager().and_then(|m| m.club).and_then(|k| season.clubs.get(&k))
    else {
        return;
    };
    let Some(records) = cx.state.records.as_ref() else { return };
    let mut rows = Vec::new();
    for p in medical::sick_list(club, |_| 0) {
        let id = club.status.get(p).map_or(0, |s| s.id);
        let Some(r) = (id <= LAST_REAL_ID).then(|| records.player(id)).flatten() else { continue };
        if let Some(note) = medical::note(r, medical::visited(r)) {
            let name = records.known_as(id).unwrap_or_default();
            rows.push(format!("{name} {}", cx.game.text(note)));
        }
    }
    if let Some(list) = control(cx.dialog, NOTES).and_then(|c| view.list_mut(c)) {
        list.rows = rows;
    }
}

/// The current manager's club as the season follows it.
fn club<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

/// The injection given or taken back (`OnUserC000_00518342`), with the
/// club and in his record; whether anything changed.
fn toggle_injection(cx: &mut Cx, player: usize) -> bool {
    let tables = &cx.game.rules.medical;
    let Some(status) = club(cx).and_then(|c| c.status.get(player)).copied() else { return false };
    let Some(r) = cx.state.records.as_mut().and_then(|r| r.players.get_mut(usize::from(status.id)))
    else {
        return false;
    };
    if status.id == 0 || !medical::toggle_injection(tables, r, status.injured) {
        return false;
    }
    let injected =
        r.knocks() & medical::INJECTION != 0 && tables.injection_allowed(r.injury_kind());
    let season = cx.state.season.as_mut();
    let key = season.as_ref().and_then(|s| s.manager()?.club);
    if let (Some(season), Some(key)) = (season, key)
        && let Some(status) = season.clubs.get_mut(&key).and_then(|c| c.status.get_mut(player))
    {
        status.injected = injected;
    }
    true
}

/// A sick visit asked for (`FUN_00518436`): not with himself, not with a
/// player who is not injured, nor with another manager, and only on a
/// week's first day; then the planner books it.
fn visit(cx: &mut Cx, player: usize) -> Outcome {
    let Some(season) = cx.state.season.as_ref() else { return Outcome::Stay };
    let Some(status) = club(cx).and_then(|c| c.status.get(player)).copied() else {
        return Outcome::Stay;
    };
    let own = cx.state.records.as_ref().and_then(|r| r.human_slot(season.current));
    if own.is_some_and(|slot| status.id == LAST_REAL_ID + 1 + slot as u16) {
        return Outcome::Popup(Request::with(popups::NOTICE, Args::Text(TALK_TO_YOURSELF)));
    }
    if status.injured <= 0 || status.id == 0 {
        return Outcome::Stay;
    }
    if status.id > LAST_REAL_ID {
        return Outcome::Popup(Request::with(
            popups::NOTICE,
            Args::TextFor(TALK_TO_HIM, status.id),
        ));
    }
    if season.day % WEEK != 0 {
        return Outcome::Popup(Request::with(popups::NOTICE, Args::Text(NEXT_WEEK)));
    }
    Outcome::Popup(Request::with(planner::PLANNER, Args::Book(VISIT_KIND, status.id)))
}

/// The operations due, at OK (`FUN_00767917`); the club follows them.
fn operate(cx: &mut Cx) {
    let Some(club) = club(cx) else { return };
    let squad: Vec<(u16, i32)> = club.status.iter().map(|s| (s.id, s.injured)).collect();
    let Some(records) = cx.state.records.as_mut() else { return };
    let operated = medical::operate(records, &squad);
    let Some(season) = cx.state.season.as_mut() else { return };
    let Some(key) = season.manager().and_then(|m| m.club) else { return };
    let Some(club) = season.clubs.get_mut(&key) else { return };
    for (id, days) in operated {
        if let Some(status) = club.status.iter_mut().find(|s| s.id == id) {
            status.injured = days;
        }
    }
}

/// How a player took a sick visit (dialog 210, `CSpielerbesuch`): who was
/// visited, and his reaction; OK closes it.
pub struct Visit;

pub const VISIT: u16 = a3_core::cycle::dialogs::VISIT;
const VISIT_CAPTION_ID: u16 = 9927;
const VISIT_CAPTION: u16 = 1716;
const VISITED: u16 = 3058;
const REACTION: u16 = 3059;
const VISIT_OK: u16 = 3060;

/// Puts the visit's caption in its frame.
pub(crate) fn compose_visit(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&VISIT) else { return };
    dialog.controls.insert(0, super::caption(dialog.frame, VISIT_CAPTION_ID, String::new()));
}

impl Screen for Visit {
    fn ids(&self) -> &[u16] {
        &[VISIT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let Args::Visit { player, kind, draw } = args else { return };
        let dialog = cx.dialog;
        if let Some(season) = &cx.state.season {
            set(
                view,
                dialog,
                VISIT_CAPTION_ID,
                super::hub::club_title(cx.game, season, VISIT_CAPTION),
            );
        }
        let name = cx.state.records.as_ref().and_then(|r| r.known_as(player)).unwrap_or_default();
        let template = control(dialog, VISITED).map_or("", |c| dialog.controls[c].text.as_str());
        set(view, dialog, VISITED, template.replacen("%s", &name, 1));
        let reaction = medical::Reaction { kind, draw }.text();
        set(view, dialog, REACTION, cx.game.text(reaction).to_owned());
        popups::ball(view, cx);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(b) if cx.dialog.controls[b].id == Some(VISIT_OK) => {
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}
