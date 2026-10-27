//! The text menu, where a manager who chose it takes the turn: the bar's
//! symbols spelled out as words in seven groups, the next match with both
//! table places and strengths, and the board's confidence.
//! "Weiter" ends the turn; dialog 502, the national team's smaller
//! variant, is not built yet.

use tracing::info;

use a3_files::rch::{Dialog, Kind};

use a3_core::cycle::dialogs::TEXT_MENU;
use a3_core::records::Records;
use a3_core::stock;

use super::{Args, Cx, Outcome, Reply, Screen, hub, indices, office, set};
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

/// Controls by their ids in the dialog file: the seven option groups, the
/// three pushbuttons, and the three texts the game fills itself.
const GROUP_TRAINING: u16 = 6302;
const GROUP_CALENDAR: u16 = 6303;
const GROUP_MARKET: u16 = 6304;
const GROUP_FINANCES: u16 = 6305;
const GROUP_CLUB: u16 = 6306;
const GROUP_OTHER: u16 = 6307;
const GROUP_NATIONAL: u16 = 6308;
const MENU_OPTIONS: u16 = 6309;
const MENU_ANNOUNCE: u16 = 6310;
const MENU_FURTHER: u16 = 6311;
const MENU_PAIRING: u16 = 6313;
const MENU_STRENGTH: u16 = 6314;
const MENU_CONFIDENCE: u16 = 6478;

/// The entries, as the game's table at 0x882168 pairs group, item and
/// action (the action divided by 100 names the symbol on the bar); the
/// options button leads to the dressing room, as the office's remote
/// control does.  Behind "Statistiken" stands the bar's fourteenth
/// symbol, which leads nowhere built yet.
const ENTRIES: [(u16, usize, usize); 17] = [
    (GROUP_TRAINING, 0, hub::LINEUP_SYMBOL),
    (GROUP_TRAINING, 1, hub::COMPARISON_SYMBOL),
    (GROUP_TRAINING, 2, hub::TACTICS_SYMBOL),
    (GROUP_TRAINING, 3, hub::LONG_TRAINING_SYMBOL),
    (GROUP_TRAINING, 4, hub::SHORT_TRAINING_SYMBOL),
    (GROUP_TRAINING, 5, hub::INDIVIDUAL_TRAINING_SYMBOL),
    (GROUP_CALENDAR, 0, hub::YEAR_CALENDAR_SYMBOL),
    (GROUP_CALENDAR, 1, hub::WEEK_PLANNER_SYMBOL),
    (GROUP_MARKET, 0, hub::SQUAD_SYMBOL),
    (GROUP_FINANCES, 0, hub::BALANCE_SYMBOL),
    (GROUP_FINANCES, 1, hub::FINANCES_SYMBOL),
    (GROUP_FINANCES, 2, hub::STADIUM_SYMBOL),
    (GROUP_CLUB, 0, hub::BOARD_SYMBOL),
    (GROUP_CLUB, 1, hub::CONTACTS_SYMBOL),
    (GROUP_OTHER, 0, 14),
    (GROUP_OTHER, 1, hub::HONOURS_SYMBOL),
    (GROUP_NATIONAL, 0, hub::NATIONAL_SYMBOL),
];

/// The text menu.
pub struct MenuText;

