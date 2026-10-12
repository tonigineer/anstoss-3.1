//! A manager's turn on the game's main screens, the way it shows them by
//! default: with the menu bar, nineteen symbols along the bottom that lead
//! to every part of the game. The turn opens on the line-up; a manager
//! without a club starts in the dressing room, where the game's options are.
//!
//! The line-up shows the club's formation on the pitch, a box for every
//! spot, the bench beside it and the whole squad below, with what the team
//! counts for in goal, defence, midfield and attack. The whistle at the end
//! of the bar ends the turn.

use std::collections::HashMap;

use bevy::prelude::*;

use super::flow::{Action, Session, control};
use super::turn::{self, Question};
use super::{PlayerBox, Target, View};
use crate::game::Game;
use crate::lineup::{self, BENCH, FIELD, Lineup, PARTS};
use crate::rch::{Align, Control, Dialog, Item, Kind};
use crate::season::{BAR_SYMBOLS, Rules, Season};
use crate::world::Player;

pub const LINEUP: u16 = 147;
pub const DRESSING_ROOM: u16 = 302;
pub const DIALOGS: [u16; 2] = [LINEUP, DRESSING_ROOM];

/// The bar's pictures: as it stands, with a symbol pressed, lit — every
/// other symbol from a strip of its own — and greyed out.
pub const BAR_NORMAL: u16 = 1130;
pub const BAR_PRESSED: u16 = 1131;
pub const BAR_LIT: [u16; 2] = [1132, 2510];
pub const BAR_OFF: u16 = 1133;
pub const BAR_IMAGES: [u16; 5] = [BAR_NORMAL, BAR_PRESSED, BAR_LIT[0], BAR_LIT[1], BAR_OFF];
/// A player's box: blue with a player in it, yellow without.
pub const BOX_FILLED: u16 = 1253;
pub const BOX_EMPTY: u16 = 1255;
/// The bar runs along the bottom of the screen; its symbols are as wide as
/// it is high.
const BAR: Rect = Rect { min: Vec2::new(0.0, 448.0), max: Vec2::new(640.0, 480.0) };
const SYMBOL: f32 = 32.0;
/// The bar's symbols by number: the line-up, the training the game keeps
/// shut on Saturdays, the national team, the dressing room and the whistle.
const LINEUP_SYMBOL: usize = 0;
const SATURDAY_CLOSED: [usize; 2] = [3, 5];
const NATIONAL_SYMBOL: usize = 15;
const DRESSING_SYMBOL: usize = 17;
pub const WHISTLE: usize = 18;
/// What the game takes for Saturday.
const SATURDAY: usize = 6;
const WEEK_DAYS: usize = 7;
/// The screen's title, which the game draws into its frame.
const CAPTION_RECT: Rect = Rect { min: Vec2::new(5.0, 3.0), max: Vec2::new(635.0, 19.0) };
const LINEUP_CAPTION: u16 = 2515;
const DRESSING_CAPTION: u16 = 2527;
const DATE: u16 = 1997;

/// Ids for the controls the game makes itself.
const CAPTION_ID: u16 = 9920;
const BAR_ID: u16 = 9930;
/// What the bar takes the place of: the OK button and, on the line-up, the
/// second row of column titles, which the game moves away and hides.
const OK_BUTTON: u16 = 1;
const SECOND_TITLES: u16 = 5515;

