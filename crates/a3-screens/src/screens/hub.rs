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

use bevy_math::{Rect, Vec2};
use tracing::info;

use a3_core::lineup::{self, BENCH, FIELD, Lineup, PARTS, Place};
use a3_core::load;
use a3_core::records::Records;
use a3_core::season::{BAR_SYMBOLS, ClubState, Rules, Season, Status};
use a3_core::world::Player;
use a3_files::rch::{Align, Control, Dialog, Item, Kind};

use a3_core::cycle::dialogs::QUESTION;
pub use a3_core::cycle::dialogs::{DRESSING_ROOM, LINEUP};

use super::calendar::FIXTURE_AHEAD;
use super::formations::FORMATIONS;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, set};
use crate::controls::player_box::PlayerBox;
use crate::game::Game;
use crate::pointer::{Grip, Target};
use crate::view::View;

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
/// "%dT": days away from the club.
const ONLY_DAYS: u16 = 3609;
const NO_TRAINING: u16 = 3238;

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
/// The segment of the duties that opens the formations.
const FORMATION_ITEM: usize = 1;
/// The squad list's columns as the game sets them: where each starts, and
/// how the centred and right-aligned ones sit in their width.
const COLUMNS: [(f32, Option<(Align, f32)>); 11] = [
    (0.0, None),
    (105.0, Some((Align::Centre, 30.0))),
    (136.0, Some((Align::Centre, 40.0))),
    (225.0, Some((Align::Centre, 20.0))),
    (245.0, Some((Align::Right, 18.0))),
    (264.0, Some((Align::Left, 7.0))),
    (270.0, Some((Align::Right, 25.0))),
    (295.0, Some((Align::Right, 25.0))),
    (322.0, Some((Align::Right, 20.0))),
    (355.0, Some((Align::Centre, 20.0))),
    (385.0, None),
];
/// Where the list puts a player's abilities.
const ABILITY_COLUMNS: [f32; 3] = [180.0, 196.0, 212.0];
/// Where a row shows its player's status, and the graphics of the statuses
/// by the original's numbers, from 1 (`CMannschaftsaufstellung::FUN_0050a20c`;
/// the tips are texts 5175 to 5190).
const STATUS_COLUMN: f32 = 403.0;
const STATUS_ICONS: [u16; 16] = [
    1281, 1277, 1279, 1280, 1275, 1276, 1282, 1283, 1284, 1285, 1286, 2311, 2312, 2313, 1907, 1278,
];
/// The statuses: injured, red-carded — the original's choice when it does
/// not tell a yellow or a cup's ban from it, which needs the next match —
/// unfit to risk, suspended by the club, away, with the national team, the
/// dumbbell, knocked about, banned for doping, in prison, taken by a UFO.
const INJURED: u8 = 1;
const RED_CARD: u8 = 3;
const RISKY: u8 = 5;
const SUSPENDED: u8 = 6;
const ABSENT: u8 = 7;
const NATIONAL: u8 = 8;
const DUMBBELL: u8 = 10;
const KNOCKED: u8 = 11;
const DOPING: u8 = 12;
const PRISON: u8 = 13;
const UFO: u8 = 14;
/// The list's colours: the line-up black, the bench blue, the rest of the
/// squad green.
const PLAYING_INK: [u8; 3] = [0, 0, 0];
const BENCH_INK: [u8; 3] = [10, 10, 250];
const STANDS_INK: [u8; 3] = [69, 147, 11];
/// Out, or at the end of his contract.
const OUT_INK: [u8; 3] = [255, 0, 0];
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
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    for id in [LINEUP, DRESSING_ROOM] {
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
                ink: [0; 3],
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
            ink: [0; 3],
        });
    }
}

/// The line-up and the dressing room, with the menu bar; its symbols lead
/// to the other main screens, its whistle ends the turn.
pub struct MenuBar;

