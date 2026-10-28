//! The statistics archive (dialog 18, `CArchivTabControl`; STATISTIKEN.md
//! in the RE repo), the bar's fifteenth symbol: a combo of the categories
//! that have pages for the club, the pages of the chosen one in a list,
//! and the page itself on the right — its column titles above, a table
//! below, a strip under it to step through leagues, countries or rounds
//! (its init `FUN_00458642`, the list `FUN_0045940a`, the strip
//! `FUN_004599d8`).
//!
//! Of the pages, the league tables of `FUN_00444ec6` are drawn: the
//! current one, at home, away, of either half of the season, of either
//! half of the games, with two points a win; the forecast of the
//! champion (`FUN_00445ded`); the season's results in a cross-table
//! (`FUN_00445951`); the rounds of the national cups (`FUN_004469c0`); and
//! the Europa League's round (`FUN_004483c7`) with the European Supercup's
//! single pairing (`FUN_0044858a`); in the Liga category the league's goal
//! scorers (`FUN_00449bd6`) and their scorer points (`FUN_0044a3fa`). The
//! others are crossed out.

use std::collections::HashMap;

use a3_core::opponent::League;
use a3_core::records::{Records, Team};
use a3_core::season::Rules;
use a3_core::statistics::{self, CATEGORY_NAMES, Club, Page};
use a3_files::rch::{Align, Control, Dialog, Kind};
use bevy_math::Rect;

use super::hub::{self, STATISTICS_SYMBOL};
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::slider;
use crate::controls::table::{Graph, TableRow, TableState};
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

pub use a3_core::cycle::dialogs::STATISTICS;
const CAPTION_ID: u16 = 9935;
const CAPTION: u16 = 2859;
const CATEGORIES: u16 = 1283;
const PAGES: u16 = 1284;
const HEADER: u16 = 1292;
const FRAME: u16 = 1285;
const STRIP: u16 = 6319;
const STRIP_NAME: u16 = 4780;
const OK: u16 = 1;
/// The table the pages fill, where the hidden static 1285 stands.
const TABLE: u16 = 29999;
/// A row is this wide (`FUN_00458c56`'s 0x1c8).
const ROW_WIDTH: i32 = 0x1c8;

/// The strips a page steps with (`FUN_004599d8`): the countries played,
/// none, the leagues, the own and the opponent's statistics, the kinds of
/// goals, the countries and "International".
const COUNTRIES: i16 = 1;
const NONE: [i16; 3] = [2, 3, 7];
const LEAGUES: i16 = 4;
const SIDES: (i16, [u16; 2]) = (9, [5068, 5069]);
const GOALS: (i16, [u16; 4]) = (0xb, [3001, 3002, 3003, 3004]);
const NATIONS: (i16, u16) = (0xf, 3012);

/// The table pages' header texts, with the place before and without
/// (0xb6c, 0xb6b), and their columns; the rows' columns.
const HEADER_BEFORE: u16 = 0xb6c;
const HEADER_PLAIN: u16 = 0xb6b;
const C: u16 = 0x4000;
const R: u16 = 0x8000;
const HEADER_BEFORE_COLUMNS: [(u16, u16); 10] = [
    (C, 20),
    (R | 22, 26),
    (60, 0),
    (C | 182, 40),
    (C | 217, 20),
    (C | 240, 20),
    (C | 263, 20),
    (C | 285, 20),
    (C | 310, 30),
    (C | 340, 40),
];
const HEADER_PLAIN_COLUMNS: [(u16, u16); 9] = [
    (R, 20),
    (25, 0),
    (C | 162, 40),
    (R | 195, 20),
    (R | 220, 20),
    (R | 245, 20),
    (C | 285, 20),
    (C | 310, 30),
    (C | 340, 40),
];
const ROW_BEFORE_COLUMNS: [(u16, u16); 12] = [
    (R, 20),
    (R | 22, 26),
    (60, 0),
    (R | 190, 20),
    (R | 215, 18),
    (R | 238, 18),
    (R | 261, 18),
    (R | 284, 18),
    (R | 305, 25),
    (R | 335, 25),
    (362, 0),
    (365, 25),
];
const ROW_PLAIN_COLUMNS: [(u16, u16); 11] = [
    (R, 20),
    (25, 0),
    (R | 170, 20),
    (R | 195, 20),
    (R | 220, 20),
    (R | 245, 20),
    (R | 284, 18),
    (R | 305, 25),
    (R | 335, 25),
    (362, 0),
    (365, 25),
];
/// The forecast's header (0xa47) and columns, and its rows' width.
const FORECAST_HEADER: u16 = 0xa47;
const FORECAST_HEADER_COLUMNS: [(u16, u16); 8] = [
    (C, 20),
    (30, 0),
    (C | 195, 20),
    (C | 220, 20),
    (C | 245, 20),
    (C | 275, 20),
    (C | 300, 30),
    (C | 335, 40),
];
const FORECAST_COLUMNS: [(u16, u16); 10] = [
    (R, 20),
    (30, 0),
    (R | 195, 20),
    (R | 220, 20),
    (R | 245, 20),
    (R | 275, 20),
    (R | 300, 25),
    (R | 335, 25),
    (362, 0),
    (365, 0),
];
const FORECAST_WIDTH: i32 = 0x198;
/// The tables' category and their codes; the forecast's and the season
/// results' — the cross-table's rows as wide as the frame (`FUN_00458c56`).
const TABLES: usize = 0;
const TABLE_CODES: std::ops::RangeInclusive<i16> = 0..=7;
const FORECAST: i16 = 8;
const CROSS: i16 = 9;
const CROSS_WIDTH: i32 = 0x1dd;
/// The goal pages' header, one text for all four (`FUN_0044917a`; the
/// axis' own label 0xa51 stands empty and is never drawn).
const GOAL_HEADER: u16 = 0xa50;
/// The Liga category and its two player pages' header texts (2713 and
/// 2715) with their header's and rows' columns (`FUN_00449bd6`'s blocks
/// at 0x879708 and 0x87972c, `FUN_0044a3fa`'s at 0x8797e8 and 0x879808).
/// The goal page's own club stands blue (COLORREF 0xff0000) among black
/// rows; the scorer page inks as the tables do.
const LIGA: usize = 2;
const TOR_HEADER: u16 = 0xa99;
const SCORER_HEADER: u16 = 0xa9b;
const VORLAGEN_HEADER: u16 = 0xa9a;
const FREISTOSS_HEADER: u16 = 0xa9d;
const ELFMETER_HEADER: u16 = 0xa9e;
const KILLER_HEADER: u16 = 0xa9f;
const EIGENTORE_HEADER: u16 = 0xa9c;
const TOR_HEADER_COLUMNS: [(u16, u16); 9] = [
    (R, 0x12),
    (0x18, 0),
    (0x82, 0),
    (C | 250, 0x1e),
    (C | 280, 0x14),
    (C | 300, 0x14),
    (C | 320, 0x14),
    (C | 340, 0x14),
    (C | 360, 0x14),
];
const TOR_ROW_COLUMNS: [(u16, u16); 9] = [
    (R, 0x12),
    (0x18, 0),
    (0x82, 0),
    (R | 250, 0x18),
    (R | 275, 0x14),
    (R | 295, 0x14),
    (R | 315, 0x14),
    (R | 335, 0x14),
    (R | 355, 0x14),
];
const SCORER_HEADER_COLUMNS: [(u16, u16); 8] = [
    (R, 0x14),
    (0x19, 0),
    (0x82, 0),
    (C | 0xf5, 0x28),
    (C | 0x11d, 0x1e),
    (C | 0x13b, 0x19),
    (C | 0x159, 0x1e),
    (C | 0x172, 0x1e),
];
const SCORER_ROW_COLUMNS: [(u16, u16); 8] = [
    (R, 0x14),
    (0x19, 0),
    (0x82, 0),
    (R | 0xf0, 0x1e),
    (R | 0x11d, 0x14),
    (C | 0x13b, 0x19),
    (R | 0x159, 0x14),
    (R | 0x172, 0x14),
];
/// The three pages behind them draw their headers and their rows from
/// one and the same blocks (`FUN_0044a02e` at 0x8798a8, `FUN_0044ab7a`
/// and `FUN_0044af3b` at 0x8798a8 and 0x879908): the assisters' two
/// figures, and the free-kick and penalty takers' three.
const VORLAGEN_COLUMNS: [(u16, u16); 5] = [
    (R, 0x14),
    (0x19, 0),
    (0xa0, 0),
    (C | 0x118, 0x32),
    (C | 0x140, 0x28),
];
const TAKER_COLUMNS: [(u16, u16); 6] = [
    (R, 0x14),
    (0x19, 0),
    (0x96, 0),
    (C | 0x122, 0x28),
    (C | 0x14a, 0x28),
    (C | 0x172, 0x28),
];
/// The Elfmeterkiller and the Eigen goal scorers again from single blocks
/// for header and rows (`FUN_0044b2fc` at 0x879968, `FUN_0044a810` at
/// 0x879868): the keeper's two figures, the scorer's one.
const KILLER_COLUMNS: [(u16, u16); 5] = [
    (R, 0x14),
    (0x19, 0),
    (0x96, 0),
    (C | 0x10e, 0x32),
    (C | 0x14a, 0x32),
];
const EIGENTORE_COLUMNS: [(u16, u16); 4] = [
    (R, 0x14),
    (0x19, 0),
    (0xa0, 0),
    (C | 0x122, 0x3c),
];
const TOR_OWN_INK: [u8; 3] = [0, 0, 255];
/// The Fairneß's header and rows (`FUN_0044bb6d` with 0x879a70, and
/// 0x879a28): rank, club, points, the three kinds of cards and the
/// points a matchday; the season's record for one match after a blank
/// row in two columns of its own (0x879a60), its text 0x825 "Saisonrekord
/// in einem Spiel:\t%s (%d)".
const FAIRNESS_HEADER: u16 = 0xaa1;
const FAIRNESS_HEADER_COLUMNS: [(u16, u16); 7] = [
    (R, 0x14),
    (0x19, 0),
    (C | 0xb9, 0x1e),
    (C | 0xf0, 0x1e),
    (C | 0x10e, 0x1e),
    (C | 0x12c, 0x23),
    (C | 0x15e, 0x3c),
];
const FAIRNESS_ROW_COLUMNS: [(u16, u16); 7] = [
    (R, 0x14),
    (0x19, 0),
    (R | 0xb4, 0x1e),
    (R | 0xe6, 0x1e),
    (R | 0x102, 0x1e),
    (R | 0x122, 0x23),
    (R | 0x168, 0x1e),
];
const FAIRNESS_RECORD: u16 = 0x825;
const RECORD_COLUMNS: [(u16, u16); 2] = [(0, 0x8c), (0xc8, 0)];
/// The Durchschnittsalter's header and rows, one block for both
/// (`FUN_0044cd71`, 0x879b58 and 0x879b80 alike): rank, club, the mean
/// age, the players under 22 and over 30.
const AGE_HEADER: u16 = 0xdd9;
const AGE_COLUMNS: [(u16, u16); 5] =
    [(R, 0x14), (0x19, 0), (C | 0xc8, 0x32), (C | 0xfa, 0x32), (C | 0x12c, 0x32)];
