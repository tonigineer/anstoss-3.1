//! The office, where a manager who chose it takes the turn: who sits in it
//! and when, the next match, and the objects there to use today; the
//! grandest clubs have a grand office. The jacket on the door ends the turn.

use std::collections::HashMap;

use bevy_math::Vec2;
use tracing::info;

use a3_core::records::Records;
use a3_core::season::Match;
use a3_files::rch::{Dialog, Kind};

use a3_core::cycle::dialogs::{GRAND_OFFICE, OFFICE};

use super::calendar::bitmap;
use super::{Args, Cx, Outcome, Reply, Screen, control, hub, indices, menu_text, set};
use crate::controls::bitmap::Picture;
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

/// Controls by their ids in the dialog file.
const OFFICE_TITLE: u16 = 6353;
const OFFICE_PAIRING: u16 = 6354;
const OFFICE_DOOR: u16 = 6131;
/// The objects in the room and the symbols of the bar they stand for, as
/// the game's table at 0x87fc38 pairs them (by its actions divided by
/// 100); the jacket on the door ends the turn.  The grand office shares
/// this table; the national office, dialog 484, keeps most of these ids at
/// other places and drops squad, money, both trainings, nation and
/// honours; the file titles it "ANSTOSS 3 - Büro".
const OBJECTS: [(u16, usize); 18] = [
    (6116, hub::LINEUP_SYMBOL),
    (6117, hub::COMPARISON_SYMBOL),
    (6124, hub::TACTICS_SYMBOL),
    (6127, hub::LONG_TRAINING_SYMBOL),
    (6128, hub::SHORT_TRAINING_SYMBOL),
    (6129, hub::INDIVIDUAL_TRAINING_SYMBOL),
    (6118, hub::YEAR_CALENDAR_SYMBOL),
    (6123, hub::WEEK_PLANNER_SYMBOL),
    (6126, hub::BALANCE_SYMBOL),
    (6125, hub::FINANCES_SYMBOL),
    (6120, hub::SQUAD_SYMBOL),
    (5777, hub::STADIUM_SYMBOL),
    (6130, hub::CONTACTS_SYMBOL),
    (6122, hub::BOARD_SYMBOL),
    // The fourteenth object, behind the table on the wall: the bar's
    // fourteenth symbol, which leads nowhere built yet.
    (6115, 14),
    (6315, hub::NATIONAL_SYMBOL),
    (6119, hub::HONOURS_SYMBOL),
    // The remote control: the dressing room and the game's options.
    (6121, hub::DRESSING_SYMBOL),
];
/// The crest on the office pennant, which the game draws itself, under an
/// id of its own.
const OFFICE_CREST_ID: u16 = 9910;
const OFFICE_PENNANT: u16 = 6130;
const OFFICE_CREST: Vec2 = Vec2::new(11.0, 27.0);
const GRAND_OFFICE_CREST: Vec2 = Vec2::new(9.0, 22.0);
const CREST_SIZE: f32 = 32.0;

/// Texts by id.
const DATE: u16 = 1997;
const HOME: u16 = 3664;
const AWAY: u16 = 3665;
const HOME_RANKED: u16 = 4242;
const AWAY_RANKED: u16 = 4243;

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
        show_office(cx, view);
        let dialog = cx.dialog;
        // The door always leads somewhere; an object does when its
        // screen stands.
        let works = |t: Target| {
            let Target::Button(b) = t else { return false };
            match dialog.controls[b].id.unwrap_or(0) {
                OFFICE_DOOR => true,
                id => OBJECTS
                    .iter()
                    .find(|&&(object, _)| object == id)
                    .is_some_and(|&(_, symbol)| hub::bar_screen(symbol).is_some()),
            }
        };
        super::mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let Target::Button(button) = target else { return Outcome::Stay };
        let id = cx.dialog.controls[button].id.unwrap_or(0);
        if id == OFFICE_DOOR {
            return hub::whistle(cx);
        }
        // An object leads to the same screen as its symbol on the bar,
        // and stands or falls with it.
        let Some(&(_, symbol)) = OBJECTS.iter().find(|&&(object, _)| object == id) else {
            return Outcome::Stay;
        };
        if hub::bar_closed(cx, symbol) {
            return Outcome::Stay;
        }
        match hub::bar_screen(symbol) {
            Some(screen) => Outcome::Close(Reply::Goto(screen)),
            None => {
                info!("the object {id}: not built yet");
                Outcome::Stay
            }
        }
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
fn show_office(cx: &Cx, view: &mut View) {
    let (dialog, game) = (cx.dialog, cx.game);
    let Some(season) = &cx.state.season else { return };
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
            |(club, m)| pairing(game, cx.state.records.as_ref(), club, &m),
        );
    set(view, dialog, OFFICE_PAIRING, pairing);
    if let Some(crest) = control(dialog, OFFICE_CREST_ID) {
        let picture = manager.club.and_then(|c| game.rules.crest(&game.world, c));
        view.set_picture(crest, picture.map(Picture::file));
    }

    // An object stands or falls with its symbol on the bar.
    for button in indices(dialog, |k| matches!(k, Kind::Button { .. })) {
        let id = dialog.controls[button].id.unwrap_or(0);
        let disabled = id != OFFICE_DOOR
            && OBJECTS
                .iter()
                .find(|&&(object, _)| object == id)
                .is_some_and(|&(_, symbol)| hub::bar_closed(cx, symbol));
        view.set_disabled(button, disabled);
    }
}

/// "Own club (A, 6.) - opponent (H, 9.)": the manager's club first, each
/// side marked home or away, with its table position once the league has
/// one (seen so on the original's office, as on its text menu).
fn pairing(game: &Game, records: Option<&Records>, own: (usize, usize), m: &Match) -> String {
    let home = m.home == own;
    let opponent = if home { m.away } else { m.home };
    let one = |club: (usize, usize), home: bool| {
        let (name, place) = menu_text::club_of(game, records, club);
        side(game, &name, home, menu_text::place(place))
    };
    format!("{} - {}", one(own, home), one(opponent, !home))
}

/// One side of a pairing: "Name (H)", or "Name (H, 3.)" with its position.
pub(crate) fn side(game: &Game, name: &str, home: bool, position: Option<usize>) -> String {
    match position {
        Some(position) => game
            .text(if home { HOME_RANKED } else { AWAY_RANKED })
            .replacen("%s", name, 1)
            .replacen("%d", &position.to_string(), 1),
        None => game.text(if home { HOME } else { AWAY }).replacen("%s", name, 1),
    }
}