impl Screen for MenuBar {
    fn ids(&self) -> &[u16] {
        &[LINEUP, DRESSING_ROOM]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(season) = &cx.state.season else { return };
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

        let open = if view.id == LINEUP { LINEUP_SYMBOL } else { DRESSING_SYMBOL };
        if let Some(state) = control(dialog, BAR_ID).and_then(|b| view.bar_mut(b)) {
            state.chosen = open;
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
            state.tips =
                game.rules.bar_tips.iter().map(|&t| game.text(t as u16).to_owned()).collect();
        }
        for id in [TITLES, SPECIAL, DUTIES, DRESSING_OPTIONS] {
            if let Some(options) = control(dialog, id).and_then(|i| view.options_mut(i)) {
                options.actions = true;
            }
        }
        if view.id == LINEUP
            && let Some(club) = manager.club
            && let Some(state) = season.clubs.get(&club)
        {
            show_lineup(view, dialog, game, season, state, cx.state.records.as_ref());
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if let Target::Drop { from, to } = target {
            if view.id == LINEUP {
                drop_player(cx, view, from, to);
            }
            return Outcome::Stay;
        }
        let Target::Option { control: c, item } = target else { return Outcome::Stay };
        if view.id == LINEUP && cx.dialog.controls[c].id == Some(DUTIES) && item == FORMATION_ITEM {
            return Outcome::Popup(Request::new(FORMATIONS));
        }
        if cx.dialog.controls[c].id != Some(BAR_ID) {
            if let Kind::Options { items, .. } = &cx.dialog.controls[c].kind {
                let label = items.get(item).map_or("", |i| i.text.as_str()).replace("\\n", " ");
                info!("{label}: not built yet");
            }
            return Outcome::Stay;
        }
        match item {
            // Training is never set for the coming week, so the game asks.
            WHISTLE => Outcome::Popup(Request::with(QUESTION, Args::Text(NO_TRAINING))),
            LINEUP_SYMBOL if view.id != LINEUP => Outcome::Close(Reply::Goto(LINEUP)),
            DRESSING_SYMBOL if view.id != DRESSING_ROOM => {
                Outcome::Close(Reply::Goto(DRESSING_ROOM))
            }
            LINEUP_SYMBOL | DRESSING_SYMBOL => Outcome::Stay,
            _ => {
                let tip = view.bar(c).and_then(|b| b.tips.get(item)).cloned().unwrap_or_default();
                info!("{tip}: not built yet");
                Outcome::Stay
            }
        }
    }

    /// Yes to ending the turn without training set; a formation chosen.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        match reply {
            Reply::Ok => Outcome::Close(Reply::Ok),
            Reply::Choice(formation) => {
                stand_in(cx, view, formation);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

/// The line-up in another of the manager's formations: every player on the
/// spot of his place in it (`CSpielsystemeEditieren`'s OK).
fn stand_in(cx: &mut Cx, view: &mut View, formation: usize) {
    let (game, dialog) = (cx.game, cx.dialog);
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let Some(manager) = season.manager() else { return };
    let Some(club) = manager.club else { return };
    let Some(spots) = manager.formations.get(formation).map(|f| f.1) else { return };
    let Some(club_state) = season.clubs.get_mut(&club) else { return };
    club_state.formation = formation;
    club_state.lineup.spots = spots;
    let (season, records) = (state.season.as_ref().expect("the season"), state.records.as_ref());
    if let Some(club_state) = season.clubs.get(&club) {
        show_lineup(view, dialog, game, season, club_state, records);
    }
}

/// A player dragged on the line-up and let go of: moved as the game moves
/// him, and the line-up shown again.
fn drop_player(cx: &mut Cx, view: &mut View, from: Grip, to: Option<Grip>) {
    let (game, dialog) = (cx.game, cx.dialog);
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let Some(club) = season.manager().and_then(|m| m.club) else { return };
    let Some(club_state) = season.clubs.get_mut(&club) else { return };
    let order = list_order(&club_state.lineup, club_state.players.len());
    let place = |grip: Grip| match grip {
        Grip::Control(c) => {
            let id = dialog.controls.get(c)?.id?;
            if (FIELD_BOXES..FIELD_BOXES + FIELD as u16).contains(&id) {
                Some(Place::Field(usize::from(id - FIELD_BOXES)))
            } else if (BENCH_BOXES..BENCH_BOXES + BENCH as u16).contains(&id) {
                Some(Place::Bench(usize::from(id - BENCH_BOXES)))
            } else {
                None
            }
        }
        Grip::Row { control, row } => {
            (dialog.controls.get(control)?.id? == SQUAD).then_some(())?;
            let scroll = view.list(control).map_or(0, |l| l.scroll);
            order.get(scroll + row).map(|&p| Place::Player(p))
        }
    };
    let Some(from) = place(from) else { return };
    let to = to.and_then(place);
    if club_state.lineup.drop(from, to) {
        let (season, records) =
            (state.season.as_ref().expect("the season"), state.records.as_ref());
        if let Some(club_state) = season.clubs.get(&club) {
            show_lineup(view, dialog, game, season, club_state, records);
        }
    }
}

/// The squad as the list shows it: the eleven, the bench, then the rest.
fn list_order(lineup: &Lineup, players: usize) -> Vec<usize> {
    let squad: Vec<usize> = lineup.squad().collect();
    squad.iter().copied().chain((0..players).filter(|p| !squad.contains(p))).collect()
}

/// The line-up: the boxes on the pitch and the bench, the squad below, the
/// comparison with the next opponent.
fn show_lineup(
    view: &mut View,
    dialog: &Dialog,
    game: &Game,
    season: &Season,
    state: &ClubState,
    records: Option<&Records>,
) {
    let pitch = &game.rules.pitch;
    let players = &state.players;
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
            at: None,
        }
    };
    for (i, (&spot, &player)) in lineup.spots.iter().zip(&lineup.field).enumerate() {
        let Some(c) = control(dialog, FIELD_BOXES + i as u16) else { continue };
        let position = pitch.position(spot);
        let shown = match player {
            Some(p) => filled(p, position),
            None => {
                let id = pitch.position_names.get(usize::from(position)).copied().unwrap_or(0);
                PlayerBox { name: game.text(id as u16).to_owned(), ..PlayerBox::default() }
            }
        };
        // On the spot of the club's formation.
        let at = PITCH_ORIGIN + Vec2::new(f32::from(spot.0), f32::from(spot.1));
        view.set_player(c, PlayerBox { at: Some(at), ..shown });
    }
    for (i, &player) in lineup.bench.iter().enumerate() {
        let Some(c) = control(dialog, BENCH_BOXES + i as u16) else { continue };
        let shown = player.map_or_else(PlayerBox::default, |p| filled(p, players[p].positions[0]));
        view.set_player(c, shown);
    }

    let status = |p: usize| state.status.get(p).copied().unwrap_or_default();
    if let Some(state) = control(dialog, SQUAD).and_then(|l| view.list_mut(l)) {
        let order = list_order(lineup, players.len());
        let today = season.date();
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
                    trend(form, status(p).last_form).to_owned(),
                    fitness(p).morale.to_string(),
                    tenths(pitch.strength_at(player, fitness(p), main)),
                    player.shirt.to_string(),
                    player.age(today).to_string(),
                    out_for(&status(p), game),
                ]
                .join("\t"),
            );
            state.inks.push(Some(if out(&status(p)) {
                OUT_INK
            } else if lineup.playing(p) {
                PLAYING_INK
            } else if lineup.on_bench(p) {
                BENCH_INK
            } else {
                STANDS_INK
            }));
            let mut icons: Vec<(f32, u16)> =
                ABILITY_COLUMNS.into_iter().zip(pitch.ability_icons(player)).collect();
            if let Some(icon) = status_icon(&status(p)) {
                icons.push((STATUS_COLUMN, STATUS_ICONS[usize::from(icon - 1)]));
            }
            state.row_icons.push(icons);
        }
    }
    show_comparison(view, dialog, game, season, state, records);
}