/// The Ausländer's header and rows (`FUN_0044d117` with 0x879ba8 and
/// 0x879bc8): rank, club, foreigners and nations — and a fifth column
/// the page declares over blocks of four, its place and width read from
/// the next block's first words; no text ever stands in it.
const FOREIGN_HEADER: u16 = 0xe75;
const FOREIGN_HEADER_COLUMNS: [(u16, u16); 5] =
    [(R, 0x14), (0x19, 0), (R | 0xc8, 0x37), (R | 0xff, 0x3c), (0x14, 0x8000)];
const FOREIGN_ROW_COLUMNS: [(u16, u16); 5] =
    [(R, 0x14), (0x19, 0), (R | 0xc8, 0x32), (R | 0xfa, 0x32), (0x14, 0x8000)];
/// The Chancenverwertung's header and rows (`FUN_0044c08e` with
/// 0x879ad0, and 0x879aa8): rank, club, quote, chances and goals, its own
/// club red.
const CHANCEN_HEADER: u16 = 0xaa2;
const CHANCEN_HEADER_COLUMNS: [(u16, u16); 5] =
    [(R, 0x14), (0x19, 0), (C | 0xb4, 0x3c), (C | 0xf0, 0x3c), (C | 0x136, 0x3c)];
const CHANCEN_ROW_COLUMNS: [(u16, u16); 5] =
    [(R, 0x14), (0x19, 0), (R | 0xbe, 0x1e), (R | 0xff, 0x1e), (R | 0x136, 0x28)];
/// The cup rows' columns (`FUN_006df48b` with 0x879428 and 0x879450): the
/// two sides, the legs' scores in the right-pulled columns, and the note
/// behind them. A side with no club reads "ohne Verein" (0xfef), the note
/// "n.V." (0x761) or "n.E." (0x762), else a space.
const CUP_COLUMNS: [(u16, u16); 10] = [
    (0, 0),
    (120, 0),
    (125, 0),
    (R | 245, 20),
    (267, 0),
    (270, 0),
    (R | 290, 20),
    (312, 0),
    (315, 0),
    (350, 0),
];
const NO_CLUB: u16 = 0xfef;
const EXTRA_TIME: u16 = 0x761;
const PENALTIES: u16 = 0x762;
/// The European cups' rows' columns (`FUN_006df48b` with 0x879478 and
/// 0x8794a0): the two clubs with their countries' letters, the legs'
/// scores, and the note. The Supercup's header carries text 0xa4e.
const EUROPA_COLUMNS: [(u16, u16); 10] = [
    (0, 0),
    (140, 0),
    (145, 0),
    (R | 290, 15),
    (307, 0),
    (310, 0),
    (R | 330, 15),
    (347, 0),
    (350, 0),
    (365, 0),
];
const SUPERCUP_HEADER: u16 = 0xa4e;
/// The Champions League's group pages (`FUN_00447c15`): each group's
/// header "Gruppe %i:" (text 0xc44) and its blanks stand in one column of
/// their own (`FUN_006df44c`), its played pairs in the seven columns at
/// 0x879508 and 0x879524, and its table in the eight at 0x8794c8 and
/// 0x8794e8 — the place right, the club with its country's letters, and
/// the figures.
const GROUP_HEADER: u16 = 0xc44;
const GROUP_COLUMNS: [(u16, u16); 1] = [(0, 0)];
const MATCH_COLUMNS: [(u16, u16); 7] = [
    (15, 0),
    (165, 0),
    (170, 0),
    (R | 320, 0),
    (342, 0),
    (345, 0),
    (380, 20),
];
const STANDINGS_COLUMNS: [(u16, u16); 8] = [
    (R | 15, 15),
    (35, 0),
    (R | 175, 25),
    (R | 210, 25),
    (R | 240, 25),
    (R | 285, 20),
    (307, 0),
    (310, 0),
];
/// The own club's rows red, the others black (COLORREF 0xff).
const OWN_INK: [u8; 3] = [255, 0, 0];
const INK: [u8; 3] = [0, 0, 0];
/// The places' boxes by their code (COLORREFs 0xffff, 0x8080, 0xa000,
/// 0xa0a0ff, 0xc800, 0xc8, 0x808000).
const MARK_COLOURS: [[u8; 3]; 7] = [
    [255, 255, 0],
    [128, 128, 0],
    [0, 160, 0],
    [255, 160, 160],
    [0, 200, 0],
    [200, 0, 0],
    [0, 128, 128],
];
/// The letters of the clubs' flags (text 0x5d0 "MEANVCQ") by bit, the
/// first set counting, and "P" or "F" after it, behind a comma (0x5d1,
/// 0x5d2).
const LETTERS: u16 = 0x5d0;
const LETTER_BITS: [(u16, usize); 7] =
    [(0x1, 0), (0x2, 4), (0x4, 3), (0x8, 2), (0x40, 5), (0x10, 1), (0x4000, 6)];
