//! A manager's turn on the game's main screens, the way it shows them by
//! default: with the menu bar, nineteen symbols along the bottom that lead
//! to every part of the game. The turn opens on the line-up; a manager
//! without a club starts in the dressing room, where the game's options are.
//!
//! The line-up shows the club's formation on the pitch, a box for every
//! spot, the bench beside it and the whole squad below, with what the team
//! counts for in goal, defence, midfield and attack. A manager may switch it
//! to a list (dialog 429), where a click takes a player in or out and a
//! small pitch shows the eleven. The whistle at the end of the bar ends the
//! turn.

use std::collections::HashMap;

use bevy_math::{Rect, Vec2};
use tracing::info;

use a3_core::lineup::{self, ALLROUNDER, BENCH, FIELD, Lineup, PARTS, Place};
use a3_core::load;
use a3_core::records::Records;
use a3_core::season::{BAR_SYMBOLS, ClubState, Rules, Season, Status};
use a3_core::squadlist::{self, Sort};
use a3_core::world::Player;
use a3_files::bitmap_font::to_cp1252;
use a3_files::rch::{Align, Control, Dialog, Item, Kind};

pub use a3_core::cycle::dialogs::{COMPARISON, DRESSING_ROOM, LINEUP, LINEUP_LIST, TACTICS};
use a3_core::cycle::dialogs::{NOTICE, QUESTION};

use super::calendar::FIXTURE_AHEAD;
use super::formations::FORMATIONS;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, set};
use super::{discipline, duties, mark_inert, medical, planner, playerinfo, special};
use crate::controls::player_box::PlayerBox;
use crate::controls::{ControlState, list};
use crate::game::Game;
use crate::pointer::{Grip, Target};
use crate::view::View;
use a3_core::autolineup;
use a3_core::mood::{self, LAST_REAL_ID};

/// The bar runs along the bottom of the screen; its symbols are as wide as
/// it is high.
const BAR: Rect = Rect { min: Vec2::new(0.0, 448.0), max: Vec2::new(640.0, 480.0) };
const SYMBOL: f32 = 32.0;
/// The bar's symbols by number: the line-up, the training the game keeps
/// shut on Saturdays, the national team, the dressing room and the whistle.
const LINEUP_SYMBOL: usize = 0;
pub(crate) const COMPARISON_SYMBOL: usize = 1;
pub(crate) const TACTICS_SYMBOL: usize = 2;
const SATURDAY_CLOSED: [usize; 2] = [3, 5];
const NATIONAL_SYMBOL: usize = 15;
const DRESSING_SYMBOL: usize = 17;
pub const WHISTLE: usize = 18;
/// The screens the bar's symbols lead to, as far as they are built.
const BAR_SCREENS: [(usize, u16); 4] = [
    (LINEUP_SYMBOL, LINEUP),
    (COMPARISON_SYMBOL, COMPARISON),
    (TACTICS_SYMBOL, TACTICS),
    (DRESSING_SYMBOL, DRESSING_ROOM),
];
/// What the game takes for Saturday.
const SATURDAY: usize = 6;
const WEEK_DAYS: usize = 7;
/// The screen's title, which the game draws into its frame.
const LINEUP_CAPTION: u16 = 2515;
const DRESSING_CAPTION: u16 = 2527;
const DATE: u16 = 1997;
/// "%dT": days away from the club.
const ONLY_DAYS: u16 = 3609;
const NO_TRAINING: u16 = 3238;

/// Ids for the controls the game makes itself.
const CAPTION_ID: u16 = 9920;
/// The menu of a player: "Spielerinfo", "Einzelgespräch", a blank line,
/// "Prämie", "Geldstrafe" (the commands 0, 2, 9, 3, 4 of the table at
/// 0x884a38, their texts at 0x8a4508). The talk is not offered on a week's
/// last day.
const MENU_ID: u16 = 9950;
const MENU: [(Command, u16); 5] = [
    (Command::Info, 4036),
    (Command::Talk, 4045),
    (Command::Blank, 4046),
    (Command::Bonus, 4066),
    (Command::Fine, 4067),
];
const WEEK: usize = 7;

