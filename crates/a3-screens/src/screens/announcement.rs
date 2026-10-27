//! The next league match's announcement (dialog 358,
//! `CSpielankuendigungLiga`, its init `FUN_0054f27e` and paint
//! `FUN_00550164`): the league and season, the matchday, both clubs with
//! their crests, last results and places, the first leg, last season's
//! game, the home side's record against the other, the teams' strengths,
//! the advance sales, the matchday's other games and, for Germany, the map
//! with both clubs marked.
//!
//! The planner opens it when the next match is a league match within a
//! week of today (`FUN_0058e33c`).

use std::collections::HashMap;

use a3_core::opponent::{self, League};
use a3_core::records::{Records, Team};
use a3_files::rch::{Align, Control, Dialog, Kind};
use bevy_math::{Rect, Vec2};

use super::finances::count;
use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel, set};
use crate::controls::bitmap::Picture;
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

pub const LEAGUE_ANNOUNCEMENT: u16 = 358;
/// The match's kind for the league (club +0xb4c).
pub const LEAGUE_MATCH: u8 = 2;
/// How far ahead the announcement shows a match, in days.
pub const AHEAD: i16 = 7;

const TITLE: u16 = 2973;
const MATCHDAY: u16 = 2974;
/// Each side's name, last results and place: the home side's, the away
/// side's (`DAT_008880a4`, `DAT_008880b0`).
const SIDES: [[u16; 3]; 2] = [[2975, 2976, 2978], [2977, 2980, 2979]];
const CRESTS: [u16; 2] = [2971, 2972];
const FIRST_LEG: u16 = 2981;
const LAST_YEAR: u16 = 2982;
const TENDENCY: u16 = 2983;
const STRENGTHS: u16 = 2984;
const ADVANCE: u16 = 2985;
const GAMES: u16 = 4570;
const GAME_TABS: [f32; 3] = [0.0, 150.0, 158.0];
const SEASON_TEXT: u16 = 1852;
const MATCHDAY_TEXT: u16 = 1529;
/// The season tickets the home side's plan counts on (club +0x124, team
/// +0x39c), which the sales add to.
const SEASON_TICKETS: usize = 0x124 + a3_core::finances::CLUB_TO_TEAM;

/// The map: its picture by the country's cup system — Germany's (0xb)
/// would be a file named at run time and falls back to the first —, where
/// it is drawn, and how a club's place on it is scaled.
const MAPS: [u16; 6] = [2304, 2305, 2306, 2307, 2308, 2780];
const GERMANY: u8 = 0xb;
const MAP: u16 = 9940;
const MARKS: u16 = 9941;
const COUNTRY: u16 = 9942;
const MAP_RECT: [f32; 4] = [13.0, 199.0, 313.0, 418.0];
const COUNTRY_AT: Vec2 = Vec2::new(35.0, 400.0);
/// The own club red, the other green (COLORREFs).
const MARK_COLOURS: [u32; 2] = [0x0000ff, 0x00ff00];
const MARK: i32 = 3;

/// The map, its marks and the country's name are the game's own drawing,
/// not the dialog file's.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&LEAGUE_ANNOUNCEMENT) else { return };
    let [x0, y0, x1, y1] = MAP_RECT;
    let rect = Rect::new(x0, y0, x1, y1);
    let control = |kind, id, rect| Control {
        kind,
        rect,
        align: Align::Left,
        text: String::new(),
        id: Some(id),
        font: a3_files::rch::PLAIN_FONT,
        ink: [0; 3],
    };
    dialog.controls.push(control(Kind::Bitmap, MAP, rect));
    dialog.controls.push(control(Kind::Paint, MARKS, rect));
    let name = Rect::from_corners(COUNTRY_AT, COUNTRY_AT + Vec2::new(200.0, 16.0));
    dialog.controls.push(control(Kind::Static, COUNTRY, name));
}