/// A player's form against the one the list showed last.
fn trend(form: i32, last: Option<i32>) -> &'static str {
    match last {
        Some(last) if form < last => "-",
        Some(last) if form > last => "+",
        _ => " ",
    }
}

/// How long a player is out, as the list says it (`FUN_0050ada7`): the
/// first of a long absence in weeks, an injury of a week or more in weeks,
/// a suspension in matches — "?" while their length is not known — and
/// days away from the club.
fn out_for(status: &Status, game: &Game) -> String {
    const UNKNOWN: u8 = 0x80;
    const MATCHES: u8 = 0x3f;
    let days = WEEK_DAYS as i32;
    if status.long > 0 {
        if status.long_unknown { "?".into() } else { ((status.long + days - 1) / days).to_string() }
    } else if status.injured > 0 {
        if status.injured >= days { (status.injured / days).to_string() } else { String::new() }
    } else if status.suspended & MATCHES != 0 {
        if status.suspended & UNKNOWN != 0 {
            "?".into()
        } else {
            (status.suspended & MATCHES).to_string()
        }
    } else if status.away > 0 {
        game.text(ONLY_DAYS).replace("%d", &status.away.to_string())
    } else {
        String::new()
    }
}

/// The status a row shows, as `FUN_0050ada7` picks it: the last that
/// holds of knocked about, the dumbbell, an injury — or a suspension, or a
/// long absence — and the club's view: suspended by it, away, or at the end
/// of his contract.
fn status_icon(status: &Status) -> Option<u8> {
    const MARKED: u8 = 0x10;
    let mut icon = None;
    if status.knocked {
        icon = Some(KNOCKED);
    }
    if status.dumbbell {
        icon = Some(DUMBBELL);
    }
    if status.long > 0 {
        icon = Some(DOPING);
    } else if status.suspended != 0 {
        icon = Some(RED_CARD);
    } else if status.injured >= WEEK_DAYS as i32 {
        icon = Some(INJURED);
    } else if status.injured > 0 {
        icon = Some(RISKY);
    }
    let contracted = status.contract.is_none_or(|years| years > 0);
    if status.away == 0 && contracted {
        if status.away_kind & MARKED != 0 {
            icon = Some(SUSPENDED);
        }
    } else if status.away == 0 {
        icon = Some(ABSENT);
    } else {
        icon = Some(match status.away_kind & 0xf {
            2 => NATIONAL,
            5 => PRISON,
            7 => UFO,
            _ => ABSENT,
        });
    }
    icon
}