/// The line-up's controls: the boxes on the pitch and the bench.
const FIELD_BOXES: u16 = 2510;
const BENCH_BOXES: u16 = 2521;
/// The pitch on the screen: where its grid starts, and how big a box is.
const PITCH_ORIGIN: Vec2 = Vec2::new(14.0, 22.0);
const BOX_SIZE: Vec2 = Vec2::new(99.0, 24.0);
/// The bench's boxes, one under the other.
const BENCH_ORIGIN: Vec2 = Vec2::new(530.0, 53.0);
const BENCH_STEP: f32 = 26.0;
/// The squad list, its two rows of column titles, the "Spz."/"Auto." and
/// "Aufgaben im Spiel" buttons.
const SQUAD: u16 = 2509;
const TITLES: u16 = 5514;
const SPECIAL: u16 = 2572;
const DUTIES: u16 = 5266;
/// The squad list's columns as the game sets them: where each starts, and
/// how the centred and right-aligned ones sit in their width.
const COLUMNS: [(f32, Option<(Align, f32)>); 11] = [
    (0.0, None),
    (105.0, Some((Align::Centre, 30.0))),
    (136.0, Some((Align::Centre, 40.0))),
    (225.0, Some((Align::Centre, 20.0))),
    (245.0, Some((Align::Right, 18.0))),
    (264.0, None),
    (270.0, Some((Align::Right, 25.0))),
    (295.0, Some((Align::Right, 25.0))),
    (322.0, Some((Align::Right, 20.0))),
    (355.0, Some((Align::Centre, 20.0))),
    (385.0, None),
];
/// Where the list puts a player's abilities.
const ABILITY_COLUMNS: [f32; 3] = [180.0, 196.0, 212.0];
/// The list's colours: the line-up black, the bench blue, the rest of the
/// squad green.
const PLAYING_INK: [u8; 3] = [0, 0, 0];
const BENCH_INK: [u8; 3] = [10, 10, 250];
const STANDS_INK: [u8; 3] = [69, 147, 11];
/// The comparison with the next opponent: the cohesion, both clubs' names,
/// and goal, defence, midfield and attack, the total and the average form
/// for either.
const COHESION: u16 = 2556;
const OWN_NAME: u16 = 2547;
const OPPONENT_NAME: u16 = 2548;
const OWN_PARTS: [u16; PARTS] = [2549, 2550, 2551, 2552];
const OWN_TOTAL: u16 = 2555;
const OWN_FORM: u16 = 2553;
const OPPONENT_PARTS: [u16; PARTS] = [2561, 2562, 2563, 2564];
const OPPONENT_TOTAL: u16 = 2567;
const OPPONENT_FORM: u16 = 2565;
const HOME: u16 = 3664;
const AWAY: u16 = 3665;
/// The dressing room's options.
const DRESSING_OPTIONS: u16 = 4035;

/// Puts the main screens together: the bar along the bottom, the title in
/// the frame, and on the line-up the boxes where the first formation and the
/// bench put them.
pub fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    for id in DIALOGS {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        dialog.controls.retain(|c| !matches!(c.id, Some(OK_BUTTON | SECOND_TITLES)));
        dialog.controls.insert(
            0,
            Control {
                kind: Kind::Static,
                rect: CAPTION_RECT,
                align: Align::Left,
                text: String::new(),
                id: Some(CAPTION_ID),
                bold: true,
            },
        );
        if id == LINEUP {
            let spots = rules.pitch.formations.first().map_or([(0, 0); FIELD], |f| f.1);
            for control in &mut dialog.controls {
                let Some(id) = control.id else { continue };
                let at = if (FIELD_BOXES..FIELD_BOXES + FIELD as u16).contains(&id) {
                    let (x, y) = spots[usize::from(id - FIELD_BOXES)];
                    PITCH_ORIGIN + Vec2::new(f32::from(x), f32::from(y))
                } else if (BENCH_BOXES..BENCH_BOXES + BENCH as u16).contains(&id) {
                    BENCH_ORIGIN + Vec2::new(0.0, BENCH_STEP * f32::from(id - BENCH_BOXES))
                } else {
                    continue;
                };
                control.kind = Kind::Player;
                control.rect = Rect::from_corners(at, at + BOX_SIZE);
            }
        }
        let items = rules
            .bar_symbols
            .iter()
            .map(|&x| Item {
                rect: Rect::new(f32::from(x), 0.0, f32::from(x) + SYMBOL, BAR.height()),
                text: String::new(),
            })
            .collect();
        dialog.controls.push(Control {
            kind: Kind::Bar { items },
            rect: BAR,
            align: Align::Left,
            text: String::new(),
            id: Some(BAR_ID),
            bold: false,
        });
    }
}