/// What an entry of a player's menu does.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Command {
    Info,
    Talk,
    Blank,
    Bonus,
    Fine,
}
const BAR_ID: u16 = 9930;
/// What the bar takes the place of: the OK button.
const OK_BUTTON: u16 = 1;
/// The line-up's second row of column titles, the season's, which the game
/// puts where the first is and hides, with a picture of its own under it;
/// the button left of them that switches the rows (`0x5114ee`).
const SECOND_TITLES: u16 = 5515;
const SECOND_TITLES_PICTURE: u16 = 2259;
const ROWS_BUTTON: u16 = 2571;

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
/// The list view's small pitch: a marker for each slot of the eleven, at
/// a spot's x shrunk by 2.8 (the float at 0x81e40c) and its y, from this
/// corner; a disc for the kind of the spot (`s_tor01.bmp` … `s_ang01.bmp`);
/// and the line below it, how many players are in the eleven.
const MARKERS: u16 = 2510;
const MARKER_ORIGIN: (i32, i32) = (465, 30);
const MARKER_SHRINK: f32 = 2.8;
const KEEPER_MARKER: u16 = 2773;
const DEFENCE_MARKER: u16 = 2772;
const MIDFIELD_MARKER: u16 = 2771;
const ATTACK_MARKER: u16 = 2770;
const STATUS_LINE: u16 = 5277;
const PLAYERS_SET: [u16; 3] = [4686, 4687, 4688];
/// A marker's number: white where the player fits, blue on a side
/// position of his, green for an all-rounder, red anywhere else.
const FITS_INK: [u8; 3] = [255, 255, 255];
const SIDE_INK: [u8; 3] = [30, 110, 255];
const ALLROUNDER_INK: [u8; 3] = [0, 255, 0];
const MISFIT_INK: [u8; 3] = [255, 0, 0];
/// The hanging striker's position, which every player of the attacking
/// kinds fits (0x8a50ac lists them).
const HANGING_STRIKER: u8 = 17;
const SQUAD: u16 = 2509;
const TITLES: u16 = 5514;
const SPECIAL: u16 = 2572;
const DUTIES: u16 = 5266;
/// The line-up's two buttons: the special variants, and "Auto.".
const SPECIAL_ITEM: usize = 0;
/// No co-trainer to line the team up: text 3131.
const NO_CO_TRAINER: u16 = 3131;
/// The segments of the duties that open the formations, and who takes the
/// set pieces ("Aufgaben im Spiel").
const FORMATION_ITEM: usize = 1;
const TAKERS_ITEM: usize = 3;
/// The segment that keeps where the boxes stand in the formation, after
/// asking (text 5071, `FUN_0050da9b`); the question told apart by this
/// stand-in for its popup.
const KEEP_ITEM: usize = 2;
/// The segments that open the medical department, and what the game tells
/// of the next opponent.
const MEDICAL_ITEM: usize = 6;
/// The segment that switches between the pitch and the list.
const SWITCH_ITEM: usize = 0;
const OPPONENT_ITEM: usize = 7;
const KEEP_FORMATION: u16 = 5071;
const KEEP_QUESTION: u16 = u16::MAX - 1;
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
/// The season's row of columns (the tables at 0x884b08 and 0x884b2c):
/// name, position, nation, form this season, condition, freshness,
/// appearances, goals, a regular place; and where it shows how content a
/// player is — happy, content, all right, wanting to go (`0x89f918[57..]`).
const STATS_COLUMNS: [(f32, Option<(Align, f32)>); 9] = [
    (0.0, None),
    (105.0, Some((Align::Centre, 30.0))),
    (140.0, Some((Align::Centre, 30.0))),
    (180.0, Some((Align::Right, 30.0))),
    (225.0, Some((Align::Right, 20.0))),
    (253.0, Some((Align::Right, 25.0))),
    (285.0, Some((Align::Right, 20.0))),
    (320.0, Some((Align::Right, 20.0))),
    (385.0, Some((Align::Centre, 30.0))),
];
const CONTENT_COLUMN: f32 = 358.0;
const CONTENT_ICONS: [u16; 4] = [1807, 1808, 1806, 1809];
/// A regular place guaranteed in his contract, and "Ja" (text 1028).
const REGULAR_PLACE: u16 = 0x4000;
const YES: u16 = 1028;
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
/// The mark of a player suspended by the club, in his absence's kind.
const MARKED: u8 = 0x10;
/// A mark in a player's contract's terms the list view shows with icon 15.
const TERMS_MARK: u16 = 0x10;
const TERMS_ICON: u8 = 15;
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
/// "Spiel beenden" of the game options, and what it asks first
/// (`FUN_005b1286`); the question is told apart from the whistle's by this
/// stand-in for its popup.
const QUIT_ITEM: usize = 4;
const QUIT_REALLY: u16 = 2690;
const QUIT_QUESTION: u16 = u16::MAX;

/// Puts the main screens together: the bar along the bottom, the title in
/// the frame, and on the line-up the boxes where the first formation and the
/// bench put them.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    for id in [LINEUP, LINEUP_LIST, DRESSING_ROOM] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        dialog.controls.retain(|c| c.id != Some(OK_BUTTON));
        let first_titles = dialog.controls.iter().find(|c| c.id == Some(TITLES)).map(|c| c.rect);
        if let Some(rect) = first_titles
            && let Some(second) = dialog.controls.iter_mut().find(|c| c.id == Some(SECOND_TITLES))
        {
            second.rect = rect;
        }
        dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
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
        if id == LINEUP_LIST {
            // The small pitch's markers, which the screen moves and labels.
            for control in &mut dialog.controls {
                if control.id.is_some_and(|id| (MARKERS..MARKERS + FIELD as u16).contains(&id)) {
                    control.kind = Kind::Marker;
                }
            }
        }
        if line_up(id) {
            // The menu a right click on a player opens, over everything.
            let menu = Control {
                kind: Kind::Menu,
                rect: Rect::new(0.0, 0.0, 0.0, 0.0),
                align: Align::Left,
                text: String::new(),
                id: Some(MENU_ID),
                font: a3_files::rch::PLAIN_FONT,
                ink: [0; 3],
            };
            dialog.controls.push(menu);
        }
        add_bar(dialog, rules);
    }
}

/// The menu bar along the bottom of a main screen.
pub(crate) fn add_bar(dialog: &mut Dialog, rules: &Rules) {
    let items = rules
        .bar_symbols
        .iter()
        .map(|&x| Item {
            rect: Rect::new(f32::from(x), 0.0, f32::from(x) + SYMBOL, BAR.height()),
            text_at: Vec2::ZERO,
            text: String::new(),
        })
        .collect();
    dialog.controls.push(Control {
        kind: Kind::Bar { items },
        rect: BAR,
        align: Align::Left,
        text: String::new(),
        id: Some(BAR_ID),
        font: a3_files::rch::PLAIN_FONT,
        ink: [0; 3],
    });
}