/// The manager's club's next match, if it is a league match within a week
/// of today: his team and the other.
pub(crate) fn next_league_match<'a>(cx: &'a Cx) -> Option<(&'a Team, &'a Team)> {
    let own = super::squad::own_team(cx)?;
    let records = cx.state.records.as_ref()?;
    let today = records.globals.day() as i16;
    if own.next_country() == 0 || own.next_day() >= today + AHEAD {
        return None;
    }
    let other = records.team(own.next_country().into(), own.next_number() as u16)?;
    (own.next_kind() == LEAGUE_MATCH).then_some((own, other))
}

/// Whether the announcement button does what the original's does: a game
/// with its records, whose next match within the week, if any, is a
/// league match (the cup's dialog 201 is not built).
pub(crate) fn button_works(cx: &Cx) -> bool {
    let Some(own) = super::squad::own_team(cx) else { return false };
    let Some(records) = cx.state.records.as_ref() else { return false };
    let today = records.globals.day() as i16;
    let due = own.next_country() != 0 && own.next_day() < today + AHEAD;
    !due || own.next_kind() == LEAGUE_MATCH
}

/// "11." — a number with the game's ordinal suffix, which is "." for all
/// (`FUN_00730af1`).
fn ordinal(n: impl std::fmt::Display) -> String {
    format!("{n}.")
}

/// A team's club by the world's key: its country's slot and its number.
fn key(game: &Game, team: &Team) -> Option<(usize, usize)> {
    Some((game.world.slot_of(team.country().into())?, usize::from(team.number())))
}

fn crest(game: &Game, team: &Team) -> Option<Picture> {
    game.rules.crest(&game.world, key(game, team)?).map(Picture::file)
}

/// What a team counts for, its four parts reckoned anew from its line-up
/// as the game does first (`FUN_006a18c2`) and summed, in whole points:
/// the manager's own club as he has set it, another as its record has it.
fn strength(cx: &Cx, team: &Team) -> i32 {
    let (game, pitch) = (cx.game, &cx.game.rules.pitch);
    let own = cx.state.season.as_ref().and_then(|s| s.manager()?.club);
    let parts = key(game, team).and_then(|key| {
        if Some(key) == own {
            let club = cx.state.season.as_ref()?.clubs.get(&key)?;
            return Some(pitch.parts(&club.lineup, &club.players, &club.fitness));
        }
        let club = a3_core::load::club(cx.state.records.as_ref()?, &game.data, key)?;
        Some(pitch.parts(&club.lineup, &club.players, &club.fitness))
    });
    let parts = parts.unwrap_or_else(|| team.strengths().map(i32::from));
    parts.iter().sum::<i32>() / 10
}

#[derive(Default)]
pub struct Announcement;