/// The pitch in the line-up's picture, the panel that compares the teams
/// and the box of the cohesion beside the column titles.
const PITCH: Rect = Rect { min: Vec2::new(8.0, 22.0), max: Vec2::new(516.0, 265.0) };
const PITCH_PICTURE: u16 = 1010;
const COMPARISON_PANEL: Rect = Rect { min: Vec2::new(452.0, 300.0), max: Vec2::new(634.0, 372.0) };
const COHESION_BOX: Rect = Rect { min: Vec2::new(454.0, 268.0), max: Vec2::new(516.0, 299.0) };

/// What the remastered line-up draws behind its controls: the pitch from the
/// game's picture, and cards for the panels.
pub(super) fn decorations(id: u16) -> Vec<super::Decoration> {
    use super::{Decoration, Picture};
    if id != LINEUP {
        return Vec::new();
    }
    let size = PITCH.size();
    let part = [PITCH.min.x as u32, PITCH.min.y as u32, size.x as u32, size.y as u32];
    let pitch = Picture { part: Some(part), ..Picture::graphic(PITCH_PICTURE) };
    vec![
        Decoration::Picture { picture: pitch, at: PITCH, stretch: true },
        Decoration::Card(COMPARISON_PANEL),
        Decoration::Card(COHESION_BOX),
    ]
}

/// The screen a manager's turn opens on.
pub fn open(session: &mut Session) -> Action {
    let clubless =
        session.season.as_ref().and_then(Season::manager).is_none_or(|m| m.club.is_none());
    let screen = if clubless { DRESSING_ROOM } else { LINEUP };
    session.hub = screen;
    Action::Open(screen)
}

pub fn setup(view: &mut View, dialog: &Dialog, game: &Game, session: &mut Session) {
    session.hub = view.id;
    let Some(season) = &session.season else { return };
    let Some(manager) = season.manager() else { return };
    let club = manager.club.and_then(|(slot, c)| game.world.country(slot)?.clubs.get(c));
    let caption = game.text(if view.id == LINEUP { LINEUP_CAPTION } else { DRESSING_CAPTION });
    let date = season.date();
    let date = game
        .text(DATE)
        .replace("%1", &date.format("%-d").to_string())
        .replace("%2", &date.format("%-m").to_string())
        .replace("%3", &date.format("%Y").to_string());
    let who = format!("{caption} {} {}", manager.first_name, manager.last_name);
    let title = match club {
        Some(club) => format!("{who} ({}), {date}", club.name),
        None => format!("{who}, {date}"),
    };
    set(view, dialog, CAPTION_ID, title);

    if let Some(bar) = control(dialog, BAR_ID) {
        let state = &mut view.controls[bar];
        state.actions = true;
        state.chosen = if view.id == LINEUP { LINEUP_SYMBOL } else { DRESSING_SYMBOL };
        state.off = if manager.club.is_none() {
            game.rules.bar_without_club.clone()
        } else {
            vec![false; BAR_SYMBOLS]
        };
        if season.day % WEEK_DAYS == SATURDAY {
            SATURDAY_CLOSED.iter().for_each(|&s| state.off[s] = true);
        }
        if manager.national_team.is_none() {
            state.off[NATIONAL_SYMBOL] = true;
        }
        state.tips = game.rules.bar_tips.iter().map(|&t| game.text(t as u16).to_owned()).collect();
    }
    for id in [TITLES, SPECIAL, DUTIES, DRESSING_OPTIONS] {
        if let Some(i) = control(dialog, id) {
            view.controls[i].actions = true;
        }
    }
    if view.id == LINEUP
        && let Some((slot, index)) = manager.club
        && let Some(club) = game.world.country(slot).and_then(|c| c.clubs.get(index))
        && let Some(state) = season.clubs.get(&(slot, index))
    {
        show_lineup(view, dialog, game, season, &club.players, state);
    }
}