const P: (u16, u16) = (0x1000, 0x5d1);
const F: (u16, u16) = (0x2000, 0x5d2);
/// The country names' table (`0xae5d8c`), by a page's country slot.
const SLOT_COUNTRIES: usize = 0xae5d8c;

/// The page's table where the hidden static stands, the caption and the
/// bar; OK goes with the bar.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&STATISTICS) else { return };
    let frame = dialog.controls.iter().find(|c| c.id == Some(FRAME)).map(|c| c.rect);
    dialog.controls.retain(|c| c.id != Some(OK) && c.id != Some(FRAME));
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
    let rect = frame.unwrap_or(Rect::new(162.0, 54.0, 639.0, 420.0));
    dialog.controls.push(Control {
        kind: Kind::Table,
        rect,
        align: Align::Left,
        text: String::new(),
        id: Some(TABLE),
        font: a3_files::rch::PLAIN_FONT,
        ink: [0; 3],
    });
    hub::add_bar(dialog, rules);
}

#[derive(Default)]
pub struct Statistics {
    whistle: bool,
    /// The categories with pages for the club, and each one's pages.
    categories: Vec<usize>,
    listed: Vec<Vec<usize>>,
    /// The category chosen, and the page shown by its entry.
    category: usize,
    page: Option<usize>,
    /// The list's row last chosen.
    row: usize,
    /// How far the page's table is scrolled, kept while the page is drawn
    /// anew.
    scrolled: usize,
    /// How far the cross-table's bar is stepped, kept the same way.
    across: usize,
    /// The strip: its names and the one chosen; the leagues it stands for
    /// when it steps through them.
    strip: Vec<String>,
    chosen: usize,
    leagues: Vec<(u16, u8)>,
}

fn level(cx: &Cx) -> i16 {
    statistics::level(own_team(cx))
}

/// Runs `f` with what the conditions ask of the club.
fn with_club<T>(cx: &Cx, f: impl FnOnce(&Club) -> T) -> Option<T> {
    let records = cx.state.records.as_ref()?;
    let season = cx.state.season.as_ref()?;
    let squad: Vec<u16> = season
        .manager()
        .and_then(|m| season.clubs.get(&m.club?))
        .map_or_else(Vec::new, |c| c.status.iter().map(|s| s.id).collect());
    let manager = super::kalkulation::own_manager(cx);
    let club = Club { records, team: own_team(cx), manager, squad: &squad };
    Some(f(&club))
}

/// A page's name in the list (`FUN_0045940a`): its text, or a European
/// cup's name; with its country's name, or with its season.
fn page_name(game: &Game, records: &Records, page: &Page) -> String {
    let mut name = if page.name & 0xf0000 == statistics::CUP_NAME {
        let names = records.countries.cup_names();
        names[if page.name & 0xf == 1 { 0 } else { 1 }].clone()
    } else {
        game.text(page.name as u16).to_owned()
    };
    let slot = usize::from((page.flags & statistics::SLOT) >> 12);
    if slot != 0 {
        let country = records.countries.get::<i16>(SLOT_COUNTRIES + 2 * slot);
        name = name.replacen("%s", game.world.nation(country as u16), 1);
    }
    let code = page.flags & statistics::CONDITION;
    if statistics::PAST_SEASONS.contains(&code) {
        let year = i32::from(records.globals.start_year()) - i32::from(code - 100);
        name = format!("{name} {}", season_text(year));
    }
    name
}

/// "2024/25", or "1999/2000" at a century's turn (`FUN_00730e8a`).
fn season_text(year: i32) -> String {
    if (year + 1) % 100 == 0 {
        format!("{year}/{}", year + 1)
    } else {
        format!("{year}/{:02}", (year + 1) % 100)
    }
}

/// A club's name with its letters: "FC Bayern München (M)".
fn lettered(game: &Game, records: &Records, country: u16, team: i16) -> String {
    let name = game.club_name(Some(records), country as u8, team);
    let flags = statistics::letters(records, country, team);
    let letters: Vec<char> = game.text(LETTERS).chars().collect();
    let mut tag: String = LETTER_BITS
        .iter()
        .find(|&&(bit, _)| flags & bit != 0)
        .and_then(|&(_, k)| letters.get(k))
        .map(char::to_string)
        .unwrap_or_default();
    for (bit, text) in [P, F] {
        if flags & bit != 0 {
            if !tag.is_empty() {
                tag.push(',');
            }
            tag.push_str(game.text(text));
            break;
        }
    }
    if tag.is_empty() { name } else { format!("{name} ({tag})") }
}

/// Sets the header's titles and their columns.
fn set_header(view: &mut View, dialog: &Dialog, text: String, columns: &[(u16, u16)]) {
    let Some(header) = control(dialog, HEADER).and_then(|c| view.header_mut(c)) else { return };
    header.text = text;
    header.tabs = columns.iter().map(|&(x, _)| f32::from(x & 0x3fff)).collect();
    header.aligns = columns
        .iter()
        .map(|&(x, w)| match x & (C | R) {
            C => Some((Align::Centre, f32::from(w))),
            R => Some((Align::Right, f32::from(w))),
            _ => None,
        })
        .collect();
}