impl Screen for MenuText {
    fn ids(&self) -> &[u16] {
        &[TEXT_MENU]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        show_menu(cx, view);
        let dialog = cx.dialog;
        // "Weiter" and the options always lead somewhere; an entry does
        // when its screen stands.
        let works = |t: Target| match t {
            Target::Button(b) => {
                let id = dialog.controls[b].id.unwrap_or(0);
                id == MENU_FURTHER
                    || id == MENU_OPTIONS && hub::bar_screen(hub::DRESSING_SYMBOL).is_some()
            }
            Target::Option { control, item } => entry_of(dialog, control, item)
                .is_some_and(|symbol| hub::bar_screen(symbol).is_some()),
            _ => false,
        };
        super::mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let symbol = match target {
            Target::Button(b) => {
                let id = cx.dialog.controls[b].id.unwrap_or(0);
                match id {
                    MENU_FURTHER => return hub::whistle(cx),
                    MENU_OPTIONS => hub::DRESSING_SYMBOL,
                    MENU_ANNOUNCE => {
                        info!("the match announcement: not built yet");
                        return Outcome::Stay;
                    }
                    _ => return Outcome::Stay,
                }
            }
            Target::Option { control, item } => {
                let Some(id) = cx.dialog.controls[control].id else { return Outcome::Stay };
                match ENTRIES.iter().find(|(group, entry, _)| *group == id && *entry == item) {
                    Some(&(_, _, symbol)) => symbol,
                    None => return Outcome::Stay,
                }
            }
            _ => return Outcome::Stay,
        };
        // An entry leads to the same screen as its symbol on the bar, and
        // stands or falls with it.
        if hub::bar_closed(cx, symbol) {
            return Outcome::Stay;
        }
        match hub::bar_screen(symbol) {
            Some(screen) => Outcome::Close(Reply::Goto(screen)),
            None => {
                info!("the menu entry {symbol}: not built yet");
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

/// The bar's symbol an option of the dialog leads to.
fn entry_of(dialog: &Dialog, control: usize, item: usize) -> Option<usize> {
    let id = dialog.controls[control].id?;
    ENTRIES
        .iter()
        .find(|(group, entry, _)| *group == id && *entry == item)
        .map(|&(_, _, symbol)| symbol)
}

/// The menu's own texts: the next match with both table places, the two
/// strengths, and the board's confidence.
fn show_menu(cx: &Cx, view: &mut View) {
    let (dialog, game) = (cx.dialog, cx.game);
    let Some(season) = &cx.state.season else { return };
    let Some(manager) = season.manager() else { return };
    let Some(club) = manager.club else { return };
    let records = cx.state.records.as_ref();
    if let (Some(records), Some(slot)) = (records, records.and_then(|r| r.human_slot(season.current)))
    {
        set(view, dialog, MENU_CONFIDENCE, format!("{} %", stock::confidence(records, slot)));
    }
    let Some(m) = season.next_match(&game.world, &game.calendar, club) else {
        // Without a match ahead the game names only one's own club.
        let (name, _) = club_of(game, records, club);
        set(view, dialog, MENU_PAIRING, name);
        return;
    };
    let home = m.home == club;
    let opponent = if home { m.away } else { m.home };
    // The pairing names both sides like the office's, each marked home or
    // away — and here each with its table place once the league has one.
    let (own_name, own_place) = club_of(game, records, club);
    let (their_name, their_place) = club_of(game, records, opponent);
    set(
        view,
        dialog,
        MENU_PAIRING,
        format!(
            "{} - {}",
            office::side(game, &own_name, home, place(own_place)),
            office::side(game, &their_name, !home, place(their_place)),
        ),
    );
    // One's own strength is what its line-up of the moment adds up to, the
    // opponent's what his record last saved (the club's +0x94 … +0x9a).
    let own = season.clubs.get(&club).map_or_else(
        || saved_strength(game, records, club),
        |state| {
            let parts = game.rules.pitch.parts(&state.lineup, &state.players, &state.fitness);
            Some(parts.iter().sum::<i32>() as i64 / 10)
        },
    );
    if let (Some(own), Some(theirs)) = (own, saved_strength(game, records, opponent)) {
        set(view, dialog, MENU_STRENGTH, format!("{own} : {theirs}"));
    }

    // A closed entry takes its group's disabled ink, as a shut button does.
    for c in indices(dialog, |k| matches!(k, Kind::Options { .. })) {
        let Kind::Options { items, .. } = &dialog.controls[c].kind else { continue };
        let dimmed = (0..items.len())
            .map(|item| entry_of(dialog, c, item).is_some_and(|s| hub::bar_closed(cx, s)))
            .collect::<Vec<bool>>();
        if let Some(options) = view.controls[c].state.as_options_mut() {
            options.dimmed = dimmed;
        }
    }
}

/// A club of the pairing: its name and its place in the table, 0 while
/// the league has none.
fn club_of(game: &Game, records: Option<&Records>, club: (usize, usize)) -> (String, u8) {
    let country = game.world.country(club.0);
    let name = country
        .and_then(|c| c.clubs.get(club.1))
        .map_or_else(String::new, |c| c.name.clone());
    let place = country
        .and_then(|c| records?.team(c.nation, u16::try_from(club.1).ok()?))
        .map_or(0, |t| t.table_place());
    (name, place)
}

/// The table place as the pairing texts want it, none while the league
/// has none.
fn place(place: u8) -> Option<usize> {
    (place != 0).then_some(place as usize)
}

/// What a club's record last saved it counts for (the club's +0x94 …
/// +0x9a), summed and divided by ten.
fn saved_strength(game: &Game, records: Option<&Records>, club: (usize, usize)) -> Option<i64> {
    let nation = game.world.country(club.0)?.nation;
    let team = records?.team(nation, u16::try_from(club.1).ok()?)?;
    Some(team.strengths().iter().map(|&part| i64::from(part)).sum::<i64>() / 10)
}