pub fn act(view: &mut View, dialog: &Dialog, session: &mut Session, target: Target) -> Action {
    let Target::Option { control: c, item } = target else { return Action::Stay };
    if dialog.controls[c].id != Some(BAR_ID) {
        if let Kind::Options { items, .. } = &dialog.controls[c].kind {
            let label = items.get(item).map_or("", |i| i.text.as_str()).replace("\\n", " ");
            info!("{label}: not built yet");
        }
        return Action::Stay;
    }
    match item {
        WHISTLE => {
            // Training is never set for the coming week, so the game asks.
            session.question = Some(Question::NoTraining);
            Action::Open(turn::QUESTION)
        }
        LINEUP_SYMBOL if view.id != LINEUP => Action::Open(LINEUP),
        DRESSING_SYMBOL if view.id != DRESSING_ROOM => Action::Open(DRESSING_ROOM),
        LINEUP_SYMBOL | DRESSING_SYMBOL => Action::Stay,
        _ => {
            let tip = view.controls[c].tips.get(item).cloned().unwrap_or_default();
            info!("{tip}: not built yet");
            Action::Stay
        }
    }
}

/// The line-up: the boxes on the pitch and the bench, the squad below, the
/// comparison with the next opponent.
fn show_lineup(
    view: &mut View,
    dialog: &Dialog,
    game: &Game,
    season: &Season,
    players: &[Player],
    state: &crate::season::ClubState,
) {
    let pitch = &game.rules.pitch;
    let lineup = &state.lineup;
    let fitness = |p: usize| lineup::fitness_of(&state.fitness, p);
    let abbreviation = |kind: u8| {
        let id = pitch.abbreviations.get(usize::from(kind)).copied().unwrap_or(0);
        game.text(id as u16).to_owned()
    };
    let name = |p: &Player| match p.first_name.chars().next() {
        Some(initial) => format!("{}, {initial}.", p.last_name),
        None => p.last_name.clone(),
    };
    let filled = |p: usize, position: u8| {
        let player = &players[p];
        PlayerBox {
            name: name(player),
            position: abbreviation(player.positions[0]),
            strength: tenths(pitch.strength_at(player, fitness(p), position)),
            fit: pitch.fit(player, position),
            icons: pitch.ability_icons(player),
            filled: true,
        }
    };
    for (i, (&spot, &player)) in lineup.spots.iter().zip(&lineup.field).enumerate() {
        let Some(c) = control(dialog, FIELD_BOXES + i as u16) else { continue };
        let position = pitch.position(spot);
        view.controls[c].player = Some(match player {
            Some(p) => filled(p, position),
            None => {
                let id = pitch.position_names.get(usize::from(position)).copied().unwrap_or(0);
                PlayerBox { name: game.text(id as u16).to_owned(), ..PlayerBox::default() }
            }
        });
    }
    for (i, &player) in lineup.bench.iter().enumerate() {
        let Some(c) = control(dialog, BENCH_BOXES + i as u16) else { continue };
        view.controls[c].player =
            Some(player.map_or_else(PlayerBox::default, |p| filled(p, players[p].positions[0])));
    }

    if let Some(list) = control(dialog, SQUAD) {
        let order: Vec<usize> = lineup
            .squad()
            .chain((0..players.len()).filter(|&p| !lineup.squad().any(|q| q == p)))
            .collect();
        let today = season.date();
        let state = &mut view.controls[list];
        state.tabs = COLUMNS.iter().map(|&(at, _)| at).collect();
        state.aligns = COLUMNS.iter().map(|&(_, align)| align).collect();
        state.rows.clear();
        state.inks.clear();
        state.row_icons.clear();
        for p in order {
            let player = &players[p];
            let [main, second, third] = player.positions;
            let sides = match (second, third) {
                (0, _) => " ".to_owned(),
                (s, 0) => abbreviation(s),
                (s, t) => format!("{}/{}", abbreviation(s), abbreviation(t)),
            };
            let form = lineup::form(fitness(p));
            state.rows.push(
                [
                    name(player),
                    abbreviation(main),
                    sides,
                    player.strength.to_string(),
                    form.to_string(),
                    " ".to_owned(),
                    lineup::FULL_MORALE.to_string(),
                    tenths(pitch.strength_at(player, fitness(p), main)),
                    player.shirt.to_string(),
                    player.age(today).to_string(),
                    String::new(),
                ]
                .join("\t"),
            );
            state.inks.push(Some(if lineup.playing(p) {
                PLAYING_INK
            } else if lineup.on_bench(p) {
                BENCH_INK
            } else {
                STANDS_INK
            }));
            state
                .row_icons
                .push(ABILITY_COLUMNS.into_iter().zip(pitch.ability_icons(player)).collect());
        }
    }
    show_comparison(view, dialog, game, season, players, state);
}