/// The Fairneß's points a matchday as "%2.2f" writes the x87 quotient:
/// to two places, a half rounded up as the C library of its day does —
/// with a club's whole points over a few dozen matchdays a half can only
/// come exactly, in eighths. Before the first matchday it divides by 0.
fn per_matchday(points: i16, played: i16) -> String {
    if played <= 0 {
        return if points == 0 { "-1.#J" } else { "1.#J" }.to_owned();
    }
    let (points, played) = (i32::from(points), i32::from(played));
    let hundredths = (200 * points + played) / (2 * played);
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

/// A float as "%3.1f" writes it: to one place, a half rounded up as the
/// C library of its day does. Ten times a float stands exactly in a
/// double, so a half is told exactly.
fn one_place(value: f32) -> String {
    let tenths = (f64::from(value) * 10.0 + 0.5).floor() as i64;
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// The leagues the strip steps through (`FUN_0045a439`): the main
/// country's, then the first of each other country played.
fn leagues(records: &Records) -> Vec<(u16, u8)> {
    let globals = &records.globals;
    let main = globals.main_country() as u16;
    let count = records.countries.leagues(main.into()).max(0) as u8;
    let more = usize::try_from(globals.more_count()).unwrap_or(0);
    (0..count)
        .map(|level| (main, level))
        .chain(globals.more_countries().iter().take(more).map(|&c| (c as u16, 0)))
        .collect()
}

/// The own club's league among them (`FUN_0063e2df`).
fn own_league(cx: &Cx, leagues: &[(u16, u8)]) -> usize {
    let Some(team) = own_team(cx) else { return 0 };
    let (country, level) = (u16::from(team.country()), team.level());
    leagues
        .iter()
        .position(|&(c, l)| c == country && (l == level || l == 0 && leagues[0].0 != c))
        .unwrap_or(0)
}

impl Statistics {
    /// The categories with pages for the club (`FUN_00458d3d`).
    fn fill_categories(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let level = level(cx);
        let pages = &game.rules.statistics.categories;
        self.listed = with_club(cx, |club| {
            pages
                .iter()
                .map(|cat| {
                    let shown = |(_, page): &(usize, &Page)| statistics::listed(club, level, page);
                    cat.iter().enumerate().filter(shown).map(|(i, _)| i).collect()
                })
                .collect()
        })
        .unwrap_or_default();
        self.categories = (0..self.listed.len()).filter(|&k| !self.listed[k].is_empty()).collect();
        if let Some(combo) = control(dialog, CATEGORIES).and_then(|c| view.combo_mut(c)) {
            combo.items =
                self.categories.iter().map(|&k| game.text(CATEGORY_NAMES[k]).to_owned()).collect();
            combo.selected = Some(0);
        }
        self.category = self.categories.first().copied().unwrap_or(0);
    }

    /// The chosen category's pages in the list, the first chosen.
    fn fill_pages(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return };
        let pages = &game.rules.statistics.categories[self.category];
        let entries = self.listed.get(self.category).cloned().unwrap_or_default();
        if let Some(list) = control(dialog, PAGES).and_then(|c| view.list_mut(c)) {
            list.rows = entries.iter().map(|&e| page_name(game, records, &pages[e])).collect();
            list.keys = entries.clone();
            list.selected = Some(0);
            list.scroll = 0;
        }
        self.row = 0;
    }

    /// Shows a page of the chosen category (`FUN_004598eb`): its strip
    /// anew, then the page.
    fn show(&mut self, cx: &Cx, view: &mut View, entry: usize) {
        if self.page == Some(entry) {
            return;
        }
        self.page = Some(entry);
        self.scrolled = 0;
        self.across = 0;
        let Some(&page) = cx.game.rules.statistics.categories[self.category].get(entry) else {
            return;
        };
        self.set_strip(cx, view, page.selector);
        self.draw_page(cx, view);
    }

    /// The strip's names for a page's kind of strip; a kind it does not
    /// know keeps the last (`FUN_004599d8`). The handle is as long as the
    /// track is to the names (`FUN_0045ad88`).
    fn set_strip(&mut self, cx: &Cx, view: &mut View, kind: i16) {
        let game = cx.game;
        let Some(records) = cx.state.records.as_ref() else { return };
        let text = |id: u16| game.text(id).to_owned();
        let globals = &records.globals;
        let more = usize::try_from(globals.more_count()).unwrap_or(0);
        let countries = || {
            std::iter::once(globals.main_country())
                .chain(globals.more_countries().into_iter().take(more))
                .map(|c| game.world.nation(c as u16).to_owned())
                .collect::<Vec<_>>()
        };
        self.strip = match kind {
            COUNTRIES => countries(),
            k if NONE.contains(&k) => Vec::new(),
            LEAGUES => {
                self.leagues = leagues(records);
                let name = |&(c, l): &(u16, u8)| {
                    League::of(records, c, l).map(|l| l.name()).unwrap_or_default()
                };
                self.leagues.iter().map(name).collect()
            }
            k if k == SIDES.0 => SIDES.1.map(text).to_vec(),
            k if k == GOALS.0 => GOALS.1.map(text).to_vec(),
            k if k == NATIONS.0 => {
                let mut names = countries();
                names.push(text(NATIONS.1));
                names
            }
            // A national cup: the rounds it is played from and to, by
            // name.
            k if statistics::CUP_STRIPS.contains(&k) => {
                let (first, last) = statistics::cup_rounds(records, (k - 0x12) as usize);
                (i16::max(first, 0)..=i16::min(last, 6))
                    .map(|r| text(statistics::CUP_ROUND_NAMES[r as usize]))
                    .collect()
            }
            // A league cup's strip: the rounds of the one cup object,
            // whichever of the pages asks for it.
            k if statistics::LEAGUE_CUP_STRIPS.contains(&k) => {
                let (first, last) = statistics::league_cup_rounds(records);
                (i16::max(first, 0)..=i16::min(last, 6))
                    .map(|r| text(statistics::CUP_ROUND_NAMES[r as usize]))
                    .collect()
            }
            // The Champions League: the rounds up to the stage it has
            // reached, by the counter at 0xb5d164.
            k if k == statistics::CL_STRIP => {
                let n = statistics::cl_strip_count(records);
                statistics::CL_ROUND_NAMES[..n].iter().map(|&r| text(r)).collect()
            }
            // The Europa League: as many round names as are played, the
            // counter at 0xb5d166 one below the round being played.
            k if k == statistics::UEFA_STRIP => {
                let n = statistics::uefa_round_count(records);
                statistics::UEFA_ROUND_NAMES[..n].iter().map(|&r| text(r)).collect()
            }
            _ => return,
        };
        let n = self.strip.len();
        if n < 2 {
            if self.chosen >= n {
                self.chosen = 0;
            }
        } else {
            self.chosen = self.chosen.min(n - 1);
        }
        self.show_strip(cx, view);
    }

    fn show_strip(&self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        let n = self.strip.len() as i32;
        if let Some(c) = control(dialog, STRIP) {
            let handle = slider::handle_for(&dialog.controls[c], n);
            if let Some(value) = view.value_mut(c) {
                value.range = (0, (n - 1).max(0));
                value.value = self.chosen as i32;
                value.handle = Some(handle);
            }
        }
        let name = self.strip.get(self.chosen).cloned().unwrap_or_default();
        set(view, dialog, STRIP_NAME, name);
    }

    /// The page shown, as its category draws it; one not built is crossed
    /// out.
    fn draw_page(&self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        let pages = &cx.game.rules.statistics.categories[self.category];
        let page = self.page.and_then(|e| pages.get(e));
        let built = match page.map(|p| p.code) {
            Some(code) if self.category == TABLES && TABLE_CODES.contains(&code) => {
                self.table_page(cx, view, code)
            }
            Some(FORECAST) if self.category == TABLES => self.forecast_page(cx, view),
            Some(CROSS) if self.category == TABLES => self.cross_page(cx, view),
            Some(code) if self.category == TABLES && statistics::GOAL_PAGES.contains(&code) => {
                self.goal_page(cx, view, code)
            }
            Some(code) if self.category == TABLES && statistics::CUP_PAGES.contains(&code) => {
                self.cup_page(cx, view, false)
            }
            Some(code)
                if self.category == TABLES && statistics::LEAGUE_CUP_PAGES.contains(&code) =>
            {
                self.cup_page(cx, view, true)
            }
            Some(statistics::CL_PAGE) if self.category == TABLES => self.cl_page(cx, view),
            Some(statistics::UEFA_PAGE) if self.category == TABLES => self.uefa_page(cx, view),
            Some(statistics::SUPERCUP_PAGE) if self.category == TABLES => {
                self.supercup_page(cx, view)
            }
            Some(code @ (statistics::TOR_PAGE..=statistics::ELFMETER_PAGE
                | statistics::KILLER_PAGE
                | statistics::EIGENTORE_PAGE))
                if self.category == LIGA =>
            {
                self.liga_page(cx, view, code)
            }
            Some(statistics::FAIRNESS_PAGE) if self.category == LIGA => {
                self.fairness_page(cx, view)
            }
            Some(statistics::CHANCEN_PAGE) if self.category == LIGA => {
                self.chance_page(cx, view)
            }
            Some(code @ (statistics::AGE_PAGE | statistics::FOREIGN_PAGE))
                if self.category == LIGA =>
            {
                self.squad_page(cx, view, code)
            }
            _ => false,
        };
        let table = control(dialog, TABLE);
        if !built {
            set_header(view, dialog, String::new(), &[(0, 0)]);
            if let Some(t) = table.and_then(|c| view.table_mut(c)) {
                t.rows.clear();
            }
        }
        mark_inert(view, dialog, |t| hub::on_bar(dialog, t).is_none_or(hub::bar_works));
        if let (false, Some(c)) = (built, table) {
            view.inert.push(Target::Control(c));
        }
    }

    /// The league tables (`FUN_00444ec6`): the league the strip stands on,
    /// its rows as `kind` reckons them, each with its place, its name and
    /// letters, its figures, and its place's box.
    fn table_page(&self, cx: &Cx, view: &mut View, code: i16) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        let Some(league) = League::of(records, country, level) else { return false };
        let kind = code as u8;
        let before = kind == statistics::ALL || kind == statistics::TWO_POINTS;
        let (header, header_columns, columns): (u16, &[(u16, u16)], &[(u16, u16)]) = if before {
            (HEADER_BEFORE, &HEADER_BEFORE_COLUMNS, &ROW_BEFORE_COLUMNS)
        } else {
            (HEADER_PLAIN, &HEADER_PLAIN_COLUMNS, &ROW_PLAIN_COLUMNS)
        };
        set_header(view, dialog, game.text(header).to_owned(), header_columns);
        let own = own_team(cx).map(|t: &Team| (u16::from(t.country()), i16::from(t.number())));
        let marks = league.marks();
        let rows = statistics::table(records, &league, kind)
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let name = lettered(game, records, country, row.team);
                let figures = format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t:\t{}",
                    row.games,
                    row.won,
                    row.drawn,
                    row.lost,
                    row.points,
                    row.goals_for - row.goals_against,
                    row.goals_for,
                    row.goals_against
                );
                let text = if before {
                    let place = league.place_before(row.place);
                    let place = if place == 0 { String::new() } else { format!("({place}.)") };
                    format!("{}.\t{place}\t{name}\t{figures}", i + 1)
                } else {
                    format!("{}.\t{name}\t{figures}", i + 1)
                };
                let mark = usize::try_from(marks.get(i).copied().unwrap_or(0) - 1)
                    .ok()
                    .and_then(|k| MARK_COLOURS.get(k).copied());
                let ink = if own == Some((country, row.team)) { OWN_INK } else { INK };
                TableRow { text, ink, mark, columns: None }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: columns.to_vec(),
                rows,
                width: ROW_WIDTH,
                centred: -1,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The season's results (`FUN_00445951` into the game's
    /// `CTabellenWnd`): the league the strip stands on laid crosswise,
    /// rows and columns in the league's own places — the order the
    /// season started in, not today's standings — each row the club of
    /// its place, each of the ten columns shown the club of its number,
    /// every cell the game the row's club had at home against the
    /// column's, a club's own cell over a blue box, and the place's
    /// number over each column.
    fn cross_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        let Some(league) = League::of(records, country, level) else { return false };
        let pages = &game.rules.statistics.categories[self.category];
        let Some(page) = self.page.and_then(|e| pages.get(e)) else { return false };
        set_header(view, dialog, game.text(page.name as u16).to_owned(), &[(0, 0)]);
        let own = own_team(cx).map(|t: &Team| (u16::from(t.country()), i16::from(t.number())));
        let cells = statistics::cross_table(&league);
        let last = (self.across + 10).min(league.teams());
        let rows = (0..league.teams())
            .map(|place| {
                let team = league.team(place);
                let name = game.club_name(Some(records), country as u8, team);
                let mut text = format!("{} {}", place + 1, name);
                for j in self.across..last {
                    text.push('\t');
                    if let Some((a, b)) = cells[place][j] {
                        text.push_str(&format!("{a}:{b}"));
                    }
                }
                let ink = if own == Some((country, team)) { OWN_INK } else { INK };
                TableRow { text, ink, mark: None, columns: None }
            })
            .collect();
        // The name stands two in and runs to where the cells begin; each
        // cell is 30 wide and the next stands 31 over (`FUN_0072cec3`).
        let mut columns: Vec<(u16, u16)> = vec![(2, 138)];
        for j in self.across..last {
            columns.push((C | (139 + 31 * (j - self.across) as u16), 30));
        }
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns,
                rows,
                width: CROSS_WIDTH,
                centred: -1,
                scroll: 0,
                cross: Some(self.across),
                graph: None,
            };
        }
        true
    }

    /// The goal pages (`FUN_0044917a` over `FUN_00448c76`'s sums): the
    /// league the strip stands on, its clubs by the goals they scored,
    /// conceded, the difference or both together — biggest first, ties
    /// keeping the league's places — each with a bar on the axis that
    /// ends at the greatest value's next full score of 25, the plot
    /// 376 pixels wide and the bar rounded to it; a negative bar green.
    /// Every row is labelled "name (value)" (`FUN_0044917a`'s "%s
    /// (%d)"), the own club's red. At most twenty rows are shown.
    fn goal_page(&self, cx: &Cx, view: &mut View, code: i16) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        let Some(league) = League::of(records, country, level) else { return false };
        set_header(view, dialog, game.text(GOAL_HEADER).to_owned(), &[(0, 0)]);
        let own = own_team(cx).map(|t: &Team| (u16::from(t.country()), i16::from(t.number())));
        let mode = statistics::GOAL_PAGES.iter().position(|&c| c == code).unwrap_or(0);
        let (order, axis) = statistics::goal_graph(&statistics::goal_sums(&league), mode);
        let plot = 376.0 / axis as f32;
        let mut rows = Vec::new();
        let mut bars = Vec::new();
        for (place, value) in order.into_iter().take(0x14) {
            let team = league.team(place);
            let name = game.club_name(Some(records), country as u8, team);
            let text = format!("{name} ({value})");
            let ink = if own == Some((country, team)) { OWN_INK } else { INK };
            rows.push(TableRow { text, ink, mark: None, columns: None });
            bars.push(((f32::from(value.unsigned_abs()) * plot + 0.5) as i32, value < 0));
        }
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: vec![],
                rows,
                width: ROW_WIDTH,
                centred: -1,
                scroll: 0,
                cross: None,
                graph: Some(Graph { bars }),
            };
        }
        true
    }

    /// The Liga category's player pages (`FUN_00449bd6`, `FUN_0044a3fa`,
    /// `FUN_0044a02e`, `FUN_0044ab7a`, `FUN_0044af3b`, `FUN_0044b2fc` and
    /// `FUN_0044a810`): the league the strip stands on draws the list the
    /// game keeps for it as the season goes on and saves with the game
    /// (`DAT_00b0b6b8`) — a hundred top scorers by their goals, twenty
    /// scorers by their points, and twenty each of the assisters, the
    /// free-kick and penalty takers, the penalty keepers and the own
    /// goal scorers; the page never sorts. Each row reads
    /// "%d.\t%s\t%s…" (0x879ed8, 0x879ef4,
    /// 0x879f04, 0x879f30, 0x879f44, 0x879f58 and 0x879f20) with the
    /// club's name and the figures from the gate byte on; the goal and
    /// scorer pages name the player "%s, %c" (0x879ed0), his last name,
    /// a comma and his first name's initial, the five others spell both
    /// names out ("FUN_00637cbb"'s "%s %s", 0x89d96c). The own club's row
    /// stands blue on all but the scorer page, where it is red.
    fn liga_page(&self, cx: &Cx, view: &mut View, code: i16) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        // header text, header's and rows' columns, the saved list, the
        // figures a row prints, whether the names are spelled out, and
        // whether the page hands its row count to the window (the taker,
        // keeper and own goal pages hand 0xffffffff instead, so their
        // rows start at the top).
        let (header, header_columns, columns, list, shown, spelled, centred) = match code {
            statistics::TOR_PAGE => (
                TOR_HEADER,
                &TOR_HEADER_COLUMNS[..],
                &TOR_ROW_COLUMNS[..],
                statistics::torjaeger(records, country, level),
                6,
                false,
                true,
            ),
            statistics::SCORER_PAGE => (
                SCORER_HEADER,
                &SCORER_HEADER_COLUMNS[..],
                &SCORER_ROW_COLUMNS[..],
                statistics::scorers(records, country, level),
                5,
                false,
                true,
            ),
            statistics::VORLAGEN_PAGE => (
                VORLAGEN_HEADER,
                &VORLAGEN_COLUMNS[..],
                &VORLAGEN_COLUMNS[..],
                statistics::vorlagen(records, country, level),
                2,
                true,
                true,
            ),
            statistics::FREISTOSS_PAGE => (
                FREISTOSS_HEADER,
                &TAKER_COLUMNS[..],
                &TAKER_COLUMNS[..],
                statistics::freistosse(records, country, level),
                3,
                true,
                false,
            ),
            statistics::ELFMETER_PAGE => (
                ELFMETER_HEADER,
                &TAKER_COLUMNS[..],
                &TAKER_COLUMNS[..],
                statistics::elfmeter(records, country, level),
                3,
                true,
                false,
            ),
            statistics::KILLER_PAGE => (
                KILLER_HEADER,
                &KILLER_COLUMNS[..],
                &KILLER_COLUMNS[..],
                statistics::elfmeterkiller(records, country, level),
                2,
                true,
                false,
            ),
            statistics::EIGENTORE_PAGE => (
                EIGENTORE_HEADER,
                &EIGENTORE_COLUMNS[..],
                &EIGENTORE_COLUMNS[..],
                statistics::eigentore(records, country, level),
                1,
                true,
                false,
            ),
            _ => return false,
        };
        set_header(view, dialog, game.text(header).to_owned(), header_columns);
        let own = own_team(cx).map(|t: &Team| (u16::from(t.country()), i16::from(t.number())));
        let rows: Vec<_> = list
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let (first, last) = (records.name(row.first), records.name(row.last));
                let name = if spelled {
                    match (first.is_empty(), last.is_empty()) {
                        (true, _) => last.to_owned(),
                        (_, true) => first.to_owned(),
                        _ => format!("{first} {last}"),
                    }
                } else {
                    match first.chars().next() {
                        Some(initial) => format!("{last}, {initial}"),
                        None => last.to_owned(),
                    }
                };
                let club = game.club_name(Some(records), country as u8, row.team);
                let mut text = format!("{}.\t{name}\t{club}", i + 1);
                for v in &row.values[..shown] {
                    text.push('\t');
                    text.push_str(&v.to_string());
                }
                let ink = if own == Some((country, row.team)) {
                    if code == statistics::SCORER_PAGE {
                        OWN_INK
                    } else {
                        TOR_OWN_INK
                    }
                } else {
                    INK
                };
                TableRow { text, ink, mark: None, columns: None }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            // The page hands its row count to the window (`FUN_00458c56`'s
            // first parameter, the window's +0x10ec), so `FUN_006de7b0`
            // stands the shown rows centred in its height.
            let centred = if centred { i32::try_from(rows.len()).unwrap_or(-1) } else { -1 };
            *table = TableState {
                columns: columns.to_vec(),
                rows,
                width: ROW_WIDTH,
                centred,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Fairneß Mannschaft (`FUN_0044bb6d`): the league's clubs as
    /// the game keeps them ordered, "%d.\t%s\t%d\t%d\t%d\t%d\t%2.2f", from
    /// the top of the window, the own club red; then, once a club holds
    /// the season's record for one match, a blank row and the record.
    fn fairness_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        set_header(view, dialog, game.text(FAIRNESS_HEADER).to_owned(), &FAIRNESS_HEADER_COLUMNS);
        let own = own_team(cx).map(|t: &Team| (u16::from(t.country()), i16::from(t.number())));
        let ink = |team: i16| if own == Some((country, team)) { OWN_INK } else { INK };
        let fair = statistics::fairness(records, country, level);
        let mut rows: Vec<TableRow> = fair
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let club = game.club_name(Some(records), country as u8, row.team);
                let [red, yellow_red, yellow] = row.cards;
                let average = per_matchday(row.points, fair.played);
                let text = format!(
                    "{}.\t{club}\t{}\t{red}\t{yellow_red}\t{yellow}\t{average}",
                    i + 1,
                    row.points
                );
                TableRow { text, ink: ink(row.team), mark: None, columns: None }
            })
            .collect();
        if let Some((team, points)) = fair.record {
            // `FUN_006df44c` gives the blank its one column, `FUN_006df73a`
            // the record its two.
            rows.push(TableRow {
                text: String::new(),
                ink: INK,
                mark: None,
                columns: Some(GROUP_COLUMNS.to_vec()),
            });
            let club = game.club_name(Some(records), country as u8, team);
            let text = super::finances::printf(
                &game.text(FAIRNESS_RECORD).replace("\\t", "\t"),
                &[&club, &points],
            );
            rows.push(TableRow {
                text,
                ink: ink(team),
                mark: None,
                columns: Some(RECORD_COLUMNS.to_vec()),
            });
        }
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: FAIRNESS_ROW_COLUMNS.to_vec(),
                rows,
                width: ROW_WIDTH,
                centred: -1,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The pages of the squads' figures, from the top of the window, the
    /// own club red: the Durchschnittsalter (`FUN_0044cd71`), the youngest
    /// squad first, "%d.\t%s\t%3.1f\t%d\t%d"; the Ausländer
    /// (`FUN_0044d117`), the most foreigners first, "%d.\t%s\t%d\t%d".
    fn squad_page(&self, cx: &Cx, view: &mut View, code: i16) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(today) = cx.state.season.as_ref().map(|s| s.date()) else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        let ages = code == statistics::AGE_PAGE;
        let (header, header_columns, columns) = if ages {
            (AGE_HEADER, &AGE_COLUMNS, &AGE_COLUMNS)
        } else {
            (FOREIGN_HEADER, &FOREIGN_HEADER_COLUMNS, &FOREIGN_ROW_COLUMNS)
        };
        set_header(view, dialog, game.text(header).to_owned(), header_columns);
        let own = own_team(cx).map(|t: &Team| (u16::from(t.country()), i16::from(t.number())));
        let figures = statistics::club_figures(records, country, level, today);
        let sorted =
            if ages { statistics::by_age(figures) } else { statistics::by_foreigners(figures) };
        let rows = sorted
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let club = game.club_name(Some(records), country as u8, row.team);
                let text = if ages {
                    format!(
                        "{}.\t{club}\t{}\t{}\t{}",
                        i + 1,
                        one_place(row.average_age),
                        row.under_22,
                        row.over_30
                    )
                } else {
                    format!("{}.\t{club}\t{}\t{}", i + 1, row.foreigners, row.nations)
                };
                let ink = if own == Some((country, row.team)) { OWN_INK } else { INK };
                TableRow { text, ink, mark: None, columns: None }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: columns.to_vec(),
                rows,
                width: ROW_WIDTH,
                centred: -1,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Chancenverwertung (`FUN_0044c08e`): the league's clubs by
    /// their quote, the rank without a dot ("%d\t%s\t%d%%\t%d\t%d", a
    /// plain 0% without a chance), from the top of the window.
    fn chance_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        set_header(view, dialog, game.text(CHANCEN_HEADER).to_owned(), &CHANCEN_HEADER_COLUMNS);
        let own = own_team(cx).map(|t: &Team| (u16::from(t.country()), i16::from(t.number())));
        let rows = statistics::chancen(records, country, level)
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let club = game.club_name(Some(records), country as u8, row.team);
                let text =
                    format!("{}\t{club}\t{}%\t{}\t{}", i + 1, row.quote, row.chances, row.goals);
                let ink = if own == Some((country, row.team)) { OWN_INK } else { INK };
                TableRow { text, ink, mark: None, columns: None }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: CHANCEN_ROW_COLUMNS.to_vec(),
                rows,
                width: ROW_WIDTH,
                centred: -1,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }
}