/// Whether the list writes a player in red: out for a while, suspended, or
/// at the end of his contract.
fn out(status: &Status) -> bool {
    status.long > 0
        || status.suspended != 0
        || status.injured >= WEEK_DAYS as i32
        || status.away > 0
        || status.contract.is_some_and(|years| years <= 0)
}

/// The panel that holds the line-up against the next opponent's, if the
/// club plays within the weeks the week screen looks ahead: in a loaded game
/// the opponent as the save has it, else the data files' squad in the first
/// formation.
fn show_comparison(
    view: &mut View,
    dialog: &Dialog,
    game: &Game,
    season: &Season,
    state: &ClubState,
    records: Option<&Records>,
) {
    let pitch = &game.rules.pitch;
    let players = &state.players;
    let Some(own) = season.manager().and_then(|m| m.club) else { return };
    let club = |(slot, c): (usize, usize)| game.world.country(slot).and_then(|k| k.clubs.get(c));
    let own_name = club(own).map_or("", |c| c.name.as_str());
    let cohesion = pitch.cohesion(state.cohesion_base, &state.lineup, players);
    set(view, dialog, COHESION, tenths(cohesion));
    // The own side as its line-up stands; the opponent's as the game last
    // reckoned it, where the save has it (`FUN_0050c9c0`).
    let side = |view: &mut View,
                lineup: &Lineup,
                players: &[Player],
                fitness: &[_],
                stored: Option<[i32; PARTS]>,
                ids: ([u16; PARTS], u16, u16)| {
        let parts = stored.unwrap_or_else(|| pitch.parts(lineup, players, fitness));
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
    let ids = (OWN_PARTS, OWN_TOTAL, OWN_FORM);
    side(view, &state.lineup, players, &state.fitness, None, ids);

    let next = season
        .next_match(&game.world, &game.calendar, own)
        .filter(|m| m.day < season.day + FIXTURE_AHEAD);
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
    let theirs = match records.and_then(|records| load::club(records, &game.data, opponent)) {
        Some(theirs) => theirs,
        None => {
            let players = club(opponent).map_or_else(Vec::new, |c| c.players.clone());
            let lineup = pitch.line_up(&players, 0);
            ClubState::new(players, lineup, Vec::new(), 0)
        }
    };
    let ids = (OPPONENT_PARTS, OPPONENT_TOTAL, OPPONENT_FORM);
    side(view, &theirs.lineup, &theirs.players, &theirs.fitness, theirs.strengths, ids);
}

/// Tenths as the game prints them: "7.3".
fn tenths(value: i32) -> String {
    format!("{:.1}", value as f32 / 10.0)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use a3_core::random::Random;
    use a3_core::season::NewManager;
    use a3_core::state::GameState;

    use super::*;
    use crate::classic::{self, Classic};
    use crate::controls::{ControlState, bar, player_box};
    use crate::screens::compose;

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_line_up_draws_without_a_window() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut game = Game::load(&dir).unwrap();
        compose(&mut game);
        let germany = game.world.country(0).unwrap();
        let bayern = germany.clubs.iter().position(|c| c.name == "FC Bayern München").unwrap();
        let manager = NewManager {
            first_name: "Anna".into(),
            last_name: "Test".into(),
            club: Some((0, bayern)),
            national_team: None,
            level: 9,
            difficulty: 2,
            negotiating: 3,
        };
        let season =
            Season::first(&game.world, &game.rules, vec![0], 0, vec![manager], Random::new(7));
        let mut state = GameState { season: Some(season), ..GameState::default() };
        let dialog = &game.dialogs[&LINEUP];
        let mut view = View::new(LINEUP, dialog);
        let mut cx = Cx { game: &game, dialog, state: &mut state };
        MenuBar.open(&mut cx, &mut view, Args::None);
        let background = classic::picture(&game, dialog);
        let classic = Classic::new(&game, dialog, &view, background.clone());
        let frame = classic.draw(dialog, &view, false, 0.0);
        assert_eq!(frame.dimensions(), (640, 480));

        // The bar: the line-up's symbol pressed, the national team's greyed
        // out — this manager has none — and the others as they stand.
        let bar_control = &dialog.controls[control(dialog, BAR_ID).unwrap()];
        for (item, image) in
            [(LINEUP_SYMBOL, bar::PRESSED), (1, bar::NORMAL), (NATIONAL_SYMBOL, bar::OFF)]
        {
            let strip = game.graphic(image).unwrap();
            let symbol = bar::symbol(bar_control, item);
            let (left, top) = (symbol.min.x as u32, symbol.min.y as u32);
            for y in 0..strip.height().min(symbol.height() as u32) {
                for x in 0..symbol.width() as u32 {
                    let shown = frame.get_pixel(left + x, top + y);
                    assert_eq!(shown, strip.get_pixel(left + x, y), "symbol {item} at {x}, {y}");
                }
            }
        }

        // A box for every spot of the formation and the bench: blue with a
        // player in it — between his abilities and his position it shows the
        // box as it is — or yellow above the name of a position the squad
        // has nobody for.
        let filled = game.graphic(player_box::FILLED).unwrap();
        let empty = game.graphic(player_box::EMPTY).unwrap();
        let boxes = (FIELD_BOXES..FIELD_BOXES + FIELD as u16)
            .chain(BENCH_BOXES..BENCH_BOXES + BENCH as u16);
        for id in boxes {
            let c = control(dialog, id).unwrap();
            let ControlState::Player(Some(player)) = &view.controls[c].state else {
                panic!("box {id} shows nothing");
            };
            let (picture, columns, rows) =
                if player.filled { (&filled, 46..56, 13..23) } else { (&empty, 0..99, 0..4) };
            let r = dialog.controls[c].rect;
            for (x, y) in columns.flat_map(|x| rows.clone().map(move |y| (x, y))) {
                let shown = frame.get_pixel(r.min.x as u32 + x, r.min.y as u32 + y);
                assert_eq!(shown, picture.get_pixel(x, y), "box {id} at {x}, {y}");
            }
        }

        // The whole squad in the list, the first row written in.
        let squad = control(dialog, SQUAD).unwrap();
        let players = germany.clubs[bayern].players.len();
        assert_eq!(view.list(squad).unwrap().rows.len(), players);
        let r = dialog.controls[squad].rect;
        let row = (r.min.y as u32 + 1)..(r.min.y as u32 + 14);
        let written = (r.min.x as u32..r.max.x as u32)
            .flat_map(|x| row.clone().map(move |y| (x, y)))
            .filter(|&(x, y)| frame.get_pixel(x, y) != background.get_pixel(x, y))
            .count();
        assert!(written > 100, "only {written} pixels of the first row are written");
    }
}