/// The panel that holds the line-up against the next opponent's, if the
/// club plays this week.
fn show_comparison(
    view: &mut View,
    dialog: &Dialog,
    game: &Game,
    season: &Season,
    players: &[Player],
    state: &crate::season::ClubState,
) {
    let pitch = &game.rules.pitch;
    let Some(own) = season.manager().and_then(|m| m.club) else { return };
    let club = |(slot, c): (usize, usize)| game.world.country(slot).and_then(|k| k.clubs.get(c));
    let own_name = club(own).map_or("", |c| c.name.as_str());
    set(view, dialog, COHESION, tenths(pitch.cohesion(&state.lineup, players)));
    let side = |view: &mut View,
                lineup: &Lineup,
                players: &[Player],
                fitness: &[_],
                ids: ([u16; PARTS], u16, u16)| {
        let parts = pitch.parts(lineup, players, fitness);
        for (&id, &part) in ids.0.iter().zip(&parts) {
            set(view, dialog, id, tenths(part));
        }
        set(view, dialog, ids.1, tenths(parts.iter().sum()));
        let forms: Vec<i32> =
            lineup.on_field().map(|(_, p)| lineup::form(lineup::fitness_of(fitness, p))).collect();
        let average = forms.iter().sum::<i32>() as f32 / forms.len().max(1) as f32;
        set(
            view,
            dialog,
            ids.2,
            if forms.is_empty() { String::new() } else { format!("{average:.1}") },
        );
    };
    side(view, &state.lineup, players, &state.fitness, (OWN_PARTS, OWN_TOTAL, OWN_FORM));

    let next = season
        .next_match(&game.world, &game.calendar, own)
        .filter(|m| m.day < season.day + WEEK_DAYS);
    let Some(m) = next else {
        set(view, dialog, OWN_NAME, own_name.to_owned());
        for id in OPPONENT_PARTS.into_iter().chain([OPPONENT_NAME, OPPONENT_TOTAL, OPPONENT_FORM]) {
            set(view, dialog, id, String::new());
        }
        return;
    };
    let home = m.home == own;
    let opponent = if home { m.away } else { m.home };
    let mark =
        |home: bool, name: &str| game.text(if home { HOME } else { AWAY }).replacen("%s", name, 1);
    set(view, dialog, OWN_NAME, mark(home, own_name));
    set(view, dialog, OPPONENT_NAME, mark(!home, club(opponent).map_or("", |c| c.name.as_str())));
    let theirs = club(opponent).map_or(&[][..], |c| &c.players[..]);
    let lineup = pitch.line_up(theirs, 0);
    side(view, &lineup, theirs, &[], (OPPONENT_PARTS, OPPONENT_TOTAL, OPPONENT_FORM));
}

/// Tenths as the game prints them: "7.3".
fn tenths(value: i32) -> String {
    format!("{:.1}", value as f32 / 10.0)
}

fn set(view: &mut View, dialog: &Dialog, id: u16, text: String) {
    if let Some(i) = control(dialog, id) {
        view.controls[i].text = text;
    }
}