impl Statistics {
    /// A cup's page (`FUN_004469c0`, and `FUN_00446c74` for the league
    /// cups): the cup of the page's country, or the one cup object, the
    /// round the strip stands on counting from the cup's first, and its
    /// pairings as the row `FUN_0044893f` writes them — sides, legs and
    /// the "n.V."/"n.E." note over a blank header, the own club's
    /// pairing red.
    fn cup_page(&self, cx: &Cx, view: &mut View, league: bool) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let pages = &game.rules.statistics.categories[self.category];
        let Some(page) = self.page.and_then(|e| pages.get(e)) else { return false };
        // A national cup's slot names the country whose cup it shows; a
        // league cup's only names it, the pairings standing in the one
        // cup object either way.
        let cup = if league {
            None
        } else {
            let slot = usize::from((page.flags & statistics::SLOT) >> 12);
            let country = records.countries.get::<i16>(SLOT_COUNTRIES + 2 * slot);
            Some(usize::from(records.countries.slot(country as usize)))
        };
        let (first, _) = match cup {
            Some(cup) => statistics::cup_rounds(records, cup),
            None => statistics::league_cup_rounds(records),
        };
        let round = first + i16::try_from(self.chosen).unwrap_or(i16::MAX);
        set_header(view, dialog, String::new(), &[(0, 0)]);
        let own = own_team(cx).map(|t| (u16::from(t.country()), i16::from(t.number())));
        let pairings = match cup {
            Some(cup) => statistics::cup_pairings(records, cup, round),
            None => statistics::league_cup_pairings(records, round),
        };
        let rows = pairings
            .iter()
            .map(|p| {
                let side = |country: u8, team: i16| {
                    if country == 0 {
                        game.text(NO_CLUB).to_owned()
                    } else {
                        game.club_name(Some(records), country, team)
                    }
                };
                let leg = |leg: Option<(u8, u8)>, empty: &str| {
                    leg.map_or_else(|| empty.to_owned(), |(a, b)| format!("\t{a}\t:\t{b}"))
                };
                let note = if p.penalties {
                    game.text(PENALTIES)
                } else if p.extra_time {
                    game.text(EXTRA_TIME)
                } else {
                    " "
                };
                let text = format!(
                    "{}\t-\t{}{}{}\t{note}",
                    side(p.home_country, p.home),
                    side(p.away_country, p.away),
                    leg(p.first, "\t\t\t"),
                    leg(p.second, "")
                );
                let ink = if own == Some((u16::from(p.home_country), p.home))
                    || own == Some((u16::from(p.away_country), p.away))
                {
                    OWN_INK
                } else {
                    INK
                };
                TableRow { text, ink, mark: None, columns: None }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table =
                TableState {
                    columns: CUP_COLUMNS.to_vec(),
                    rows,
                    width: ROW_WIDTH,
                    centred: -1,
                    scroll: self.scrolled,
                    cross: None,
                    graph: None,
                };
        }
        true
    }