/// The bar as a screen opens: its own symbol chosen, the symbols the
/// manager cannot use off — all but a few without a club, training on
/// Saturdays, the national team without one —, the tips.
pub(crate) fn open_bar(cx: &Cx, view: &mut View, own: usize) {
    let game = cx.game;
    let Some(season) = &cx.state.season else { return };
    let Some(manager) = season.manager() else { return };
    let Some(state) = control(cx.dialog, BAR_ID).and_then(|b| view.bar_mut(b)) else { return };
    state.chosen = own;
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

/// Whether a target is a symbol of the bar.
pub(crate) fn on_bar(dialog: &Dialog, target: Target) -> Option<usize> {
    match target {
        Target::Option { control: c, item } if dialog.controls[c].id == Some(BAR_ID) => Some(item),
        _ => None,
    }
}

/// Whether a symbol of the bar does something yet.
pub(crate) fn bar_works(item: usize) -> bool {
    item == WHISTLE || BAR_SCREENS.iter().any(|&(symbol, _)| symbol == item)
}

/// A symbol of the bar clicked: the screen it leads to, unless it is the
/// one shown; the whistle asks first, as no training is set for the coming
/// week, and a yes ends the turn ([`WHISTLE_ANSWERED`]).
pub(crate) fn bar_click(cx: &Cx, view: &View, item: usize, own: usize) -> Outcome {
    if item == WHISTLE {
        return Outcome::Popup(Request::with(QUESTION, Args::Text(NO_TRAINING)));
    }
    if item == own {
        return Outcome::Stay;
    }
    if let Some(&(_, screen)) = BAR_SCREENS.iter().find(|&&(symbol, _)| symbol == item) {
        return Outcome::Close(Reply::Goto(screen));
    }
    let bar = control(cx.dialog, BAR_ID).and_then(|b| view.bar(b));
    let tip = bar.and_then(|b| b.tips.get(item)).cloned().unwrap_or_default();
    info!("{tip}: not built yet");
    Outcome::Stay
}

/// The line-up and the dressing room, with the menu bar; its symbols lead
/// to the other main screens, its whistle ends the turn.
#[derive(Default)]
pub struct MenuBar {
    /// The popup the line-up opened, whose answer it waits for.
    popup: Option<u16>,
    /// The player whose menu is open, and its entries.
    menu_player: Option<usize>,
    menu: Vec<Command>,
    /// The column the squad list is sorted by, and the one each row of
    /// titles had when the other was switched to; each opening starts
    /// afresh.
    sort: Sort,
    kept: [usize; 2],
}

impl Screen for MenuBar {
    fn ids(&self) -> &[u16] {
        &[LINEUP, LINEUP_LIST, DRESSING_ROOM]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(season) = &cx.state.season else { return };
        let Some(manager) = season.manager() else { return };
        let caption = if line_up(view.id) { LINEUP_CAPTION } else { DRESSING_CAPTION };
        set(view, dialog, CAPTION_ID, manager_title(game, season, caption));

        open_bar(cx, view, own_symbol(view.id));
        for id in [SPECIAL, DUTIES, DRESSING_OPTIONS] {
            if let Some(options) = control(dialog, id).and_then(|i| view.options_mut(i)) {
                options.actions = true;
            }
        }
        // The list drops who may not play from the eleven and the bench
        // as it opens (`FUN_00502195`, `FUN_00696631`).
        let own = manager.club;
        if view.id == LINEUP_LIST
            && let Some(club) = own
            && let Some(state) = cx.state.season.as_mut().and_then(|s| s.clubs.get_mut(&club))
        {
            for p in 0..state.players.len() {
                if !autolineup::may_play(&state.status.get(p).copied().unwrap_or_default()) {
                    state.lineup.remove(p);
                }
            }
        }
        let Some(season) = &cx.state.season else { return };
        let Some(manager) = season.manager() else { return };
        if line_up(view.id)
            && let Some(club) = manager.club
            && let Some(state) = season.clubs.get(&club)
        {
            // Sorted as it opens, "Name" lit (`CMannschaftsaufstellung`'s
            // constructor, `CMannaufListe`'s).
            self.sort = Sort { list: view.id == LINEUP_LIST, ..Sort::default() };
            self.kept = [0; 2];
            if let Some(titles) = control(dialog, TITLES).and_then(|i| view.options_mut(i)) {
                titles.chosen = self.sort.column;
            }
            if let Some(second) = control(dialog, SECOND_TITLES) {
                view.controls[second].hidden = true;
                if let Some(titles) = view.options_mut(second) {
                    titles.background = Some(SECOND_TITLES_PICTURE);
                    titles.chosen = 0;
                }
            }
            let records = cx.state.records.as_ref();
            show_lineup(view, dialog, game, season, state, records, Boxes::Refill, self.sort);
        }
        // Crossed out: what does nothing yet.
        let works = |target: Target| {
            if let Target::Button(c) = target {
                return dialog.controls[c].id == Some(ROWS_BUTTON);
            }
            let Target::Option { control: c, item } = target else { return false };
            match dialog.controls[c].id {
                Some(BAR_ID) => bar_works(item),
                Some(SPECIAL) => true,
                Some(TITLES) => item < squadlist::COLUMNS,
                Some(SECOND_TITLES) => item < squadlist::STATS_COLUMNS,
                Some(DUTIES) => {
                    matches!(
                        item,
                        SWITCH_ITEM
                            | FORMATION_ITEM
                            | KEEP_ITEM
                            | TAKERS_ITEM
                            | MEDICAL_ITEM
                            | OPPONENT_ITEM
                    )
                }
                Some(DRESSING_OPTIONS) => item == QUIT_ITEM,
                _ => false,
            }
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        // An open menu takes the next click: on an entry, or anywhere else
        // to close it.
        if let Some(player) = self.menu_player.take() {
            let menu = control(cx.dialog, MENU_ID);
            if let Some(ControlState::Menu(m)) = menu.map(|m| &mut view.controls[m].state) {
                m.at = None;
            }
            if let Target::Option { control: c, item } = target
                && Some(c) == menu
            {
                match self.menu.get(item) {
                    Some(Command::Info) => {
                        // The list's info pages through its rows as the
                        // list box keeps them: backwards (`AddHead`).
                        if view.id == LINEUP_LIST {
                            let rows = control(cx.dialog, SQUAD).and_then(|l| view.list(l));
                            let rows = rows.map(|l| l.keys.iter().rev().copied().collect());
                            cx.state.paging = rows;
                        }
                        self.popup = Some(playerinfo::CURRENT);
                        return Outcome::Popup(Request::with(
                            playerinfo::CURRENT,
                            Args::Player(player),
                        ));
                    }
                    Some(Command::Bonus) => {
                        self.popup = Some(discipline::BONUS);
                        return Outcome::Popup(Request::with(
                            discipline::BONUS,
                            Args::Player(player),
                        ));
                    }
                    // A talk is booked in the planner; not with a manager
                    // who plays.
                    Some(Command::Talk) => {
                        let id = club_state(cx).and_then(|c| c.status.get(player)).map(|s| s.id);
                        if let Some(id) = id.filter(|&id| id != 0 && id <= LAST_REAL_ID) {
                            self.popup = Some(planner::PLANNER);
                            return Outcome::Popup(Request::with(
                                planner::PLANNER,
                                Args::Record(id),
                            ));
                        }
                    }
                    Some(Command::Fine) => {
                        self.popup = Some(discipline::FINE);
                        return Outcome::Popup(Request::with(
                            discipline::FINE,
                            Args::Player(player),
                        ));
                    }
                    Some(command) => info!("{command:?} ({player}): not built yet"),
                    None => {}
                }
            }
            return Outcome::Stay;
        }
        // The "i" button switches to the other row, also on a right click,
        // which steps it back.
        if let Target::Menu(Grip::Control(c)) | Target::Button(c) = target
            && line_up(view.id)
            && cx.dialog.controls[c].id == Some(ROWS_BUTTON)
        {
            self.switch_rows(cx, view, c);
            return Outcome::Stay;
        }
        // The list box's own menu has no talk (`CListBoxSpielerInfo`).
        if let Target::Menu(grip) = target {
            if line_up(view.id) {
                let day = cx.state.season.as_ref().map_or(0, |s| s.day);
                let talk = view.id == LINEUP && day % WEEK != WEEK - 1;
                self.menu = MENU
                    .iter()
                    .map(|&(command, _)| command)
                    .filter(|&c| c != Command::Talk || talk)
                    .collect();
                self.menu_player = open_menu(cx, view, grip, &self.menu);
            }
            return Outcome::Stay;
        }
        if let Target::Drop { from, to, .. } = target {
            if view.id == LINEUP {
                drop_player(cx, view, from, to, self.sort);
            }
            return Outcome::Stay;
        }
        // A click on a row of the list view takes its player in or out
        // (`OnSelChange`).
        if let Target::Row { control: c, row } = target
            && view.id == LINEUP_LIST
            && cx.dialog.controls[c].id == Some(SQUAD)
        {
            let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
            if let Some(player) = player {
                let sort = self.sort;
                change_lineup(cx, view, Boxes::Refill, sort, |tables, club, _| {
                    a3_core::refit::click(tables, club, player);
                });
            }
            return Outcome::Stay;
        }
        let Target::Option { control: c, item } = target else { return Outcome::Stay };
        // The other view: the manager keeps to it, and the turn opens it at
        // once (`EndDialog(100)`, `FUN_00650b84`). The pitch view's parts
        // stay with the club, as the list shows them.
        if line_up(view.id) && cx.dialog.controls[c].id == Some(DUTIES) && item == SWITCH_ITEM {
            switch_view(cx, view.id == LINEUP);
            return Outcome::Close(Reply::Goto(LINEUP));
        }
        if line_up(view.id) && cx.dialog.controls[c].id == Some(DUTIES) && item == FORMATION_ITEM {
            self.popup = Some(FORMATIONS);
            return Outcome::Popup(Request::new(FORMATIONS));
        }
        if line_up(view.id) && cx.dialog.controls[c].id == Some(DUTIES) && item == KEEP_ITEM {
            let formation = club_state(cx).map_or(0, |c| c.formation);
            self.popup = Some(KEEP_QUESTION);
            let args = if view.id == LINEUP_LIST {
                Args::KeepFormation(formation)
            } else {
                Args::TextForFormation(KEEP_FORMATION, formation)
            };
            return Outcome::Popup(Request::with(QUESTION, args));
        }
        // The next opponent's info, if there is one (`FUN_00510540`).
        if line_up(view.id) && cx.dialog.controls[c].id == Some(DUTIES) && item == OPPONENT_ITEM {
            let Some((dialog, opponent)) = next_opponent(cx) else { return Outcome::Stay };
            self.popup = Some(dialog);
            return Outcome::Popup(Request::with(dialog, Args::Club(opponent.0, opponent.1)));
        }
        if line_up(view.id) && cx.dialog.controls[c].id == Some(DUTIES) && item == MEDICAL_ITEM {
            self.popup = Some(medical::MEDICAL);
            return Outcome::Popup(Request::new(medical::MEDICAL));
        }
        if line_up(view.id) && cx.dialog.controls[c].id == Some(DUTIES) && item == TAKERS_ITEM {
            self.popup = Some(duties::TAKERS);
            return Outcome::Popup(Request::new(duties::TAKERS));
        }
        if line_up(view.id) && cx.dialog.controls[c].id == Some(SPECIAL) {
            if item == SPECIAL_ITEM {
                self.popup = Some(special::SPECIAL);
                return Outcome::Popup(Request::new(special::SPECIAL));
            }
            change_lineup(cx, view, Boxes::Refill, self.sort, |tables, club, _| {
                autolineup::bench(tables, club);
            });
            return Outcome::Stay;
        }
        // A column title sorts the list by its column (`FUN_0050fd07`);
        // nothing else changes.
        if line_up(view.id) && matches!(cx.dialog.controls[c].id, Some(TITLES | SECOND_TITLES)) {
            self.sort.click(item);
            change_lineup(cx, view, Boxes::Keep, self.sort, |_, _, _| {});
            return Outcome::Stay;
        }
        if view.id == DRESSING_ROOM
            && cx.dialog.controls[c].id == Some(DRESSING_OPTIONS)
            && item == QUIT_ITEM
        {
            self.popup = Some(QUIT_QUESTION);
            return Outcome::Popup(Request::with(QUESTION, Args::Text(QUIT_REALLY)));
        }
        if cx.dialog.controls[c].id != Some(BAR_ID) {
            if let Kind::Options { items, .. } = &cx.dialog.controls[c].kind {
                let label = items.get(item).map_or("", |i| i.text.as_str()).replace("\\n", " ");
                info!("{label}: not built yet");
            }
            return Outcome::Stay;
        }
        bar_click(cx, view, item, own_symbol(view.id))
    }

    /// Yes to ending the turn without training set; a formation chosen.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        match (self.popup.take(), reply) {
            (Some(FORMATIONS), Reply::Choice(formation)) => {
                stand_in(cx, view, formation, self.sort);
                Outcome::Stay
            }
            (Some(special::SPECIAL), Reply::Choice(variant)) => {
                let has_co_trainer = club_state(cx).is_some_and(|c| c.co_trainer);
                if autolineup::CO_TRAINER.contains(&variant) && !has_co_trainer {
                    // Its OK only takes the notice in.
                    self.popup = Some(NOTICE);
                    return Outcome::Popup(Request::with(NOTICE, Args::Text(NO_CO_TRAINER)));
                }
                let mut put_up = true;
                change_lineup(cx, view, Boxes::Refill, self.sort, |tables, club, today| {
                    put_up = autolineup::variant(tables, club, variant, today);
                });
                if !put_up {
                    info!("variant {variant}: not built yet");
                }
                Outcome::Stay
            }
            // The player info goes on to another of its pages.
            (Some(page), Reply::Goto(next))
                if playerinfo::PAGES_IN_ORDER.contains(&page)
                    && playerinfo::PAGES_IN_ORDER.contains(&next) =>
            {
                self.popup = Some(next);
                Outcome::Popup(Request::new(next))
            }
            // The list box's menu fills nothing again.
            (Some(discipline::BONUS | discipline::FINE), _) if view.id == LINEUP_LIST => {
                Outcome::Stay
            }
            // A bonus or a fine may have changed the players' morale; the
            // takers their duties, a deposed captain his morale. The boxes
            // keep what they show but for the duties.
            (Some(discipline::BONUS | discipline::FINE | duties::TAKERS | medical::MEDICAL), _) => {
                change_lineup(cx, view, Boxes::Keep, self.sort, |_, _, _| {});
                Outcome::Stay
            }
            (Some(QUIT_QUESTION), Reply::Ok) => Outcome::Close(Reply::Quit),
            (Some(KEEP_QUESTION), Reply::Ok) => {
                keep_formation(cx);
                Outcome::Stay
            }
            (None, Reply::Ok) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

impl MenuBar {
    /// The other row of column titles, the list sorted as it was there,
    /// the button showing its other picture; nothing but the list changes.
    fn switch_rows(&mut self, cx: &mut Cx, view: &mut View, button: usize) {
        let dialog = cx.dialog;
        self.kept[usize::from(self.sort.stats)] = self.sort.column;
        self.sort.stats = !self.sort.stats;
        self.sort.column = self.kept[usize::from(self.sort.stats)];
        for (id, shown) in [(TITLES, !self.sort.stats), (SECOND_TITLES, self.sort.stats)] {
            let Some(c) = control(dialog, id) else { continue };
            view.controls[c].hidden = !shown;
            if let Some(titles) = view.options_mut(c) {
                titles.chosen = self.sort.column;
            }
        }
        if let Kind::States { pictures } = &dialog.controls[button].kind
            && let Some(&picture) = pictures.get(usize::from(self.sort.stats))
            && let ControlState::Button(state) = &mut view.controls[button].state
        {
            state.images = Some((picture, picture));
            state.ready.clone_from(pictures);
        }
        change_lineup(cx, view, Boxes::Keep, self.sort, |_, _, _| {});
    }
}

/// A title as the turn's screens write it in their frame (`FUN_00730fed`):
/// the screen's, the manager's name, his club and the date.
pub(crate) fn manager_title(game: &Game, season: &Season, caption: u16) -> String {
    let Some(manager) = season.manager() else { return String::new() };
    let club = manager.club.and_then(|(slot, c)| game.world.country(slot)?.clubs.get(c));
    let date = season.date();
    let date = game
        .text(DATE)
        .replace("%1", &date.format("%-d").to_string())
        .replace("%2", &date.format("%-m").to_string())
        .replace("%3", &date.format("%Y").to_string());
    let who = format!("{} {} {}", game.text(caption), manager.first_name, manager.last_name);
    match club {
        Some(club) => format!("{who} ({}), {date}", club.name),
        None => format!("{who}, {date}"),
    }
}

/// A title as the popups of the turn write it (text 1431, `FUN_00731086`):
/// the popup's, the manager's name and his club; its last "%s" the
/// original fills from a buffer nothing writes.
pub(crate) fn club_title(game: &Game, season: &Season, kind: u16) -> String {
    let Some(manager) = season.manager() else { return String::new() };
    let club = manager.club.and_then(|(slot, c)| game.world.country(slot)?.clubs.get(c));
    game.text(CLUB_TITLE)
        .replacen("%s", game.text(kind), 1)
        .replacen("%s", &manager.first_name, 1)
        .replacen("%s", &manager.last_name, 1)
        .replacen("%s", club.map_or("", |c| c.name.as_str()), 1)
        .replacen("%s", "", 1)
        .trim_end()
        .to_owned()
}
const CLUB_TITLE: u16 = 1431;

/// The club the line-up compares with, and which of the opponent's dialogs
/// the game shows of it.
fn next_opponent(cx: &Cx) -> Option<(u16, (usize, usize))> {
    let (game, season) = (cx.game, cx.state.season.as_ref()?);
    let own = season.manager()?.club?;
    let next = season
        .next_match(&game.world, &game.calendar, own)
        .filter(|m| m.day < season.day + FIXTURE_AHEAD)?;
    let opponent = if next.home == own { next.away } else { next.home };
    let records = cx.state.records.as_ref();
    let nation = game.world.country(opponent.0)?.nation;
    let team = records.and_then(|r| r.team(nation, u16::try_from(opponent.1).ok()?));
    Some((super::opponent::dialog_for(records, team), opponent))
}

/// The current manager's club as the season follows it.
fn club_state<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

/// The bar's symbol of a screen with the bar.
fn own_symbol(id: u16) -> usize {
    if line_up(id) { LINEUP_SYMBOL } else { DRESSING_SYMBOL }
}

/// Whether a screen is the line-up, on the pitch or as a list.
fn line_up(id: u16) -> bool {
    matches!(id, LINEUP | LINEUP_LIST)
}

/// The current manager's line-up switched to the list or back to the
/// pitch, with him and in his record. Leaving the pitch, the club keeps the
/// parts the pitch view reckoned last (club +0x94), which the list shows.
fn switch_view(cx: &mut Cx, list: bool) {
    let pitch = &cx.game.rules.pitch;
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let current = season.current;
    let Some(manager) = season.managers.get_mut(current) else { return };
    manager.list_view = list;
    if list
        && let Some(club) = manager.club
        && let Some(club) = season.clubs.get_mut(&club)
    {
        club.strengths = Some(pitch.parts(&club.lineup, &club.players, &club.fitness));
    }
    if let Some(records) = state.records.as_mut()
        && let Some(slot) = records.human_slot(current)
        && let Some(record) = records.managers.get_mut(slot)
    {
        record.set_list_view(list);
    }
}

/// Changes the current manager's line-up and shows it again.
fn change_lineup(
    cx: &mut Cx,
    view: &mut View,
    boxes: Boxes,
    sort: Sort,
    change: impl FnOnce(&lineup::Tables, &mut ClubState, chrono::NaiveDate),
) {
    let (game, dialog) = (cx.game, cx.dialog);
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let today = season.date();
    let Some(club) = season.manager().and_then(|m| m.club) else { return };
    let Some(club_state) = season.clubs.get_mut(&club) else { return };
    change(&game.rules.pitch, club_state, today);
    let (season, records) = (state.season.as_ref().expect("the season"), state.records.as_ref());
    if let Some(club_state) = season.clubs.get(&club) {
        show_lineup(view, dialog, game, season, club_state, records, boxes, sort);
    }
}

/// The line-up in another of the manager's formations: every player on the
/// spot of his place in it (`CSpielsystemeEditieren`'s OK).
fn stand_in(cx: &mut Cx, view: &mut View, formation: usize, sort: Sort) {
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
        show_lineup(view, dialog, game, season, club_state, records, Boxes::Refill, sort);
    }
}

/// The formation the line-up stands in takes the spots its boxes stand on,
/// with the manager and in his record.
fn keep_formation(cx: &mut Cx) {
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let current = season.current;
    let Some(key) = season.manager().and_then(|m| m.club) else { return };
    let Some(club) = season.clubs.get(&key) else { return };
    let (formation, spots) = (club.formation, club.lineup.spots);
    let Some(manager) = season.managers.get_mut(current) else { return };
    let Some(kept) = manager.formations.get_mut(formation) else { return };
    kept.1 = spots;
    if let Some(records) = state.records.as_mut()
        && let Some(slot) = records.human_slot(current)
        && let Some(record) = records.managers.get_mut(slot)
    {
        record.set_formation_spots(formation, spots);
    }
}

/// Opens the menu of the player a right click fell on, there; the player.
fn open_menu(cx: &mut Cx, view: &mut View, grip: Grip, commands: &[Command]) -> Option<usize> {
    let club = club_state(cx)?;
    let player = match place_of(cx.dialog, view, grip)? {
        Place::Field(i) => club.lineup.field[i]?,
        Place::Bench(i) => club.lineup.bench[i]?,
        Place::Player(p) => p,
    };
    let target = match grip {
        Grip::Control(c) => Target::Control(c),
        Grip::Row { control, row } => Target::Row { control, row },
    };
    let (_, rect) = crate::pointer::targets(cx.dialog, view, &crate::pointer::Pixels)
        .into_iter()
        .find(|(t, _)| *t == target)?;
    let text = |c: &Command| MENU.iter().find(|(m, _)| m == c).map_or(0, |&(_, t)| t);
    let entries = commands.iter().map(|c| cx.game.text(text(c)).to_owned()).collect();
    let menu = control(cx.dialog, MENU_ID)?;
    if let ControlState::Menu(m) = &mut view.controls[menu].state {
        m.at = Some(rect.center());
        m.entries = entries;
    }
    Some(player)
}

/// Where on the line-up a grip is: a spot, a place on the bench, or a
/// player of the list.
fn place_of(dialog: &Dialog, view: &View, grip: Grip) -> Option<Place> {
    match grip {
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
            // The row knows its player (`FUN_0069318b`).
            let list = view.list(control)?;
            list.keys.get(list.scroll + row).map(|&p| Place::Player(p))
        }
    }
}

/// A player dragged on the line-up and let go of: moved as the game moves
/// him, and the line-up shown again.
fn drop_player(cx: &mut Cx, view: &mut View, from: Grip, to: Option<Grip>, sort: Sort) {
    let (game, dialog) = (cx.game, cx.dialog);
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let Some(club) = season.manager().and_then(|m| m.club) else { return };
    let Some(club_state) = season.clubs.get_mut(&club) else { return };
    let place = |grip: Grip| place_of(dialog, view, grip);
    let Some(from) = place(from) else { return };
    let to = to.and_then(place);
    if club_state.lineup.drop(from, to) {
        let (season, records) =
            (state.season.as_ref().expect("the season"), state.records.as_ref());
        if let Some(club_state) = season.clubs.get(&club) {
            show_lineup(view, dialog, game, season, club_state, records, Boxes::Refill, sort);
        }
    }
}

/// The squad as the list shows it, sorted by its column.
fn list_order(
    game: &Game,
    season: &Season,
    club: &ClubState,
    seasons: &[squadlist::Season],
    records: Option<&Records>,
    sort: Sort,
) -> Vec<usize> {
    let names: Vec<squadlist::Name> = club
        .players
        .iter()
        .enumerate()
        .map(|(p, player)| {
            let record = club.status.get(p).and_then(|s| records?.player(s.id));
            squadlist::Name {
                nicknamed: record.is_some_and(|r| r.shown() & NICKNAME != 0),
                nickname: record.map_or_else(Vec::new, |r| {
                    records.map_or_else(Vec::new, |records| bytes(records.name(r.nickname())))
                }),
                last: bytes(&player.last_name),
                first: bytes(&player.first_name),
            }
        })
        .collect();
    squadlist::order(&game.rules.pitch, club, &names, seasons, season.date(), sort)
}

fn bytes(text: &str) -> Vec<u8> {
    text.chars().map(|c| to_cp1252(c).unwrap_or(b'?')).collect()
}

/// What the season's row of the list shows of each player, from his record
/// (`FUN_0050ada7`'s second row): his nation's letters, his form this
/// season, his league appearances and goals, how content he is. A player of
/// a new game has no season yet.
fn seasons(game: &Game, club: &ClubState, records: Option<&Records>) -> Vec<squadlist::Season> {
    let code = |nation: u8| game.world.nation_codes.get(usize::from(nation)).cloned();
    (0..club.players.len())
        .map(|p| {
            let status = club.status.get(p).copied().unwrap_or_default();
            let nation = bytes(&code(status.nations[0]).unwrap_or_default());
            let nation_name = bytes(game.world.nation(status.nations[0].into()));
            let Some(r) = records.and_then(|records| records.player(status.id)) else {
                return squadlist::Season {
                    nation,
                    nation_name,
                    form: NO_FORM,
                    content: 1,
                    ..Default::default()
                };
            };
            let weeks = i32::from(r.season_form_weeks());
            let form = if weeks == 0 { NO_FORM } else { i32::from(r.season_form()) * 10 / weeks };
            // A manager who plays is content (`FUN_00770877`).
            let content =
                if status.id > LAST_REAL_ID { 1 } else { mood::content_level(mood::parts(r)) };
            squadlist::Season {
                nation,
                nation_name,
                form,
                appearances: i32::from(r.league_matches()),
                goals: i32::from(r.goals()[0]),
                content: content as i32,
            }
        })
        .collect()
}

/// The season's form of a player without weeks, in tenths.
const NO_FORM: i32 = 100;

/// Bits of a record's `shown`: he goes by his nickname.
const NICKNAME: u32 = 0x8000_0000;

/// Whether the boxes are filled anew, or keep what they show — but for the
/// duties — as the original's do where nobody moved (it refills only the
/// boxes it marked, `FUN_0050bbc8`: all as it opens and after "Spz.", the
/// bench after "Auto.", those moved or of a new formation).
#[derive(Clone, Copy, PartialEq)]
enum Boxes {
    Refill,
    Keep,
}

/// The line-up: the boxes on the pitch and the bench, the squad below, the
/// comparison with the next opponent.
#[allow(clippy::too_many_arguments)]
fn show_lineup(
    view: &mut View,
    dialog: &Dialog,
    game: &Game,
    season: &Season,
    state: &ClubState,
    records: Option<&Records>,
    boxes: Boxes,
    sort: Sort,
) {
    let pitch = &game.rules.pitch;
    let players = &state.players;
    let lineup = &state.lineup;
    let fitness = |p: usize| lineup::fitness_of(&state.fitness, p);
    let filled = |p: usize, position: u8| filled_box(game, &players[p], fitness(p), position);
    let abbreviation = |kind: u8| abbreviation(game, kind);
    let list_view = view.id == LINEUP_LIST;
    if list_view {
        show_markers(view, dialog, game, state);
    }
    let duties = a3_core::duties::on_pitch(players, &state.duties, &lineup.field);
    let spots = if list_view { &[][..] } else { &lineup.spots[..] };
    for (i, (&spot, &player)) in spots.iter().zip(&lineup.field).enumerate() {
        let Some(c) = control(dialog, FIELD_BOXES + i as u16) else { continue };
        let position = pitch.position(spot);
        let shown = match player {
            Some(p) => PlayerBox {
                duty: duties[i].and_then(|d| pitch.duty_icon(d)),
                ..filled(p, position)
            },
            None => {
                let id = pitch.position_names.get(usize::from(position)).copied().unwrap_or(0);
                PlayerBox { name: game.text(id as u16).to_owned(), ..PlayerBox::default() }
            }
        };
        // On the spot of the club's formation.
        let at = PITCH_ORIGIN + Vec2::new(f32::from(spot.0), f32::from(spot.1));
        let shown = match view.controls[c].state.as_player() {
            Some(old) if boxes == Boxes::Keep => PlayerBox { duty: shown.duty, ..old.clone() },
            _ => PlayerBox { at: Some(at), ..shown },
        };
        view.set_player(c, shown);
    }
    // The bench shows no duties.
    let bench = if boxes == Boxes::Refill && !list_view { &lineup.bench[..] } else { &[] };
    for (i, &player) in bench.iter().enumerate() {
        let Some(c) = control(dialog, BENCH_BOXES + i as u16) else { continue };
        let shown = player.map_or_else(PlayerBox::default, |p| filled(p, players[p].positions[0]));
        view.set_player(c, shown);
    }

    let status = |p: usize| state.status.get(p).copied().unwrap_or_default();
    let seasons = seasons(game, state, records);
    let order = list_order(game, season, state, &seasons, records, sort);
    let visible = control(dialog, SQUAD).map_or(0, |l| list::visible_rows(&dialog.controls[l]));
    if let Some(state) = control(dialog, SQUAD).and_then(|l| view.list_mut(l)) {
        // The top row stays as far as the rows reach; no row stays selected.
        state.scroll = state.scroll.min(order.len().saturating_sub(visible));
        state.selected = None;
        state.keys.clone_from(&order);
        let today = season.date();
        let columns: &[_] = if sort.stats { &STATS_COLUMNS } else { &COLUMNS };
        state.tabs = columns.iter().map(|&(at, _)| at).collect();
        state.aligns = columns.iter().map(|&(_, align)| align).collect();
        state.rows.clear();
        state.inks.clear();
        state.row_icons.clear();
        state.keyed = STATUS_ICONS.into_iter().chain(CONTENT_ICONS).collect();
        state.row_boxes.clear();
        for p in order {
            // The list view's rows are not dragged.
            state.row_boxes.push((!list_view).then(|| filled(p, players[p].positions[0])));
            let player = &players[p];
            let [main, second, third] = player.positions;
            let sides = match (second, third) {
                (0, _) => " ".to_owned(),
                (s, 0) => abbreviation(s),
                (s, t) => format!("{}/{}", abbreviation(s), abbreviation(t)),
            };
            let form = lineup::form(fitness(p));
            let terms = records.and_then(|r| r.player(status(p).id)).map_or(0, |r| r.terms());
            let ink = if out(&status(p), list_view) {
                OUT_INK
            } else if lineup.playing(p) {
                PLAYING_INK
            } else if lineup.on_bench(p) {
                BENCH_INK
            } else {
                STANDS_INK
            };
            if sort.stats {
                let season = &seasons[p];
                let regular = records
                    .and_then(|r| r.player(status(p).id))
                    .is_some_and(|r| r.terms() & REGULAR_PLACE != 0);
                state.rows.push(
                    [
                        name(player),
                        abbreviation(main),
                        a3_files::sav::cp1252(&season.nation),
                        format!("{:.1}", season.form as f32 / 10.0),
                        lineup::condition(fitness(p)).to_string(),
                        lineup::freshness(fitness(p)).to_string(),
                        season.appearances.to_string(),
                        season.goals.to_string(),
                        if regular { game.text(YES).to_owned() } else { String::new() },
                    ]
                    .join("\t"),
                );
                state.inks.push(Some(ink));
                let icon = CONTENT_ICONS.get(season.content as usize).copied();
                state.row_icons.push(icon.map(|i| (CONTENT_COLUMN, i)).into_iter().collect());
                continue;
            }
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
            state.inks.push(Some(ink));
            let mut icons: Vec<(f32, u16)> =
                ABILITY_COLUMNS.into_iter().zip(pitch.ability_icons(player)).collect();
            let icon = if list_view {
                list_status_icon(&status(p), terms)
            } else {
                status_icon(&status(p))
            };
            if let Some(icon) = icon {
                icons.push((STATUS_COLUMN, STATUS_ICONS[usize::from(icon - 1)]));
            }
            state.row_icons.push(icons);
        }
    }
    // The list view shows the parts the club had as it opened.
    let own_parts = if list_view { state.strengths } else { None };
    show_comparison(view, dialog, game, season, state, records, own_parts);
}

/// The list view's small pitch (`FUN_00503420`): a marker for each slot of
/// the eleven on its spot, the player's shirt number on it in an ink for how
/// he fits there; below, how many players stand in the eleven.
fn show_markers(view: &mut View, dialog: &Dialog, game: &Game, state: &ClubState) {
    let pitch = &game.rules.pitch;
    let lineup = &state.lineup;
    for (i, (&spot, &player)) in lineup.spots.iter().zip(&lineup.field).enumerate() {
        let Some(c) = control(dialog, MARKERS + i as u16) else { continue };
        let image = match pitch.kind_under(spot) {
            1 => KEEPER_MARKER,
            2..=5 => DEFENCE_MARKER,
            6..=9 => MIDFIELD_MARKER,
            _ => ATTACK_MARKER,
        };
        let x = (f64::from(spot.0) / f64::from(MARKER_SHRINK)).trunc() as i32;
        let at =
            Vec2::new((MARKER_ORIGIN.0 + x) as f32, (MARKER_ORIGIN.1 + i32::from(spot.1)) as f32);
        let (text, ink) = match player.and_then(|p| state.players.get(p)) {
            None => (" ".to_owned(), FITS_INK),
            Some(player) => {
                let position = pitch.position(spot);
                let kind = pitch.kind(position);
                let [main, second, third] = player.positions;
                let hanging = position == HANGING_STRIKER
                    && player.positions.iter().any(|&k| pitch.hanging(k));
                let ink = if main == position || hanging || main == kind {
                    FITS_INK
                } else if second == kind || third == kind {
                    SIDE_INK
                } else if player.traits[0] & ALLROUNDER != 0 {
                    ALLROUNDER_INK
                } else {
                    MISFIT_INK
                };
                (player.shirt.to_string(), ink)
            }
        };
        view.controls[c].state =
            ControlState::Marker(Some(crate::controls::marker::Marker { image, at, text, ink }));
    }
    let set_players = lineup.field.iter().flatten().filter(|&&p| p < state.players.len()).count();
    let line = match set_players {
        0 => game.text(PLAYERS_SET[0]).to_owned(),
        1 => game.text(PLAYERS_SET[1]).to_owned(),
        n => game.text(PLAYERS_SET[2]).replacen("%d", &n.to_string(), 1),
    };
    set(view, dialog, STATUS_LINE, line);
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
    // Away and marked: suspended, whatever else (`FUN_006fef73`).
    if status.away > 0 && status.away_kind & MARKED != 0 {
        return Some(SUSPENDED);
    }
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
    // A player of hidden strength shows no contract's end.
    let contracted = status.contract.is_none_or(|years| years > 0) || status.strength_hidden;
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
/// at the end of his contract — unless his strength is hidden, or, in the
/// list view, not known yet (`FUN_00696b4c`).
fn out(status: &Status, list_view: bool) -> bool {
    let excused = if list_view { status.strength_unknown } else { status.strength_hidden };
    status.long > 0
        || status.suspended != 0
        || status.injured >= WEEK_DAYS as i32
        || status.away > 0
        || status.contract.is_some_and(|years| years <= 0) && !excused
}

/// The status the list view's rows show (`FUN_00503b7e`): as the pitch
/// view's, but a mark of the contract's terms first, a player marked at
/// home shows the risky one's icon, and the contract's end is excused by a
/// strength not known yet.
fn list_status_icon(status: &Status, terms: u16) -> Option<u8> {
    if status.away > 0 && status.away_kind & MARKED != 0 {
        return Some(SUSPENDED);
    }
    let mut icon = None;
    if terms & TERMS_MARK != 0 {
        icon = Some(TERMS_ICON);
    }
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
    let contracted = status.contract.is_none_or(|years| years > 0) || status.strength_unknown;
    if status.away == 0 && contracted {
        if status.away_kind & MARKED != 0 {
            icon = Some(RISKY);
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
    own_parts: Option<[i32; PARTS]>,
) {
    let pitch = &game.rules.pitch;
    let players = &state.players;
    let Some(own) = season.manager().and_then(|m| m.club) else { return };
    let club = |(slot, c): (usize, usize)| game.world.country(slot).and_then(|k| k.clubs.get(c));
    let own_name = club(own).map_or("", |c| c.name.as_str());
    let cohesion = a3_core::cohesion::of(pitch, state, season.date());
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
    side(view, &state.lineup, players, &state.fitness, own_parts, ids);

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

/// A player's box as the line-up fills it (`CMannStatic`): his name, his
/// main position, his strength on the spot's position, in the colour of how
/// he fits it, and his abilities.
pub(crate) fn filled_box(
    game: &Game,
    player: &Player,
    fitness: a3_core::season::Fitness,
    position: u8,
) -> PlayerBox {
    let pitch = &game.rules.pitch;
    PlayerBox {
        name: name(player),
        position: abbreviation(game, player.positions[0]),
        strength: tenths(pitch.strength_at(player, fitness, position)),
        fit: pitch.fit(player, position),
        icons: pitch.ability_icons(player),
        duty: None,
        filled: true,
        at: None,
        area: None,
    }
}

/// A position's short name: "IV".
fn abbreviation(game: &Game, kind: u8) -> String {
    let id = game.rules.pitch.abbreviations.get(usize::from(kind)).copied().unwrap_or(0);
    game.text(id as u16).to_owned()
}

/// A player as boxes and lists name him: "Name, F.".
fn name(player: &Player) -> String {
    match player.first_name.chars().next() {
        Some(initial) => format!("{}, {initial}.", player.last_name),
        None => player.last_name.clone(),
    }
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

    /// The first of the three trainings, not built yet.
    const TRAINING_SYMBOL: usize = 3;

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
        MenuBar::default().open(&mut cx, &mut view, Args::None);
        // Crossed out: the bar's symbols that lead nowhere yet — the first
        // training's —, not the line-up's, the dressing room's or the
        // whistle.
        let bar = control(dialog, BAR_ID).unwrap();
        let crossed = |item| view.inert.contains(&Target::Option { control: bar, item });
        assert!(crossed(TRAINING_SYMBOL) && !crossed(LINEUP_SYMBOL));
        assert!(!crossed(DRESSING_SYMBOL) && !crossed(WHISTLE));
        // The rest of the test looks at the picture without the crosses.
        view.inert.clear();
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
        // box as it is, a duty after three abilities leaving two columns —
        // or yellow above the name of a position the squad has nobody for.
        let filled = game.graphic(player_box::FILLED).unwrap();
        let empty = game.graphic(player_box::EMPTY).unwrap();
        let boxes = (FIELD_BOXES..FIELD_BOXES + FIELD as u16)
            .chain(BENCH_BOXES..BENCH_BOXES + BENCH as u16);
        for id in boxes {
            let c = control(dialog, id).unwrap();
            let ControlState::Player(Some(player)) = &view.controls[c].state else {
                panic!("box {id} shows nothing");
            };
            let free =
                if player.icons.len() + player.duty.iter().len() > 3 { 57..59 } else { 46..56 };
            let (picture, columns, rows) =
                if player.filled { (&filled, free, 13..23) } else { (&empty, 0..99, 0..4) };
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