impl Screen for Announcement {
    fn ids(&self) -> &[u16] {
        &[LEAGUE_ANNOUNCEMENT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some((own, other)) = next_league_match(cx) else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        let own_home = own.next_venue() <= 0;
        let sides = if own_home { [own, other] } else { [other, own] };
        let [home, away] = sides;
        let league = League::of(records, own.country().into(), own.level());

        // "1. Bundesliga 2025/26".
        let year = i32::from(records.globals.start_year());
        let next = if (year + 1) % 100 == 0 {
            (year + 1).to_string()
        } else {
            format!("{:02}", (year + 1) % 100)
        };
        let name = league.as_ref().map(League::name).unwrap_or_default();
        let title = game.text(SEASON_TEXT).replacen("%s", &name, 1).replacen(
            "%s",
            &year.to_string(),
            1,
        );
        set(view, dialog, TITLE, title.replacen("%s", &next, 1));
        let played = league.as_ref().map_or(0, League::played);
        let matchday = game.text(MATCHDAY_TEXT).replacen("%d", &(played + 1).to_string(), 1);
        set(view, dialog, MATCHDAY, matchday.replacen("%s", ".", 1));

        for (team, [name, form, place]) in sides.into_iter().zip(SIDES) {
            set(view, dialog, name, team.name());
            let stand = opponent::stand(records, team);
            set(view, dialog, form, stand.map(|s| s.form).unwrap_or_default());
            let table = team.table_place();
            let shown = if table > 0 { format!("({})", ordinal(table)) } else { String::new() };
            set(view, dialog, place, shown);
        }
        for (team, id) in sides.into_iter().zip(CRESTS) {
            if let Some(c) = control(dialog, id) {
                view.set_picture(c, crest(game, team));
            }
        }

        let calendar = &game.calendar;
        let goals =
            |g: Option<(u8, u8)>| g.map_or_else(|| "-".to_owned(), |(a, b)| format!("{a}:{b}"));
        set(view, dialog, FIRST_LEG, goals(opponent::first_leg(records, calendar, home, away)));
        let last = opponent::archived_result(records, home, away, 0, false);
        set(view, dialog, LAST_YEAR, goals(last));
        let [w, d, l] = opponent::home_record(records, calendar, home, away);
        set(view, dialog, TENDENCY, format!("{w}:{d}:{l}"));
        let strengths = format!("{} : {}", strength(cx, home), strength(cx, away));
        set(view, dialog, STRENGTHS, strengths);
        let sale = a3_core::tickets::sale(
            records,
            (home.country(), i16::from(home.number())),
            (away.country(), i16::from(away.number())),
            own.next_day(),
        );
        let advance = sale.map(|s| count(s.sold + home.get::<i32>(SEASON_TICKETS)));
        set(view, dialog, ADVANCE, advance.unwrap_or_else(|| "-".to_owned()));

        if let (Some(league), Some(list)) = (&league, control(dialog, GAMES)) {
            let rows = games(records, league, own.country().into(), played);
            if let Some(shown) = view.list_mut(list) {
                shown.tabs = GAME_TABS.to_vec();
                shown.rows = rows;
            }
        }
        show_map(game, records, own, other, view, dialog);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if ok || cancel { Outcome::Close(Reply::Ok) } else { Outcome::Stay }
    }
}

/// The matchday's games in the schedule's order, with their places once
/// a matchday is played.
fn games(records: &Records, league: &League, country: u16, played: usize) -> Vec<String> {
    let side = |slot: usize| {
        let number = league.team(slot);
        let team = u16::try_from(number).ok().and_then(|n| records.team(country, n));
        (team.map(Team::name).unwrap_or_default(), team.map_or(0, Team::table_place))
    };
    league
        .pairs(played)
        .into_iter()
        .map(|(h, a)| {
            let ((home, hp), (away, ap)) = (side(h), side(a));
            if played >= 1 {
                format!("{home} ({hp}.)\t-\t{away} ({ap}.)")
            } else {
                format!("{home}\t-\t{away}")
            }
        })
        .collect()
}

/// The map of the own club's country with both clubs marked, and the
/// country's name, for the countries with a map.
fn show_map(
    game: &Game,
    records: &Records,
    own: &Team,
    other: &Team,
    view: &mut View,
    dialog: &Dialog,
) {
    let system = records.countries.cup_system(own.country().into());
    let picture =
        if system == GERMANY { Some(MAPS[0]) } else { MAPS.get(usize::from(system)).copied() };
    let Some(picture) = picture else { return };
    if let Some(c) = control(dialog, MAP) {
        view.set_picture(c, Some(Picture::graphic(picture)));
    }
    if let Some(marks) = control(dialog, MARKS).and_then(|c| view.paint_mut(c)) {
        marks.bars.clear();
        for (team, colour) in [own, other].into_iter().zip(MARK_COLOURS) {
            let [x, y] = team.map_at();
            let x = i32::from(x) * 30 / 44;
            let y = i32::from(y) * 218 / 320;
            marks.bar(x, y, MARK, MARK, colour);
        }
    }
    set(view, dialog, COUNTRY, game.world.nation(own.country().into()).to_owned());
}