    /// The Europa League (`FUN_004483c7`): the round the strip stands on,
    /// drawn as the European row builder makes its pairs, under the cup's
    /// own name and blank columns. The qualification keeps 41 pairs even
    /// though round 1's start stands at the 50th.
    fn uefa_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let pages = &game.rules.statistics.categories[self.category];
        let Some(page) = self.page.and_then(|e| pages.get(e)) else { return false };
        set_header(view, dialog, cup_title(records, page), &[(0, 0)]);
        let pairings = statistics::uefa_pairings(records, self.chosen);
        let own = own_team(cx).map(|t| t.country());
        let rows = europa_rows(game, records, own, &pairings);
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table =
                TableState {
                    columns: EUROPA_COLUMNS.to_vec(),
                    rows,
                    width: ROW_WIDTH,
                    centred: -1,
                    scroll: self.scrolled,
                    cross: None,
                    graph: None,
                };
        }
        true
    }

    /// The Champions League (`FUN_00448131`): the qualification's and the
    /// knock-outs' pairs as on the other European pages; and a group
    /// page as `FUN_00447c15` fills it — each group its header, its
    /// played pairs, a blank, its table and a blank again. A table row
    /// stands red by `FUN_00677428` and `FUN_004497b4`: a human club's,
    /// or one of the own country.
    fn cl_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let pages = &game.rules.statistics.categories[self.category];
        let Some(page) = self.page.and_then(|e| pages.get(e)) else { return false };
        set_header(view, dialog, cup_title(records, page), &[(0, 0)]);
        let own = own_team(cx).map(|t| t.country());
        let mut rows = match self.chosen {
            0 | 4..=6 => europa_rows(game, records, own, &statistics::cl_pairings(records, self.chosen)),
            _ => Vec::new(),
        };
        for group in statistics::cl_groups(records, self.chosen) {
            let header = game.text(GROUP_HEADER).replace("%i", &(group.number + 1).to_string());
            rows.push(TableRow {
                text: header,
                ink: INK,
                mark: None,
                columns: Some(GROUP_COLUMNS.to_vec()),
            });
            let played: Vec<_> =
                group.pairings.iter().filter(|p| p.first.is_some()).copied().collect();
            for mut row in europa_rows(game, records, own, &played) {
                row.columns = Some(MATCH_COLUMNS.to_vec());
                rows.push(row);
            }
            rows.push(TableRow {
                text: String::new(),
                ink: INK,
                mark: None,
                columns: Some(GROUP_COLUMNS.to_vec()),
            });
            for (i, line) in group.table.iter().enumerate() {
                let name = if line.country == 0 {
                    game.text(NO_CLUB).to_owned()
                } else {
                    game.club_name(Some(records), line.country, line.club)
                };
                let letters = game
                    .world
                    .nation_codes
                    .get(usize::from(line.country))
                    .cloned()
                    .unwrap_or_default();
                let text = format!(
                    "{}.\t{name} ({letters})\t{}\t{}\t{}\t{}\t:{}\t",
                    i + 1,
                    line.games,
                    line.points,
                    line.goals_for - line.goals_against,
                    line.goals_for,
                    line.goals_against
                );
                let ink = if statistics::is_human(records, line.country, line.club)
                    || own == Some(line.country)
                {
                    OWN_INK
                } else {
                    INK
                };
                rows.push(TableRow {
                    text,
                    ink,
                    mark: None,
                    columns: Some(STANDINGS_COLUMNS.to_vec()),
                });
            }
            rows.push(TableRow {
                text: String::new(),
                ink: INK,
                mark: None,
                columns: Some(GROUP_COLUMNS.to_vec()),
            });
        }
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table =
                TableState {
                    columns: EUROPA_COLUMNS.to_vec(),
                    rows,
                    width: ROW_WIDTH,
                    centred: -1,
                    scroll: self.scrolled,
                    cross: None,
                    graph: None,
                };
        }
        true
    }

    /// The European Supercup (`FUN_0044858a`): the two cups' winners as
    /// one row, centred under the header's text 0xa4e — the pair often
    /// still unplayed when the season has barely begun.
    fn supercup_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        set_header(view, dialog, game.text(SUPERCUP_HEADER).to_owned(), &[(0, 0)]);
        let own = own_team(cx).map(|t| t.country());
        let rows = statistics::supercup_pairing(records)
            .map(|p| europa_rows(game, records, own, &[p]))
            .unwrap_or_default();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table =
                TableState {
                    columns: EUROPA_COLUMNS.to_vec(),
                    rows,
                    width: ROW_WIDTH,
                    centred: 1,
                    scroll: self.scrolled,
                    cross: None,
                    graph: None,
                };
        }
        true
    }
}

