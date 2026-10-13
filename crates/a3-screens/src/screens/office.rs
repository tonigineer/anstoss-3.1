//! The office, where a manager who chose it takes the turn: who sits in it
//! and when, the next match, and the objects there to use today; the
//! grandest clubs have a grand office. The jacket on the door ends the turn.

use std::collections::HashMap;

use bevy_math::Vec2;

use a3_core::season::{Match, Season};
use a3_files::rch::{Dialog, Kind};

use a3_core::cycle::dialogs::{GRAND_OFFICE, OFFICE, QUESTION};

use super::calendar::{WEEK_DAYS, bitmap};
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, indices, set};
use crate::controls::bitmap::Picture;
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

/// Controls by their ids in the dialog file.
const OFFICE_TITLE: u16 = 6353;
const OFFICE_PAIRING: u16 = 6354;
const OFFICE_DOOR: u16 = 6131;
/// The national-team trophy and the remote control: the only objects a
/// manager without a club can use, besides the door.
const OFFICE_NATIONAL: u16 = 6315;
const OFFICE_REMOTE: u16 = 6121;
/// The long-term and individual training boards, closed on Saturdays.
const OFFICE_SATURDAY_CLOSED: [u16; 2] = [6127, 6129];
/// The crest on the office pennant, which the game draws itself, under an
/// id of its own.
const OFFICE_CREST_ID: u16 = 9910;
const OFFICE_PENNANT: u16 = 6130;
const OFFICE_CREST: Vec2 = Vec2::new(11.0, 27.0);
const GRAND_OFFICE_CREST: Vec2 = Vec2::new(9.0, 22.0);
const CREST_SIZE: f32 = 32.0;
/// What the game takes for Saturday.
const SATURDAY: usize = 6;

/// Texts by id.
const DATE: u16 = 1997;
const HOME: u16 = 3664;
const AWAY: u16 = 3665;
const HOME_RANKED: u16 = 4242;
const AWAY_RANKED: u16 = 4243;
const NO_TRAINING: u16 = 3238;

/// The office pennant carries the club's crest.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for (office, at) in [(OFFICE, OFFICE_CREST), (GRAND_OFFICE, GRAND_OFFICE_CREST)] {
        let Some(dialog) = dialogs.get_mut(&office) else { continue };
        let pennant = dialog.controls.iter().find(|c| c.id == Some(OFFICE_PENNANT));
        let Some(origin) = pennant.map(|c| c.rect.min) else { continue };
        dialog.controls.push(bitmap(origin + at, CREST_SIZE, OFFICE_CREST_ID));
    }
}

/// Both offices.
pub struct Office;

impl Screen for Office {
    fn ids(&self) -> &[u16] {
        &[OFFICE, GRAND_OFFICE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        if let Some(season) = &cx.state.season {
            show_office(view, cx.dialog, cx.game, season);
        }
        // Only the door leads anywhere yet.
        let dialog = cx.dialog;
        let works = |t: Target| matches!(t, Target::Button(b) if dialog.controls[b].id == Some(OFFICE_DOOR));
        super::mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let Target::Button(button) = target else { return Outcome::Stay };
        if cx.dialog.controls[button].id == Some(OFFICE_DOOR) {
            // Training is never set for the coming week, so the game asks.
            return Outcome::Popup(Request::with(QUESTION, Args::Text(NO_TRAINING)));
        }
        Outcome::Stay
    }

    /// Yes to ending the turn without training set.
    fn answer(&mut self, _: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        match reply {
            Reply::Ok => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// The office: who sits in it and when, the next match, and the objects a
/// manager can use today.
fn show_office(view: &mut View, dialog: &Dialog, game: &Game, season: &Season) {
    let Some(manager) = season.manager() else { return };
    let date = season.date();
    let date = game
        .text(DATE)
        .replace("%1", &date.format("%-d").to_string())
        .replace("%2", &date.format("%-m").to_string())
        .replace("%3", &date.format("%Y").to_string());
    let club = manager.club.and_then(|(slot, club)| game.world.country(slot)?.clubs.get(club));
    // The game leaves an empty word in front of the name.
    let title = match club {
        Some(club) => {
            format!(" {} {} ({}), {date}", manager.first_name, manager.last_name, club.name)
        }
        None => format!(" {} {}, {date}", manager.first_name, manager.last_name),
    };
    set(view, dialog, OFFICE_TITLE, title);
    let pairing = manager
        .club
        .and_then(|club| season.next_match(&game.world, &game.calendar, club).map(|m| (club, m)))
        .map_or_else(
            || club.map(|c| c.name.clone()).unwrap_or_default(),
            |(club, m)| pairing(game, club, &m),
        );
    set(view, dialog, OFFICE_PAIRING, pairing);
    if let Some(crest) = control(dialog, OFFICE_CREST_ID) {
        let picture = manager.club.and_then(|c| game.rules.crest(&game.world, c));
        view.set_picture(crest, picture.map(Picture::file));
    }

    let saturday = season.day % WEEK_DAYS == SATURDAY;
    for button in indices(dialog, |k| matches!(k, Kind::Button { .. })) {
        let id = dialog.controls[button].id.unwrap_or(0);
        let disabled = match id {
            OFFICE_DOOR | OFFICE_REMOTE => false,
            OFFICE_NATIONAL => manager.national_team.is_none(),
            _ if manager.club.is_none() => true,
            _ => saturday && OFFICE_SATURDAY_CLOSED.contains(&id),
        };
        view.set_disabled(button, disabled);
    }
}

/// "Own club (H) - opponent (A)": the manager's club first, each side marked
/// home or away, with its table position once the league has begun.
fn pairing(game: &Game, own: (usize, usize), m: &Match) -> String {
    let name = |(slot, club): (usize, usize)| {
        game.world.country(slot).and_then(|c| c.clubs.get(club)).map_or("", |c| c.name.as_str())
    };
    let home = m.home == own;
    let opponent = if home { m.away } else { m.home };
    // There is no table before the first matchday, and so no positions.
    format!("{} - {}", side(game, name(own), home, None), side(game, name(opponent), !home, None))
}

/// One side of a pairing: "Name (H)", or "Name (H, 3.)" with its position.
fn side(game: &Game, name: &str, home: bool, position: Option<usize>) -> String {
    match position {
        Some(position) => game
            .text(if home { HOME_RANKED } else { AWAY_RANKED })
            .replacen("%s", name, 1)
            .replacen("%d", &position.to_string(), 1),
        None => game.text(if home { HOME } else { AWAY }).replacen("%s", name, 1),
    }
}