/// The name a cup page's entry carries: "Champions League" or the
/// "UEFA Cup" (the Countries block's two, 65537 and 65538).
fn cup_title(records: &Records, page: &Page) -> String {
    let names = records.countries.cup_names();
    names[if page.name & 0xf == 1 { 0 } else { 1 }].clone()
}

/// The pairs as the European row builder `FUN_00448aca` draws them: both
/// clubs with their country's three letters (the table at `0xae5768`),
/// the legs' scores where they were played, and the note as on the cup
/// pages. Red stands on every pair whose home or away club plays in the
/// own club's country — not only on the own club's own pair.
fn europa_rows(
    game: &Game,
    records: &Records,
    own: Option<u8>,
    pairings: &[statistics::CupPairing],
) -> Vec<TableRow> {
    pairings
        .iter()
        .map(|p| {
            let side = |country: u8, team: i16| {
                let name = if country == 0 {
                    game.text(NO_CLUB).to_owned()
                } else {
                    game.club_name(Some(records), country, team)
                };
                let letters =
                    game.world.nation_codes.get(usize::from(country)).cloned().unwrap_or_default();
                format!("{name} ({letters})")
            };
            let leg = |leg: Option<(u8, u8)>, empty: &str| {
                leg.map_or_else(|| empty.to_owned(), |(a, b)| format!("\t{a}\t:\t{b}"))
            };
            let note = if p.penalties {
                game.text(PENALTIES)
            } else if p.extra_time {
                game.text(EXTRA_TIME)
            } else {
                " "
            };
            let text = format!(
                "{}\t-\t{}{}{}\t{note}",
                side(p.home_country, p.home),
                side(p.away_country, p.away),
                leg(p.first, "\t\t\t"),
                leg(p.second, "")
            );
            let ink = if own == Some(p.home_country) || own == Some(p.away_country) {
                OWN_INK
            } else {
                INK
            };
            TableRow { text, ink, mark: None, columns: None }
        })
        .collect()
}

impl Statistics {
    /// The champion's forecast (`FUN_00445ded`): the rows as the rest of
    /// the season would leave them, without letters or boxes.
    fn forecast_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        let Some(league) = League::of(records, country, level) else { return false };
        set_header(view, dialog, game.text(FORECAST_HEADER).to_owned(), &FORECAST_HEADER_COLUMNS);
        let own = own_team(cx).map(|t: &Team| (u16::from(t.country()), i16::from(t.number())));
        let rows = statistics::forecast(records, &league)
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let name = game.club_name(Some(records), country as u8, row.team);
                let text = format!(
                    "{}.\t{name}\t{}\t{}\t{}\t{}\t{}\t{}\t:\t{}",
                    i + 1,
                    row.won,
                    row.drawn,
                    row.lost,
                    row.points,
                    row.goals_for - row.goals_against,
                    row.goals_for,
                    row.goals_against
                );
                let ink = if own == Some((country, row.team)) { OWN_INK } else { INK };
                TableRow { text, ink, mark: None, columns: None }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: FORECAST_COLUMNS.to_vec(),
                rows,
                width: FORECAST_WIDTH,
                centred: -1,
                scroll: 0,
                cross: None,
                graph: None,
            };
        }
        true
    }
}

impl Screen for Statistics {
    fn ids(&self) -> &[u16] {
        &[STATISTICS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        hub::open_bar(cx, view, STATISTICS_SYMBOL);
        if let Some(season) = cx.state.season.as_ref() {
            let caption = hub::manager_title_of(game, season, game.text(CAPTION));
            set(view, dialog, CAPTION_ID, caption);
        }
        *self = Self::default();
        self.fill_categories(cx, view);
        if let Some(records) = cx.state.records.as_ref() {
            self.leagues = leagues(records);
            self.chosen = own_league(cx, &self.leagues);
        }
        self.fill_pages(cx, view);
        if let Some(&first) = self.listed.get(self.category).and_then(|l| l.first()) {
            self.show(cx, view, first);
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Some(item) = hub::on_bar(dialog, target) {
            let outcome = hub::bar_click(cx, view, item, STATISTICS_SYMBOL);
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        self.whistle = false;
        let id = |c: usize| dialog.controls[c].id;
        match target {
            // Another category: its pages, the first shown; on the first
            // category the strip goes back to the own league.
            Target::Row { control: c, .. } if id(c) == Some(CATEGORIES) => {
                let row = view.combo(c).and_then(|combo| combo.selected).unwrap_or(0);
                let Some(&k) = self.categories.get(row) else { return Outcome::Stay };
                if k != self.category {
                    self.category = k;
                    self.page = None;
                    self.fill_pages(cx, view);
                }
                if row == 0 {
                    self.chosen = own_league(cx, &self.leagues);
                }
                if let Some(&first) = self.listed.get(self.category).and_then(|l| l.first()) {
                    self.show(cx, view, first);
                }
            }
            // Another page; leaving the cups' pages of the first category
            // for one of its first eight takes the strip back to the own
            // league.
            Target::Row { control: c, row } if id(c) == Some(PAGES) => {
                let Some(list) = view.list_mut(c) else { return Outcome::Stay };
                let r = list.scroll + row;
                let Some(&entry) = list.keys.get(r) else { return Outcome::Stay };
                list.selected = Some(r);
                let first_category = self.categories.first() == Some(&self.category);
                if first_category && self.row >= 8 && r < 8 {
                    self.chosen = own_league(cx, &self.leagues);
                }
                self.show(cx, view, entry);
                self.row = r;
            }
            // A table page's bar scrolled one row: the view has moved it,
            // the page keeps it when it draws anew.
            Target::Scroll { .. } => {
                if let Some(c) = control(cx.dialog, TABLE)
                    && let Some(t) = view.table(c)
                {
                    self.scrolled = t.scroll;
                }
            }
            // The cross-table's window stepped one column over: its cells
            // stand in the rows, so the page draws them anew.
            Target::Across { .. } => {
                if let Some(c) = control(cx.dialog, TABLE)
                    && let Some(t) = view.table(c)
                {
                    self.across = t.cross.unwrap_or(0);
                }
                self.draw_page(cx, view);
            }
            // The strip steps: its name, and the page drawn anew.
            Target::SliderStep { control: c, .. }
            | Target::SliderAt { control: c, .. }
            | Target::SliderTrack(c)
                if id(c) == Some(STRIP) =>
            {
                self.chosen = view.value(c).map_or(0, |v| v.value.max(0) as usize);
                self.show_strip(cx, view);
                self.draw_page(cx, view);
            }
            _ => {}
        }
        Outcome::Stay
    }

    fn answer(&mut self, _: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}
