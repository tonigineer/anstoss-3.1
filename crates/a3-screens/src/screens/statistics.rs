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
use a3_core::records::{Player, Records, Team};
use a3_core::season::Rules;
use a3_core::statistics::{self, CATEGORY_NAMES, Club, Page};
use a3_files::rch::{Align, Control, Dialog, Kind};
use bevy_math::Rect;

use super::hub::{self, STATISTICS_SYMBOL};
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::bitmap::Picture;
use crate::controls::slider;
use crate::controls::table::{Graph, Shape, TableRow, TableState};
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
/// The Mannschaft category, over the own squad.
const MANNSCHAFT: usize = 1;
/// The Aktuelles category; its Tore pro Spieltag's header and rows
/// (`FUN_00455d6c` with 0x87a350, and 0x87a338) and the closing mean in
/// two columns of its own (0x87a368), text 3875; its Beste Schiris'
/// (`FUN_00457711` with 0x87a488, and 0x87a4a8), the two lists' titles
/// 4713 and 4734.
const AKTUELLES: usize = 3;
/// The europ. Vereinsrangliste's header and rows (`FUN_00457b04` with
/// 0x87a4c8, and 0x87a4f8): rank, club, points in all, from the league,
/// the European cups and the cup.
const CLUB_RANKING_HEADER: u16 = 4717;
const CLUB_RANKING_HEADER_COLUMNS: [(u16, u16); 6] = [
    (R, 0x14),
    (0x1e, 0),
    (C | 0xb9, 0x28),
    (C | 0xf5, 0x28),
    (C | 0x127, 0x28),
    (C | 0x15e, 0x28),
];
const CLUB_RANKING_ROW_COLUMNS: [(u16, u16); 6] = [
    (R, 0x14),
    (0x1e, 0),
    (R | 0xc8, 0x14),
    (R | 0xfa, 0x1e),
    (R | 0x12c, 0x1e),
    (R | 0x15e, 0x1e),
];
const CLUB_RANKING_SHOWN: usize = 25;
/// The Zeitungsmeldungen's header, one column, and its rows' two columns
/// (`FUN_004571ee` with 0x87a448, and {0, 55}); a date as text 1997.
const NEWSPAPER_HEADER: u16 = 4709;
const NEWSPAPER_COLUMNS: [(u16, u16); 2] = [(0, 0), (0x37, 0)];
const SHORT_DATE: u16 = 1997;
/// The Qualifikationsgruppen's headers (`FUN_004555ee`: the World Cup's
/// in the years `FUN_0067bb89` names, else the European Championship's),
/// its strip's group names (0x87caa0), the fixture rows' own columns
/// (0x87a228) and the table's (0x87a1e8).
const WORLD_CUP_QUALIFIERS: u16 = 3535;
const EURO_QUALIFIERS: u16 = 3536;
const GROUP_NAMES: u16 = 3522;
const QUALIFIER_GROUPS: usize = 6;
const QUALIFIER_FIXTURE_COLUMNS: [(u16, u16); 4] =
    [(3, 0), (0x82, 0), (R | 0xf5, 0x2c), (0x122, 0)];
const QUALIFIER_TABLE_COLUMNS: [(u16, u16); 8] = [
    (R, 0x19),
    (0x1e, 0),
    (R | 0xb4, 0x1e),
    (R | 0xd7, 0x1e),
    (R | 0xfa, 0x23),
    (R | 0x11d, 0x23),
    (0x142, 0),
    (0x147, 0),
];
/// The most matchdays the page looks at, and the pairs and lines it
/// writes.
const QUALIFIER_LAST_MATCHDAY: usize = 11;
const QUALIFIER_PAIRS: usize = 3;
const QUALIFIER_LINES: usize = 6;
/// The Serien's header, one column.
const SERIES_HEADER: u16 = 2928;
/// The Elf des Tages' and the Elf des Jahres' headers, and each place's
/// corner on the canvas, from the keeper at the bottom to the strikers at
/// the top (`FUN_00451e0d`, `FUN_004523ce`).
const DAY_ELEVEN_HEADER: u16 = 2975;
const YEAR_ELEVEN_HEADER: u16 = 2976;
const DAY_ELEVEN_PLACES: [(i32, i32); 11] = [
    (194, 305),
    (194, 240),
    (72, 200),
    (318, 200),
    (32, 140),
    (356, 140),
    (62, 55),
    (338, 55),
    (194, 100),
    (100, 0),
    (288, 0),
];
/// The Spielbericht's header and its rows' columns (`FUN_004529f3` with
/// 0x87a190, 0x87a158 and 0x87a128): a label and its text; a label and
/// the two sides' figures; a goal's label, score, scorer and minute.
const REPORT_HEADER: u16 = 2977;
const REPORT_TWO: [(u16, u16); 2] = [(0, 120), (120, 0)];
const REPORT_THREE: [(u16, u16); 3] = [(0, 120), (120, 40), (160, 40)];
const REPORT_GOAL: [(u16, u16); 6] =
    [(0, 112), (R | 112, 15), (130, 0), (135, 15), (150, 0), (R | 300, 30)];
/// Its texts: the league and matchday, the labels, the goals' and the
/// other events' formats, the attendance sold out and not, the weather,
/// the comparison, the money, the rating, the referee, the television's
/// share, the playing time, the best player.
const REPORT_LEAGUE: u16 = 2458;
const REPORT_MATCH: u16 = 2978;
const REPORT_FIRST_LEG: u16 = 2979;
const REPORT_GOALS: u16 = 2980;
const REPORT_MISSED: u16 = 2981;
const REPORT_OWN_GOAL: u16 = 2607;
const REPORT_PENALTY: u16 = 2608;
const REPORT_EVENTS: u16 = 2982;
const REPORT_SOLD_OUT: u16 = 2983;
const REPORT_GATE: u16 = 2984;
const REPORT_WEATHER: u16 = 2985;
const REPORT_COMPARISON: u16 = 2986;
const REPORT_INCOME: u16 = 2991;
const REPORT_MONEY: u16 = 2992;
const REPORT_RATING: u16 = 2995;
const REPORT_REFEREE: u16 = 2996;
const REPORT_REFEREE_LABEL: u16 = 2997;
const REPORT_TV: u16 = 2998;
const REPORT_TIME: u16 = 2999;
const REPORT_BEST: u16 = 3000;
/// "%1%2 Platz".
const PLACE_TEXT: u16 = 1058;
/// The other events' formats by kind (0x8a835c), a weather's text
/// (0x8acfb4, the first two alike), a rating's (0x8a8298), a verdict's
/// (0x8a82d8, in a row from 2551); and the names of the cups by the
/// competition's high bits.
const EVENT_TEXTS: [u16; 6] = [2581, 2602, 2603, 2604, 2605, 2606];
const RATINGS: [u16; 15] =
    [2585, 2586, 2587, 2588, 2589, 2590, 2599, 2591, 2592, 2593, 2594, 2595, 2596, 2597, 2598];
const VERDICTS: u16 = 2551;
const WEATHERS: u16 = 2537;
const CUP_NAMES: [(u32, u16); 5] = [
    (0x4000_0000, 2477),
    (0x3000_0000, 2478),
    (0x0100_0000, 3611),
    (0x1000_0000, 2479),
    (0x2000_0000, 2485),
];
/// A manager no slot holds (`FUN_006a537d`).
const NO_MANAGER: u16 = 1996;
/// The Spielplan's header (`FUN_0044edba`), its columns off the strip,
/// so that nothing of it shows; a matchday's title (`FUN_006e09e6` with
/// "%d%s Spieltag") and where a pair's three cells start (0x8a67b0).
const PLAN_HEADER: u16 = 4747;
const PLAN_HEADER_COLUMNS: [(u16, u16); 3] = [(10000, 40), (10100, 40), (10200, 40)];
const PLAN_TITLE: u16 = 1529;
const PLAN_CELLS: [i32; 3] = [0, 130, 150];
/// The Letzter Spieltag's header and rows (`FUN_0045491f` with 0x87a1d8,
/// and 0x87a1a0): the pairing, its result and half time, its gate.
const LAST_MATCHDAY_HEADER: u16 = 3530;
const LAST_MATCHDAY_HEADER_COLUMNS: [(u16, u16); 2] = [(C, 0xc8), (C | 0x172, 0x32)];
const LAST_MATCHDAY_ROW_COLUMNS: [(u16, u16); 7] = [
    (0, 0),
    (0x8a, 0),
    (R | 0x118, 0xe),
    (0x127, 0),
    (R | 0x145, 0xe),
    (0x154, 0),
    (R | 0x168, 0x32),
];
/// The Tab. sonstige Länder's header and rows (`FUN_00457449` with
/// 0x87a458, and 0x87a470), the top three of each, the first red.
const OTHER_TABLES_HEADER: u16 = 4711;
const OTHER_TABLES_HEADER_COLUMNS: [(u16, u16); 3] = [(0, 0), (0x1e, 0), (C | 0xe6, 0x32)];
const OTHER_TABLES_ROW_COLUMNS: [(u16, u16); 3] = [(0, 0), (0x1e, 0), (R | 0xe6, 0x20)];
const OTHER_TABLES_SHOWN: usize = 3;
const GOALS_PER_MATCHDAY_HEADER: u16 = 3700;
const GOALS_PER_MATCHDAY_HEADER_COLUMNS: [(u16, u16); 3] =
    [(C | 3, 0x32), (C | 0x50, 0x28), (C | 0xa0, 0x28)];
const GOALS_PER_MATCHDAY_ROW_COLUMNS: [(u16, u16); 3] =
    [(R | 0xd, 0x19), (C | 0x50, 0x28), (C | 0xa0, 0x28)];
const MATCHDAY_MEAN: u16 = 3875;
const MATCHDAY_MEAN_COLUMNS: [(u16, u16); 2] = [(0, 0x12c), (0x12c, 0)];
const REFEREES_HEADER: u16 = 4712;
const REFEREES_HEADER_COLUMNS: [(u16, u16); 4] =
    [(0, 0), (0x1e, 0), (C | 0xbe, 0x32), (C | 0xfa, 0x5a)];
const REFEREES_ROW_COLUMNS: [(u16, u16); 4] =
    [(0, 0), (0x1e, 0), (R | 0xc8, 0x14), (R | 0x10e, 0x1e)];
const BEST_REFEREES: u16 = 4713;
const WORST_REFEREES: u16 = 4734;
/// How many referees each list names; the best are read from the last
/// entry down to the fourth, the worst from the first up to the fifth.
const REFEREES_SHOWN: usize = 3;
const BEST_LAST: usize = 39;
const BEST_DOWN_TO: usize = 3;
const WORST_UP_TO: usize = 5;

/// A Mannschaft page: its header text, the header's and the rows'
/// columns, whether it hands the window the squad's size (else
/// 0xffffffff), the players it keeps, the key it sorts them by, and the
/// figures its row writes after the rank and the name.
struct SquadPage {
    header: u16,
    header_columns: &'static [(u16, u16)],
    columns: &'static [(u16, u16)],
    centred: bool,
    keep: fn(&Player) -> bool,
    key: fn(&Player) -> i32,
    figures: fn(&Player) -> String,
}

/// A player's league cards — yellow, red, yellow-red — and their points
/// as the Fairneß weighs them.
fn card_points(p: &Player) -> i32 {
    let [yellow, red, yellow_red] = p.league_cards().map(i32::from);
    red * 5 + yellow_red * 3 + yellow
}

fn scorer_points(p: &Player) -> i32 {
    i32::from(p.goals()[0]) + i32::from(p.assists())
}

/// The Torjäger ewig's header and rows (`FUN_0046a366` with 0x87e258,
/// and 0x87e200): rank, name, goals, matches, their mean, the goals by
/// league level and those of the last three seasons; the header's last
/// two columns stand empty.
const ALL_TIME_HEADER: u16 = 2414;
const ALL_TIME_HEADER_COLUMNS: [(u16, u16); 11] = [
    (C, 0x14),
    (0x19, 0),
    (C | 0x87, 0x28),
    (C | 0xaf, 0x1e),
    (C | 0xcd, 0x28),
    (C | 0xfc, 0x19),
    (C | 0x115, 0x19),
    (C | 0x12e, 0x19),
    (C | 0x151, 0x50),
    (0, 0),
    (0, 0),
];
const ALL_TIME_ROW_COLUMNS: [(u16, u16); 11] = [
    (R, 0x14),
    (0x19, 0),
    (R | 0x84, 0x20),
    (R | 0xa4, 0x20),
    (R | 0xc4, 0x25),
    (R | 0xf7, 0x19),
    (R | 0x110, 0x19),
    (R | 0x129, 0x19),
    (R | 0x149, 0x1b),
    (R | 0x164, 0x1b),
    (R | 0x17f, 0x1b),
];
/// The single row of an empty Torjäger ewig, a string of the exe's own
/// (0x87e908).
const NO_ALL_TIME_SCORER: &str =
    "Keiner Ihrer Spieler hat bisher unter Ihrer Leitung ein Tor erzielt!";
/// The Einsätze ewig's header and rows (`FUN_00468695` with 0x87df80,
/// and 0x87df40): rank, name, matches in all, by league level, in the
/// cup and for his country.
const ALL_TIME_MATCHES_HEADER: u16 = 2407;
const ALL_TIME_MATCHES_HEADER_COLUMNS: [(u16, u16); 8] = [
    (C, 0x14),
    (0x19, 0),
    (C | 0xaa, 0x28),
    (C | 0xd2, 0x28),
    (C | 0xfa, 0x28),
    (C | 0x122, 0x28),
    (R | 0x14a, 0x28),
    (C | 0x17c, 0x37),
];
const ALL_TIME_MATCHES_ROW_COLUMNS: [(u16, u16); 8] = [
    (R, 0x14),
    (0x19, 0),
    (R | 0xaa, 0x23),
    (R | 0xd2, 0x23),
    (R | 0xfa, 0x23),
    (R | 0x122, 0x23),
    (R | 0x14a, 0x23),
    (R | 0x17a, 0x23),
];
/// The Vereinsresultate's header and rows (`FUN_0046ce89` with 0x87e598,
/// and 0x87e558): the weekday and the date, both sides with a dash
/// between, the result's two scores with a colon. A date is text 1776
/// "%1, %2.%3.%4" with the day's weekday among texts 1779 on
/// (`FUN_00676008`, the day of the game modulo seven).
const FIXTURES_HEADER: u16 = 3041;
const FIXTURES_HEADER_COLUMNS: [(u16, u16); 3] = [(C, 0), (C | 0x6e, 0), (C | 0x168, 0x32)];
const FIXTURES_ROW_COLUMNS: [(u16, u16); 8] = [
    (5, 0x1e),
    (R | 0x1d, 0x3c),
    (0x6e, 0),
    (0xeb, 0),
    (0xf0, 0),
    (R | 0x168, 0x14),
    (0x17e, 0),
    (0x183, 0),
];
const DATE_TEXT: u16 = 1776;
const WEEKDAY_TEXTS: u16 = 1779;
/// The Notenschnitt Spieler's header and rows (`FUN_0046e649` with
/// 0x87e6a0, and 0x87e670): the name, the mean grade, at home and its
/// matches, away and its matches.
const GRADES_HEADER: u16 = 3043;
const GRADES_HEADER_COLUMNS: [(u16, u16); 6] = [
    (0, 0),
    (C | 0x91, 0x32),
    (C | 0xcd, 0x1e),
    (C | 0xed, 0x1e),
    (C | 0x113, 0x1e),
    (C | 0x133, 0x1e),
];
const GRADES_ROW_COLUMNS: [(u16, u16); 6] = [
    (0, 0),
    (R | 0x96, 0x1e),
    (R | 0xc8, 0x1e),
    (R | 0xe6, 0x1e),
    (R | 0x10e, 0x1e),
    (R | 0x12c, 0x1e),
];
/// The Wer kam? Wer ging?'s header and rows (`FUN_0046ad89` with
/// 0x87e3a0, and 0x87e360): year, week, kind, name, club, strength,
/// position and fee. The kinds' words (0x87e33c), "SB" for the season's
/// start, the clubs of kinds 5 to 8, "Ausland" for a club of no country,
/// "ablösefrei", the total and the line of a manager without a transfer.
const TRANSFERS_HEADER: u16 = 2628;
const TRANSFERS_HEADER_COLUMNS: [(u16, u16); 8] = [
    (5, 0x19),
    (C | 0x1e, 0x12),
    (C | 0x33, 0x1c),
    (0x51, 0),
    (0xde, 0),
    (C | 0x14f, 0xf),
    (C | 0x15e, 0x14),
    (C | 0x16d, 0x5a),
];
const TRANSFERS_ROW_COLUMNS: [(u16, u16); 8] = [
    (0, 0x1e),
    (R | 0x18, 0x14),
    (R | 0x32, 0x16),
    (0x50, 0),
    (0xde, 0),
    (R | 0x14f, 0xc),
    (C | 0x15e, 0x14),
    (R | 0x172, 0x5a),
];
const TRANSFER_KINDS: [u16; 9] = [0, 2695, 2696, 2697, 2698, 2699, 2700, 2701, 2702];
const SEASON_START: u16 = 3035;
const SEASON_START_WEEK: u8 = 0xff;
const LEFT_FOR: [(u8, u16); 4] = [(5, 2705), (6, 2704), (7, 2706), (8, 2707)];
const ABROAD: u16 = 3492;
const FREE_TRANSFER: u16 = 2703;
const TRANSFER_TOTAL: u16 = 2086;
const NO_TRANSFER: u16 = 3690;
/// The kinds that bring money in: sold and loaned out.
const TRANSFER_INCOME: [u8; 2] = [2, 3];
/// The most rows shown, the newest.
const TRANSFERS_SHOWN: usize = 90;
/// The Fairneßvergleich's header (`FUN_0046a76a` with 0x87e2f0), rows
/// (0x87e2b0) and the totals' own columns (0x87e318), with their texts.
const FAIR_MATCHES_HEADER: u16 = 2416;
const FAIR_MATCHES_HEADER_COLUMNS: [(u16, u16); 3] =
    [(0, 0), (C | 0x140, 0x37), (C | 0x17c, 0x37)];
const FAIR_MATCHES_ROW_COLUMNS: [(u16, u16); 8] = [
    (0, 0),
    (0xa5, 8),
    (0xaa, 0),
    (R | 0x140, 0x1b),
    (0x15e, 0x1e),
    (R | 0x17c, 0x1b),
    (0x19a, 0x1e),
    (0, 0),
];
const FAIR_MATCHES_TOTAL_COLUMNS: [(u16, u16); 5] =
    [(0, 0), (R | 0x78, 0x1e), (0xc8, 0), (R | 0x12c, 0x1e), (0, 0)];
const FAIR_TOTAL: u16 = 2417;
const FAIR_GUESTS: u16 = 2418;
/// The own home matches' rows (COLORREF 0xff), the points' bits and the
/// flag beside them.
const FAIR_HOME_INK: [u8; 3] = [255, 0, 0];
const FAIR_POINTS: u8 = 0x7f;
const FAIR_FLAG: u8 = 0x80;
/// The Chancen's header (`FUN_00469067` with 0x87e060), rows (0x87e020)
/// and the two totals' own columns (0x87e0a0); the totals' texts, which
/// carry their tabs as "\t".
const CHANCES_HEADER: u16 = 2410;
const CHANCES_HEADER_COLUMNS: [(u16, u16); 5] =
    [(R, 0x14), (0x19, 0), (0xaa, 0), (C | 0x140, 0x37), (C | 0x17c, 0x37)];
const CHANCES_ROW_COLUMNS: [(u16, u16); 8] = [
    (R, 0x14),
    (0x19, 0),
    (0xa5, 8),
    (0xaa, 0),
    (R | 0x140, 0x1b),
    (0x15e, 0x1e),
    (R | 0x17c, 0x1b),
    (0x19a, 0x1e),
];
const CHANCES_TOTAL_COLUMNS: [(u16, u16); 4] =
    [(0, 0x78), (0x78, 0x28), (0xaf, 0x78), (0x127, 0x28)];
const OWN_CHANCES: u16 = 2091;
const OWN_RATE: u16 = 2087;
const OWN_NO_RATE: u16 = 2088;
const THEIR_CHANCES: u16 = 2092;
const THEIR_RATE: u16 = 2089;
const THEIR_NO_RATE: u16 = 2090;
/// The own home matches' rows (COLORREF 0xfa).
const HOME_MATCH_INK: [u8; 3] = [0xfa, 0, 0];
/// The Trefferverteilung's header, one column (`FUN_004704ea` with
/// 0x8ec1c8), and its canvas's texts: "Min.", "Mannschaft", "Gegner",
/// the goals and goals against by half, "Heimtore", "Auswärtstore".
const GOAL_MINUTES_HEADER: u16 = 2415;
const MINUTES_TEXT: u16 = 3057;
const TEAM_TEXT: u16 = 3058;
const OPPONENTS_TEXT: u16 = 3059;
const HALF_TEXTS: [u16; 4] = [3060, 3061, 3062, 3063];
const HOME_GOALS_TEXT: u16 = 3064;
const AWAY_GOALS_TEXT: u16 = 3065;
/// Home goals red, away goals blue (COLORREF 0xfa and 0xff0000).
const HOME_GOALS_INK: [u8; 3] = [0xfa, 0, 0];
const AWAY_GOALS_INK: [u8; 3] = [0, 0, 0xff];
/// The minutes that count to the first half.
const FIRST_HALF_MINUTES: usize = 45;
/// The Formverlauf's texts: header, the line's legend, the axes' labels
/// ("Woche", "Formpunkte/20"); its scale of 20 over 300 pixels.
const FORM_HEADER: u16 = 2420;
const FORM_LEGEND: u16 = 3048;
const WEEK_LABEL: u16 = 3047;
const FORM_LABEL: u16 = 3049;
/// The Mannschaftswerte's: header, the six lines' legends, the value
/// axis' label ("Punkte/10%"); a scale of 10 over 300 pixels.
const VALUES_HEADER: u16 = 3495;
const VALUES_LEGENDS: [u16; 6] = [2528, 2529, 2530, 2531, 2532, 3294];
const VALUES_LABEL: u16 = 2533;
/// The six lines' colours (COLORREF 0xff, 0xff0000, 0xff00, 0, 0x808080,
/// 0xffff), the Formverlauf's line the first.
const LINE_INKS: [[u8; 3]; 6] =
    [[255, 0, 0], [0, 0, 255], [0, 255, 0], [0, 0, 0], [128, 128, 128], [255, 255, 0]];
/// The Notenschnitt Mannschaft's texts: header, the two lines' legends,
/// the value axis' label ("Formpunkte/6"); a scale of 6 over 300
/// pixels, a grade worth two and a half (`DAT_00810190`), the lines red
/// and white.
const TEAM_GRADES_HEADER: u16 = 2419;
const TEAM_GRADES_LEGENDS: [u16; 2] = [3044, 3045];
const TEAM_GRADES_LABEL: u16 = 3046;
const GRADE_PIXELS: f64 = 2.5;
const TEAM_GRADES_INKS: [[u8; 3]; 2] = [[255, 0, 0], [255, 255, 255]];
/// The Formverlauf Mannschaft's header and its three lines: series 0 to
/// 2 take the weeks' bytes 2, 1 and 0, red, blue and green, under the
/// legends "Abwehr", "Mittelfeld", "Sturm".
const LINES_FORM_HEADER: u16 = 3496;
const LINES_FORM_SERIES: [(usize, [u8; 3], u16); 3] =
    [(2, [255, 0, 0], 2534), (1, [0, 0, 255], 2535), (0, [0, 255, 0], 2536)];
/// The Tabellenplatzentwicklung's header and its axis' label
/// ("Spieltag"); its graph 310 high.
const PLACES_HEADER: u16 = 3055;
const MATCHDAY_LABEL: u16 = 3056;
const PLACES_HEIGHT: i32 = 310;
/// The clubs' line colours (COLORREF 0xff, 0, 0xff00, 0xffff, 0x808080,
/// 0xff0000).
const CLUB_INKS: [[u8; 3]; 6] =
    [[255, 0, 0], [0, 0, 0], [0, 255, 0], [255, 255, 0], [128, 128, 128], [0, 0, 255]];
/// How many pixels a stored point is worth (`DAT_0080db2c`, 1.5).
const POINT_PIXELS: f64 = 1.5;
const GRAPH_HEIGHT: i32 = 300;
/// The Angstgegner's and Lieblingsgegner's header and rows (0x87e530 and
/// 0x87e4f0 for both): club, games, points, difference, goals and the
/// goals against behind a colon; the sections at home, away and in all
/// (texts 2708 to 2710), and the line of a club with none (2711, 2712).
const FOES_HEADER: u16 = 3040;
const FAVOURITES_HEADER: u16 = 3039;
const OPPONENTS_HEADER_COLUMNS: [(u16, u16); 5] =
    [(0, 0), (C | 0xa0, 0x28), (C | 0xc8, 0x1e), (C | 0xee, 0x19), (C | 0x10c, 0x19)];
const OPPONENTS_ROW_COLUMNS: [(u16, u16); 7] = [
    (0, 0),
    (R | 0x96, 0x23),
    (R | 0xbe, 0x1e),
    (R | 0xe6, 0x19),
    (R | 0x104, 0x19),
    (0x11f, 0),
    (0x124, 0),
];
const OPPONENT_SECTIONS: [(u8, u16); 3] = [
    (statistics::HOME_MATCHES, 2708),
    (statistics::AWAY_MATCHES, 2709),
    (statistics::HOME_MATCHES | statistics::AWAY_MATCHES, 2710),
];
const NO_FOES: u16 = 2711;
const NO_FAVOURITES: u16 = 2712;
/// The most opponents a section names, and how far into the table the
/// favourites are looked for.
const OPPONENTS_SHOWN: usize = 5;
const FAVOURITES_SOUGHT: usize = 10;
/// The Spielertrends' header and rows (`FUN_0046dbe2` with 0x87e638, and
/// 0x87e600): name, age, position, strength, form, D-Form and the goals
/// still needed for an upgrade; each group under its heading, texts 2147
/// to 2151 in COLORREF 0xaa.
const TRENDS_HEADER: u16 = 2421;
const TRENDS_HEADER_COLUMNS: [(u16, u16); 7] = [
    (0, 0xb2),
    (C | 0x9b, 0x28),
    (C | 0xc3, 0x1e),
    (C | 0xe2, 0x1e),
    (C | 0x100, 0x20),
    (C | 0x122, 0x28),
    (C | 0x14a, 0x64),
];
const TRENDS_ROW_COLUMNS: [(u16, u16); 7] = [
    (0, 0xb2),
    (C | 0x9b, 0x28),
    (C | 0xc3, 0x1e),
    (R | 0xf0, 0xa),
    (R | 0xff, 0x18),
    (R | 0x122, 0x20),
    (R | 0x17c, 0x14),
];
const TREND_HEADINGS: [u16; 5] = [2147, 2148, 2149, 2150, 2151];
const HEADING_INK: [u8; 3] = [0xaa, 0, 0];
const KEEPER: u8 = 1;
/// The Spielereigenschaften's header and rows (`FUN_00471109` with
/// 0x87e6d0, and 0x87e6f0): name, position, strength and traits.
const TRAITS_HEADER: u16 = 4739;
const TRAITS_HEADER_COLUMNS: [(u16, u16); 4] =
    [(3, 0), (C | 0x8c, 0x1e), (C | 0xaa, 0x1e), (0xd2, 0)];
const TRAITS_ROW_COLUMNS: [(u16, u16); 4] = [(0, 0), (C | 0x8c, 0x1e), (R | 0xaa, 0x19), (0xd2, 0)];
/// The most traits a row names, and the bit of R+0x271 that hides a
/// player's strength.
const TRAITS_SHOWN: usize = 4;
const STRENGTH_HIDDEN: u8 = 0x40;

/// The Mannschaft pages built, by their codes, with their blocks:
/// - the Torjägerliste (`FUN_00469f05(0)`, 0x87e1d8 and 0x87e160): all
///   by league goals, with cup and European goals;
/// - the Torjägerliste plus (`FUN_00469f05(1)`, 0x87e188 and 0x87e110):
///   the same order, with the last match's goals, the goals by kind and
///   the chances, and the share converted, a float "%2.1f";
/// - the Heimtorjäger (`FUN_00469b36`, 0x87e0e8 and 0x87e0c0): all by
///   their goals at home, then all their goals, with those away and all
///   in brackets;
/// - the Einsätze hintereinander (`FUN_0046895a`, 0x87dfd8 and 0x87dfc0):
///   all by their league matches in a row;
/// - the Scorer (`FUN_0046b87a`, 0x87e460 and 0x87e420): those with goals
///   or assists by their sum, each figure with the last match's in
///   brackets;
/// - the Einsätze (`FUN_004681ce`, 0x87df08 and 0x87ded0): all by their
///   league matches, then the fewer come on, then the fewer taken off,
///   with the D-Form and its Tendenz;
/// - the Fairneß (`FUN_004679ef`, 0x87de48 and 0x87de10): all by their
///   cards' points, the cards, the matchdays banned;
/// - the Vorlagen (`FUN_0046b4cf`, 0x87e400 and 0x87e3e0): those with
///   assists by them, the last match's in brackets;
/// - the Gelbe Karten (`FUN_00467de7`, 0x87dea8 and 0x87de80): all by
///   their yellow cards in the league, then the cup, then Europe.
fn squad_page(code: i16) -> Option<SquadPage> {
    Some(match code {
        statistics::SQUAD_SCORERS_PAGE => SquadPage {
            header: 2413,
            header_columns: &[
                (C, 0x14),
                (0x19, 0),
                (C | 0xd2, 0x1e),
                (C | 0x102, 0x1e),
                (C | 0x127, 0x3c),
            ],
            columns: &[(R, 0x14), (0x19, 0), (R | 0xc8, 0x1e), (R | 0xfa, 0x1e), (R | 0x12c, 0x1e)],
            centred: true,
            keep: |_| true,
            key: |p| i32::from(p.goals()[0]),
            figures: |p| {
                let [league, cup, europe] = p.goals();
                format!("{league}\t{cup}\t{europe}")
            },
        },
        statistics::SQUAD_SCORERS_PLUS_PAGE => SquadPage {
            header: 2412,
            header_columns: &[
                (C, 0x14),
                (0x19, 0),
                (C | 0xa0, 0x1e),
                (C | 0xbc, 0x19),
                (C | 0xe3, 0x19),
                (C | 0x101, 0x19),
                (C | 0x11f, 0x19),
                (C | 0x13d, 0x19),
                (C | 0x15c, 0x1e),
                (C | 0x181, 0x32),
            ],
            columns: &[
                (R, 0x14),
                (0x19, 0),
                (R | 0xa0, 0x14),
                (R | 0xb4, 0x19),
                (R | 0xdc, 0x19),
                (R | 0xfa, 0x19),
                (R | 0x118, 0x19),
                (R | 0x136, 0x19),
                (R | 0x159, 0x1e),
                (R | 0x186, 0x1e),
            ],
            centred: true,
            keep: |_| true,
            key: |p| i32::from(p.goals()[0]),
            figures: |p| {
                let goals = p.goals()[0];
                let [penalty, free_kick, distance, head, _] = p.goal_kinds();
                let chances = p.chances();
                let share = if chances == 0 {
                    0.0
                } else {
                    (f64::from(goals) * 100.0 / f64::from(chances)) as f32
                };
                format!(
                    "{goals}\t{}\t{free_kick}\t{penalty}\t{head}\t{distance}\t{chances}\t{}",
                    p.last_goals(),
                    tenths(f64::from(share))
                )
            },
        },
        statistics::SQUAD_HOME_SCORERS_PAGE => SquadPage {
            header: 2411,
            header_columns: &[
                (R, 0x14),
                (0x19, 0),
                (C | 0xd4, 0x28),
                (C | 0x106, 0x28),
                (C | 0x136, 0x28),
            ],
            columns: &[(R, 0x14), (0x19, 0), (R | 0xc8, 0x28), (R | 0xfa, 0x28), (R | 0x12c, 0x28)],
            centred: true,
            keep: |_| true,
            key: |p| i32::from(p.home_goals()) << 8 | i32::from(p.goals()[0]),
            figures: |p| {
                let (home, all) = (p.home_goals(), p.goals()[0]);
                format!("{home}\t{}\t({all})", i32::from(all) - i32::from(home))
            },
        },
        statistics::SQUAD_IN_A_ROW_PAGE => SquadPage {
            header: 2408,
            header_columns: &[(R, 0x14), (0x19, 0), (C | 0xc8, 0x3c)],
            columns: &[(R, 0x14), (0x19, 0), (R | 0xc8, 0x23)],
            centred: true,
            keep: |_| true,
            key: |p| i32::from(p.matches_in_a_row()),
            figures: |p| p.matches_in_a_row().to_string(),
        },
        statistics::SQUAD_POINTS_PAGE => SquadPage {
            header: 3037,
            header_columns: &[
                (C, 0x14),
                (0x19, 0),
                (C | 0xa0, 0x23),
                (C | 0xbf, 0x1e),
                (C | 0xf3, 0x23),
                (C | 0x10f, 0x1e),
                (C | 0x143, 0x23),
                (C | 0x15f, 0x1e),
            ],
            columns: &[
                (R, 0x14),
                (0x19, 0),
                (R | 0x96, 0x23),
                (R | 0xb9, 0x1e),
                (R | 0xe6, 0x23),
                (R | 0x109, 0x1e),
                (R | 0x136, 0x23),
                (R | 0x159, 0x1e),
            ],
            centred: false,
            keep: |p| scorer_points(p) != 0,
            key: scorer_points,
            figures: |p| {
                let (goals, assists) = (p.goals()[0], p.assists());
                let (last_goals, last_assists) = (p.last_goals(), p.last_assists());
                format!(
                    "{}\t({})\t{goals}\t({last_goals})\t{assists}\t({last_assists})",
                    u16::from(goals) + u16::from(assists),
                    u16::from(last_goals) + u16::from(last_assists)
                )
            },
        },
        statistics::SQUAD_MATCHES_PAGE => SquadPage {
            header: 2406,
            header_columns: &[
                (C, 0x14),
                (0x19, 0),
                (C | 0xa0, 0x28),
                (C | 0xc8, 0x28),
                (C | 0xf0, 0x28),
                (C | 0x11d, 0x37),
                (C | 0x154, 0x3c),
            ],
            columns: &[
                (R, 0x14),
                (0x19, 0),
                (R | 0x96, 0x28),
                (R | 0xbe, 0x23),
                (R | 0xe6, 0x23),
                (R | 0x122, 0x28),
                (R | 0x154, 0x28),
            ],
            centred: true,
            keep: |_| true,
            key: |p| {
                let [came_on, taken_off] = p.league_less().map(i32::from);
                i32::from(p.league_matches()) << 16 | (0xff - came_on) << 8 | (0xff - taken_off)
            },
            figures: |p| {
                let [came_on, taken_off] = p.league_less();
                let d_form = statistics::d_form(p);
                format!(
                    "{}\t{came_on}\t{taken_off}\t{}.{}\t{}",
                    p.league_matches(),
                    d_form / 10,
                    d_form % 10,
                    statistics::form_trend(p)
                )
            },
        },
        statistics::SQUAD_FAIRNESS_PAGE => SquadPage {
            header: 2404,
            header_columns: &[
                (C, 0x14),
                (0x19, 0),
                (C | 0xaf, 0x32),
                (C | 0xe1, 0x32),
                (C | 0x113, 0x32),
                (C | 0x145, 0x32),
                (C | 0x177, 0x46),
            ],
            columns: &[
                (R, 0x14),
                (0x19, 0),
                (C | 0xaf, 0x32),
                (C | 0xe1, 0x32),
                (C | 0x113, 0x32),
                (C | 0x145, 0x32),
                (C | 0x177, 0x46),
            ],
            centred: true,
            keep: |_| true,
            key: card_points,
            figures: |p| {
                let [yellow, red, yellow_red] = p.league_cards();
                let banned = p.banned_matchdays();
                format!("{}\t{red}\t{yellow_red}\t{yellow}\t{banned}", card_points(p))
            },
        },
        statistics::SQUAD_ASSISTS_PAGE => SquadPage {
            header: 3036,
            header_columns: &[(C, 0x18), (0x1a, 0), (C | 0xaf, 0x28), (C | 0xd7, 0x28)],
            columns: &[(R, 0x18), (0x1a, 0), (R | 0xa6, 0x28), (R | 0xcd, 0x28)],
            centred: false,
            keep: |p| p.assists() > 0,
            key: |p| i32::from(p.assists()),
            figures: |p| format!("{}\t({})", p.assists(), p.last_assists()),
        },
        statistics::SQUAD_YELLOW_PAGE => SquadPage {
            header: 2405,
            header_columns: &[
                (R, 0x14),
                (0x19, 0),
                (C | 0xa0, 0x32),
                (C | 0xd7, 0x28),
                (C | 0xff, 0x50),
            ],
            columns: &[(R, 0x14), (0x19, 0), (R | 0x96, 0x28), (R | 0xc8, 0x28), (R | 0xfa, 0x32)],
            centred: true,
            keep: |_| true,
            key: |p| {
                let [league, cup, europe] =
                    [p.league_cards()[0], p.cup_cards()[0], p.europe_cards()[0]].map(i32::from);
                league << 16 | cup << 8 | europe
            },
            figures: |p| {
                format!("{}\t{}\t{}", p.league_cards()[0], p.cup_cards()[0], p.europe_cards()[0])
            },
        },
        _ => return None,
    })
}
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
/// The Treter's header and rows (`FUN_0044b6e0` with 0x8799f0, and
/// 0x8799b8): rank, name, club, points and the three kinds of cards, the
/// last centred.
const TRETER_HEADER: u16 = 0xaa0;
const TRETER_HEADER_COLUMNS: [(u16, u16); 7] = [
    (R, 0x14),
    (0x19, 0),
    (0x82, 0),
    (C | 0x102, 0x1e),
    (C | 0x122, 0x1e),
    (C | 0x140, 0x1e),
    (C | 0x15e, 0x28),
];
const TRETER_ROW_COLUMNS: [(u16, u16); 7] = [
    (R, 0x14),
    (0x19, 0),
    (0x82, 0),
    (R | 0xfa, 0x1e),
    (R | 0x118, 0x1e),
    (R | 0x136, 0x1e),
    (C | 0x15e, 0x28),
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
/// The Mannschaftsübersicht's header and rows (`FUN_0044c9c7` with
/// 0x879b40, and 0x879b28): rank, club and strength.
const OVERVIEW_HEADER: u16 = 0xaa3;
const OVERVIEW_HEADER_COLUMNS: [(u16, u16); 3] = [(R, 0x14), (0x19, 0), (C | 0xc8, 0x3c)];
const OVERVIEW_ROW_COLUMNS: [(u16, u16); 3] = [(R, 0x14), (0x19, 0), (R | 0xc8, 0x28)];
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
/// The Erhaltene Elfmeter's header and rows (`FUN_0044df6d` with
/// 0x879c90, and 0x879cb0): rank, club, penalties awarded and scored; the
/// Häufigste Einwechsl. and Auswechsl. take the same blocks (`FUN_0044d9d1`,
/// `FUN_0044dc9f`).
const PENALTIES_HEADER: u16 = 0x1287;
const SUBS_IN_HEADER: u16 = 0x1285;
const SUBS_OUT_HEADER: u16 = 0x1286;
const PENALTIES_HEADER_COLUMNS: [(u16, u16); 4] =
    [(R, 0x19), (0x1e, 0), (C | 0xc8, 0x32), (C | 0xfa, 0x3c)];
const PENALTIES_ROW_COLUMNS: [(u16, u16); 4] =
    [(R, 0x19), (0x1e, 0), (R | 0xc8, 0x1e), (R | 0xff, 0x1e)];
/// The Plätze über Saisonziel's header and rows (`FUN_0044e23b` with
/// 0x879cd0, and 0x879ce8): rank, club and places, the figure centred.
const TARGET_HEADER: u16 = 0x1288;
const TARGET_HEADER_COLUMNS: [(u16, u16); 3] = [(R, 0x19), (0x1e, 0), (C | 0xb4, 0xa0)];
const TARGET_ROW_COLUMNS: [(u16, u16); 3] = [(R, 0x19), (0x1e, 0), (C | 0xfa, 0x14)];
/// The Häufigste Einw. and Ausw. Spieler's header and rows, both pages
/// alike (`FUN_0044e53f`, `FUN_0044e839` with 0x879d00, and 0x879d20):
/// rank, the player's last name, his club and his count.
const PLAYERS_IN_HEADER: u16 = 0x1289;
const PLAYERS_OUT_HEADER: u16 = 0x1292;
const PLAYERS_HEADER_COLUMNS: [(u16, u16); 4] =
    [(R, 0x19), (0x1e, 0), (0xa0, 0), (C | 0x10e, 0x6e)];
const PLAYERS_ROW_COLUMNS: [(u16, u16); 4] = [(R, 0x19), (0x1e, 0), (0x96, 0), (R | 0x136, 0x14)];
/// The Tore nach Einwechsl.'s header (`FUN_0044fa30` with 0x879e40) and
/// rows (0x879e60), set only once a row stands; without one a single row
/// says so, text 0x1290, in a window left without columns.
const SUB_GOALS_HEADER: u16 = 0x1291;
const SUB_GOALS_HEADER_COLUMNS: [(u16, u16); 4] =
    [(R, 0x19), (0x1e, 0), (C | 0xc8, 0x32), (C | 0xfa, 0x50)];
const SUB_GOALS_ROW_COLUMNS: [(u16, u16); 4] =
    [(R, 0x19), (0x1e, 0), (R | 0xc8, 0x1e), (R | 0xfa, 0x2d)];
const NO_SUB_GOALS: u16 = 0x1290;
/// The Schirilieblinge's header and rows (`FUN_0044c57e` with 0x879af8,
/// and 0x879b10): rank, club and how often the referees favoured it.
const REFEREE_HEADER: u16 = 0xb6f;
const REFEREE_HEADER_COLUMNS: [(u16, u16); 3] = [(R, 0x14), (0x19, 0), (C | 0xdc, 0x64)];
const REFEREE_ROW_COLUMNS: [(u16, u16); 3] = [(R, 0x14), (0x19, 0), (R | 0x104, 0x14)];
/// The Eingesetzte Spieler's header and rows (`FUN_0044d508` with
/// 0x879be8, and 0x879c18): rank, club, the players fielded, the squad,
/// those fielded still in it and their share of it.
const USED_HEADER: u16 = 0xe7b;
const USED_HEADER_COLUMNS: [(u16, u16); 6] = [
    (R, 0x14),
    (0x19, 0),
    (C | 0x87, 0x32),
    (C | 0xb9, 0x5a),
    (C | 0x113, 0x50),
    (C | 0x163, 0x50),
];
const USED_ROW_COLUMNS: [(u16, u16); 6] = [
    (R, 0x14),
    (0x19, 0),
    (C | 0x87, 0x32),
    (C | 0xb9, 0x5a),
    (R | 0x118, 0x28),
    (R | 0x177, 0x20),
];
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

/// A line of the graph window (`FUN_0071dec5`'s two lists and
/// `FUN_0071df4c`'s legend): its points in pixels above the axis, its
/// colour, its legend.
struct GraphLine {
    values: Vec<i32>,
    colour: [u8; 3],
    label: String,
}

/// The graph window's lines as its paint draws them (`FUN_0071e017`), in
/// a window of `size`: the graph as wide as the window less sixteen and
/// `height` high, at most the window's less sixteen (`FUN_0071dc15`).
/// Under it the legend, a coloured box and the label for each line, a
/// row of them under five lines, two rows of four from five on — and
/// were the window too low for both, the graph shrunk to make room. The
/// axes with their arrowheads; a tick on the value axis every
/// `height` / `y_max` pixels from the bottom, rounded; a tick for each
/// point along the other, (width − 10) / the longest line apart; each
/// line from its first point on, a pixel above its value; the axes'
/// labels at the right end below and at the top left.
fn line_graph(
    game: &Game,
    size: (i32, i32),
    height: i32,
    y_max: i32,
    lines: &[GraphLine],
    (x_label, y_label): (&str, &str),
) -> Vec<Shape> {
    let fonts = game.fonts();
    let plain = fonts.get(a3_files::rch::PLAIN_FONT);
    let text_high = plain.map_or(13, |f| f.height() as i32);
    let width_of = |t: &str| plain.map_or(0, |f| f.width(t, false) as i32);
    let round = |v: f64| (v + 0.5) as i32;
    let w = size.0 - 16;
    let full = if height < 0 || height > size.1 - 16 { size.1 - 16 } else { height };
    let (mut high, mut span, mut lift) = (full, full, 0);
    let mut step = span as f32 / y_max as f32;
    let mut scale = 1.0f32;
    let black = [0, 0, 0];
    let mut shapes = Vec::new();
    let shown = lines.iter().filter(|l| !l.values.is_empty()).count() as i32;
    if shown > 0 {
        let (across, legend) =
            if shown < 5 { (w / shown, text_high + 4) } else { (w / 4, 2 * text_high + 8) };
        if size.1 < high + legend {
            scale = (high - legend) as f32 / high as f32;
            high -= legend;
            lift = round(f64::from(lift) * f64::from(scale));
            step *= scale;
            span = round(f64::from(span) * f64::from(scale));
        }
        for (i, line) in lines.iter().take(shown as usize).enumerate() {
            let i = i as i32;
            let (x, y) = if i < 4 {
                (4 + across * i, high + 9)
            } else {
                (4 + across * (i - 4), high + legend / 2 + 9)
            };
            shapes.push(Shape::Fill { at: (x, y + 4), size: (6, text_high), colour: line.colour });
            let text = line.label.clone();
            shapes.push(Shape::Text { at: (x + 8, y + 4), text, colour: black });
        }
    }
    let mut pen = |from: (i32, i32), to: (i32, i32), colour: [u8; 3]| {
        if from != to {
            shapes.push(Shape::Pen { from, to, colour });
        }
    };
    let bottom = full + 9 - lift;
    pen((3, 7), (6, 1), black);
    pen((6, 0), (10, 8), black);
    pen((6, 0), (6, full + 9), black);
    pen((6, full + 9), (6, bottom), black);
    pen((6, bottom), (w + 6, bottom), black);
    pen((w - 1, bottom - 3), (w + 6, bottom), black);
    pen((w + 6, bottom), (w - 2, bottom + 4), black);
    let base = 9 + span - lift;
    let (mut y, mut k) = (base, 0);
    while y >= 9 {
        pen((0, y), (6, y), black);
        y = base - round(f64::from(k) * f64::from(step));
        k += 1;
    }
    let most = lines.iter().map(|l| l.values.len()).max().unwrap_or(0);
    if most > 0 {
        let across = (w - 10) as f32 / most as f32;
        let x_of = |k: usize| round(k as f64 * f64::from(across)) + 6;
        for k in 0..most {
            pen((x_of(k), base - 3), (x_of(k), base + 3), black);
        }
        let y_of = |v: i32| base - (round(f64::from(v) * f64::from(scale)) + 1);
        for line in lines.iter().filter(|l| !l.values.is_empty()) {
            let mut at = (6, y_of(line.values[0]));
            for (k, &v) in line.values.iter().enumerate().skip(1) {
                let to = (x_of(k), y_of(v));
                pen(at, to, line.colour);
                at = to;
            }
        }
    }
    if !x_label.is_empty() {
        let at = (w - width_of(x_label), full - lift + 16);
        shapes.push(Shape::Text { at, text: x_label.to_owned(), colour: black });
    }
    if !y_label.is_empty() {
        shapes.push(Shape::Text { at: (12, 0), text: y_label.to_owned(), colour: black });
    }
    shapes
}

/// An eleven on the canvas (`FUN_00451e0d`, `FUN_004523ce`): each player
/// at his place with the crest of the club he plays for now at its own
/// size left of it (`FUN_00696161`), and in black his name — with his
/// count " (%d)" where the page keeps one — over the club he played for.
fn eleven_page(
    cx: &Cx,
    view: &mut View,
    header: u16,
    country: u16,
    eleven: &[(u16, u8, Option<u8>); 11],
) -> bool {
    let (game, dialog) = (cx.game, cx.dialog);
    let Some(records) = cx.state.records.as_ref() else { return false };
    let Some(c) = control(dialog, TABLE) else { return false };
    set_header(view, dialog, game.text(header).to_owned(), &[(0, 0)]);
    let black = [0, 0, 0];
    let mut shapes = Vec::new();
    for (&(id, team, count), &(x, y)) in eleven.iter().zip(&DAY_ELEVEN_PLACES) {
        let Some(player) = records.player(id) else { continue };
        let [nation, number] = player.club();
        let file = game.world.slot_of(nation.into()).and_then(|slot| {
            game.rules.crest(&game.world, Some(records), (slot, usize::from(number)))
        });
        let crest = file.map(|file| Picture { black_clear: false, ..Picture::file(file) });
        if let Some(image) = crest.and_then(|p| p.load(game).ok()) {
            shapes.push(Shape::Image { at: (x - 32, y), image });
        }
        let mut text = records.known_as(id).unwrap_or_default();
        if let Some(n) = count {
            text += &format!(" ({n})");
        }
        shapes.push(Shape::Text { at: (x + 2, y + 2), text, colour: black });
        let text = game.club_name(Some(records), country as u8, i16::from(team));
        shapes.push(Shape::Text { at: (x + 2, y + 17), text, colour: black });
    }
    if let Some(table) = view.table_mut(c) {
        *table = TableState {
            columns: Vec::new(),
            centred: -1,
            rows: Vec::new(),
            width: ROW_WIDTH,
            scroll: 0,
            cross: None,
            graph: Some(Graph { bars: Vec::new(), steps: 0, shapes, bar: None }),
        };
    }
    true
}

/// A name from the pool's ids as `FUN_00637cbb` spells it, "%s %s", or
/// just one of the two where the other's id is 0.
fn spelled(records: &Records, first: u16, last: u16) -> String {
    match (first, last) {
        (0, _) => records.name(last).to_owned(),
        (_, 0) => records.name(first).to_owned(),
        _ => format!("{} {}", records.name(first), records.name(last)),
    }
}

/// A double as "%2.1f" writes it: to one place, a half rounded up — the
/// second place of its exact decimal expansion decides.
fn tenths(value: f64) -> String {
    fixed(value, 1)
}

/// A double to `places` places as printf writes it, a half rounded up
/// by the next digit of its exact decimal expansion.
fn fixed(value: f64, places: usize) -> String {
    let exact = format!("{:.60}", value.abs());
    let (whole, fraction) = exact.split_once('.').unwrap_or((&exact, "0"));
    let digit = |k: usize| u64::from(fraction.as_bytes().get(k).map_or(0, |b| b - b'0'));
    let scale = 10u64.pow(places as u32);
    let mut n = whole.parse::<u64>().unwrap_or(0) * scale;
    for k in 0..places {
        n += digit(k) * 10u64.pow((places - 1 - k) as u32);
    }
    if digit(places) >= 5 {
        n += 1;
    }
    let sign = if value < 0.0 && n > 0 { "-" } else { "" };
    format!("{sign}{}.{:0places$}", n / scale, n % scale)
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
            statistics::QUALIFIER_STRIP => {
                (0..QUALIFIER_GROUPS).map(|g| text(GROUP_NAMES + g as u16)).collect()
            }
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
                | statistics::TRETER_PAGE
                | statistics::EIGENTORE_PAGE))
                if self.category == LIGA =>
            {
                self.liga_page(cx, view, code)
            }
            Some(code) if self.category == MANNSCHAFT && squad_page(code).is_some() => {
                self.squad_stats_page(cx, view, code)
            }
            Some(statistics::SQUAD_TRAITS_PAGE) if self.category == MANNSCHAFT => {
                self.traits_page(cx, view)
            }
            Some(code @ (statistics::SQUAD_FOES_PAGE | statistics::SQUAD_FAVOURITES_PAGE))
                if self.category == MANNSCHAFT =>
            {
                self.opponents_page(cx, view, code == statistics::SQUAD_FOES_PAGE)
            }
            Some(
                code @ (statistics::SQUAD_FORM_PAGE
                | statistics::SQUAD_VALUES_PAGE
                | statistics::SQUAD_LINES_FORM_PAGE),
            ) if self.category == MANNSCHAFT => self.weekly_graph_page(cx, view, code),
            Some(statistics::SQUAD_TEAM_GRADES_PAGE) if self.category == MANNSCHAFT => {
                self.team_grades_page(cx, view)
            }
            Some(statistics::SQUAD_FAIRNESS_MATCHES_PAGE) if self.category == MANNSCHAFT => {
                self.fair_matches_page(cx, view)
            }
            Some(statistics::SQUAD_CHANCES_PAGE) if self.category == MANNSCHAFT => {
                self.chances_page(cx, view)
            }
            Some(statistics::SQUAD_PLACES_PAGE) if self.category == MANNSCHAFT => {
                self.places_page(cx, view)
            }
            Some(statistics::SQUAD_GOAL_MINUTES_PAGE) if self.category == MANNSCHAFT => {
                self.goal_minutes_page(cx, view)
            }
            Some(statistics::SQUAD_TRENDS_PAGE) if self.category == MANNSCHAFT => {
                self.trends_page(cx, view)
            }
            Some(statistics::SQUAD_TRANSFERS_PAGE) if self.category == MANNSCHAFT => {
                self.transfers_page(cx, view)
            }
            Some(statistics::SQUAD_GRADES_PAGE) if self.category == MANNSCHAFT => {
                self.grades_page(cx, view)
            }
            Some(statistics::SQUAD_FIXTURES_PAGE) if self.category == MANNSCHAFT => {
                self.fixtures_page(cx, view)
            }
            Some(statistics::SQUAD_ALL_TIME_PAGE) if self.category == MANNSCHAFT => {
                self.all_time_page(cx, view)
            }
            Some(statistics::SQUAD_ALL_TIME_MATCHES_PAGE) if self.category == MANNSCHAFT => {
                self.all_time_matches_page(cx, view)
            }
            Some(statistics::GOALS_PER_MATCHDAY_PAGE) if self.category == AKTUELLES => {
                self.goals_per_matchday_page(cx, view)
            }
            Some(statistics::CLUB_RANKING_PAGE) if self.category == AKTUELLES => {
                self.club_ranking_page(cx, view)
            }
            Some(statistics::SERIES_PAGE) if self.category == AKTUELLES => {
                self.series_page(cx, view)
            }
            Some(statistics::QUALIFIERS_PAGE) if self.category == AKTUELLES => {
                self.qualifiers_page(cx, view)
            }
            Some(statistics::LAST_MATCHDAY_PAGE) if self.category == AKTUELLES => {
                self.last_matchday_page(cx, view)
            }
            Some(statistics::REPORT_PAGE) if self.category == AKTUELLES => {
                self.report_page(cx, view)
            }
            Some(statistics::PLAN_PAGE) if self.category == AKTUELLES => {
                self.plan_page(cx, view, None)
            }
            Some(statistics::YEAR_ELEVEN_PAGE) if self.category == AKTUELLES => {
                self.year_eleven_page(cx, view)
            }
            Some(statistics::DAY_ELEVEN_PAGE) if self.category == AKTUELLES => {
                self.day_eleven_page(cx, view)
            }
            Some(statistics::OTHER_TABLES_PAGE) if self.category == AKTUELLES => {
                self.other_tables_page(cx, view)
            }
            Some(statistics::NEWSPAPER_PAGE) if self.category == AKTUELLES => {
                self.newspaper_page(cx, view)
            }
            Some(statistics::REFEREES_PAGE) if self.category == AKTUELLES => {
                self.referees_page(cx, view)
            }
            Some(statistics::FAIRNESS_PAGE) if self.category == LIGA => {
                self.fairness_page(cx, view)
            }
            Some(statistics::CHANCEN_PAGE) if self.category == LIGA => {
                self.chance_page(cx, view)
            }
            Some(code @ (statistics::PLAYERS_IN_PAGE | statistics::PLAYERS_OUT_PAGE))
                if self.category == LIGA =>
            {
                self.substitutes_page(cx, view, code == statistics::PLAYERS_OUT_PAGE)
            }
            Some(
                code @ (statistics::TARGET_PAGE
                | statistics::REFEREE_PAGE
                | statistics::PENALTIES_PAGE
                | statistics::SUBS_IN_PAGE
                | statistics::SUBS_OUT_PAGE
                | statistics::SUB_GOALS_PAGE),
            ) if self.category == LIGA => self.club_list_page(cx, view, code),
            Some(
                code @ (statistics::OVERVIEW_PAGE
                | statistics::AGE_PAGE
                | statistics::FOREIGN_PAGE
                | statistics::USED_PAGE),
            )
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
                graph: Some(Graph { bars, steps: axis / 25, shapes: Vec::new(), bar: None }),
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
            statistics::TRETER_PAGE => (
                TRETER_HEADER,
                &TRETER_HEADER_COLUMNS[..],
                &TRETER_ROW_COLUMNS[..],
                statistics::treter(records, country, level),
                4,
                false,
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

    /// The Mannschaft category's pages over the own squad (`FUN_004677f5`):
    /// the players the page keeps in its order, "%d.\t%s\t" with the
    /// rank and the name a player goes by (`FUN_006feb4d`), then its
    /// figures, all rows black.
    fn squad_stats_page(&self, cx: &Cx, view: &mut View, code: i16) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        let Some(page) = squad_page(code) else { return false };
        set_header(view, dialog, game.text(page.header).to_owned(), page.header_columns);
        let ids = statistics::squad_by(records, team, page.keep, page.key);
        let rows: Vec<TableRow> = ids
            .iter()
            .enumerate()
            .map(|(i, &id)| {
                let name = records.known_as(id).unwrap_or_default();
                let figures = records.player(id).map(page.figures).unwrap_or_default();
                let text = format!("{}.\t{name}\t{figures}", i + 1);
                TableRow { text, ink: INK, mark: None, columns: None }
            })
            .collect();
        let centred = if page.centred { team.players().len() as i32 } else { -1 };
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: page.columns.to_vec(),
                centred,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Formverlauf (`FUN_0046f282`), the Mannschaftswerte
    /// (`FUN_0046f51d`) or the Formverlauf Mannschaft (`FUN_0046fbb3`): the
    /// club's weeks as lines in the graph window — its form, its six
    /// values, or its three lines' form — each byte worth one and a half
    /// pixels, cut, the respect's line starting a week later; under one
    /// header column.
    fn weekly_graph_page(&self, cx: &Cx, view: &mut View, code: i16) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        let Some(c) = control(dialog, TABLE) else { return false };
        let pixels = |b: u8| (f64::from(b) * POINT_PIXELS) as i32;
        let (header, lines, y_max, value_label) = if code == statistics::SQUAD_LINES_FORM_PAGE {
            let weeks = statistics::weekly_lines(records, team);
            let lines = LINES_FORM_SERIES
                .iter()
                .map(|&(byte, colour, legend)| GraphLine {
                    values: weeks.iter().map(|w| pixels(w[byte])).collect(),
                    colour,
                    label: game.text(legend).to_owned(),
                })
                .collect();
            (LINES_FORM_HEADER, lines, 20, FORM_LABEL)
        } else if code == statistics::SQUAD_FORM_PAGE {
            let values = statistics::weekly_form(records, team).into_iter().map(pixels).collect();
            let label = game.text(FORM_LEGEND).to_owned();
            let line = GraphLine { values, colour: LINE_INKS[0], label };
            (FORM_HEADER, vec![line], 20, FORM_LABEL)
        } else {
            let weeks = statistics::weekly_values(records, team);
            let lines = (0..VALUES_LEGENDS.len())
                .map(|k| GraphLine {
                    values: weeks.iter().skip(usize::from(k == 5)).map(|w| pixels(w[k])).collect(),
                    colour: LINE_INKS[k],
                    label: game.text(VALUES_LEGENDS[k]).to_owned(),
                })
                .collect();
            (VALUES_HEADER, lines, 10, VALUES_LABEL)
        };
        set_header(view, dialog, game.text(header).to_owned(), &[(0, 0)]);
        let rect = dialog.controls[c].rect;
        let size = (rect.width() as i32, rect.height() as i32);
        let labels = (game.text(WEEK_LABEL), game.text(value_label));
        let shapes = line_graph(game, size, GRAPH_HEIGHT, y_max, &lines, labels);
        if let Some(table) = view.table_mut(c) {
            *table = TableState {
                columns: Vec::new(),
                centred: -1,
                rows: Vec::new(),
                width: ROW_WIDTH,
                scroll: 0,
                cross: None,
                graph: Some(Graph { bars: Vec::new(), steps: 0, shapes, bar: None }),
            };
        }
        true
    }

    /// The Notenschnitt Mannschaft (`FUN_0046ee67`): the club's mean grade
    /// and its opponents', matchday by matchday, as lines — each point 300
    /// less two and a half times the grade, cut, so the better grades
    /// stand higher — the first matchday's twice, as the page hands it in
    /// once before the loop.
    fn team_grades_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        let Some(c) = control(dialog, TABLE) else { return false };
        set_header(view, dialog, game.text(TEAM_GRADES_HEADER).to_owned(), &[(0, 0)]);
        let point = |b: u8| GRAPH_HEIGHT - (f64::from(b) * GRADE_PIXELS) as i32;
        let lines: Vec<GraphLine> = statistics::team_grades(records, team)
            .iter()
            .zip(TEAM_GRADES_LEGENDS.iter().zip(TEAM_GRADES_INKS))
            .map(|(grades, (&legend, colour))| {
                let first = grades.first().copied().unwrap_or(0);
                let points = std::iter::once(first).chain(grades.iter().copied());
                GraphLine {
                    values: points.map(point).collect(),
                    colour,
                    label: game.text(legend).to_owned(),
                }
            })
            .collect();
        let rect = dialog.controls[c].rect;
        let size = (rect.width() as i32, rect.height() as i32);
        let labels = (game.text(WEEK_LABEL), game.text(TEAM_GRADES_LABEL));
        let shapes = line_graph(game, size, GRAPH_HEIGHT, 6, &lines, labels);
        if let Some(table) = view.table_mut(c) {
            *table = TableState {
                columns: Vec::new(),
                centred: -1,
                rows: Vec::new(),
                width: ROW_WIDTH,
                scroll: 0,
                cross: None,
                graph: Some(Graph { bars: Vec::new(), steps: 0, shapes, bar: None }),
            };
        }
        true
    }

    /// The Fairneßvergleich (`FUN_0046a76a`): each league match played,
    /// "%s\t-\t%s\t%d\t:%d\t%d\t:%d" — the sides, their fair-play points
    /// and goals, home first, the own home matches red — and two lines of
    /// totals in five columns of their own, as coded: the club's points
    /// and those of the matches its byte flags; then the opponents' points
    /// in those, and in the others the opponents' whole bytes, flag and
    /// all. The window is handed the season's fixtures and two more.
    fn fair_matches_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        let header = game.text(FAIR_MATCHES_HEADER).to_owned();
        set_header(view, dialog, header, &FAIR_MATCHES_HEADER_COLUMNS);
        let country = team.country();
        let own = game.club_name(Some(records), country, i16::from(team.number()));
        let mut rows: Vec<TableRow> = Vec::new();
        let (mut own_all, mut own_flagged, mut theirs_flagged, mut theirs_other) = (0, 0, 0, 0);
        for m in statistics::fair_matches(records, &game.calendar, team) {
            let other = game.club_name(Some(records), country, m.opponent);
            let [own_byte, their_byte] = m.points;
            let (own_points, their_points) = (own_byte & FAIR_POINTS, their_byte & FAIR_POINTS);
            let [h, a] = m.goals;
            let (sides, points, ink) = if m.home {
                ((&own, &other), (own_points, their_points), FAIR_HOME_INK)
            } else {
                ((&other, &own), (their_points, own_points), INK)
            };
            let text = format!(
                "{}\t-\t{}\t{}\t:{}\t{h}\t:{a}",
                sides.0, sides.1, points.0, points.1
            );
            rows.push(TableRow { text, ink, mark: None, columns: None });
            own_all += i32::from(own_points);
            if own_byte >= FAIR_FLAG {
                own_flagged += i32::from(own_points);
                theirs_flagged += i32::from(their_points);
            } else {
                theirs_other += i32::from(their_byte);
            }
        }
        let text = |id: u16| game.text(id).replace("\\t", "\t");
        let lines = [
            super::finances::printf(&text(FAIR_TOTAL), &[&own_all, &own_flagged]),
            super::finances::printf(&text(FAIR_GUESTS), &[&theirs_flagged, &theirs_other]),
        ];
        for line in lines {
            let columns = Some(FAIR_MATCHES_TOTAL_COLUMNS.to_vec());
            rows.push(TableRow { text: line, ink: INK, mark: None, columns });
        }
        let fixtures = a3_core::opponent::fixture_days(records, &game.calendar, team).len();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: FAIR_MATCHES_ROW_COLUMNS.to_vec(),
                centred: fixtures as i32 + 2,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Chancen (`FUN_00469067`): each league match played, by the
    /// schedule's matchdays, "%d.\t%s\t-\t%s\t%d\t: %d\t%d\t: %d" — the
    /// matchday, the sides, their chances and goals, home first, the own
    /// home matches red (`FUN_006de3dd` with 0xfa); then an empty row and
    /// the two sides' totals in four columns of their own: the chances,
    /// and the goals scored of every hundred as "%3.1f" with a "%" — the
    /// opponents' line, without a chance, is "Gegnerischer Schnitt:\t0%"
    /// alone, as the original copies that text over the chances. The
    /// window is handed the rows; none without a match played.
    fn chances_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        set_header(view, dialog, game.text(CHANCES_HEADER).to_owned(), &CHANCES_HEADER_COLUMNS);
        let text = |id: u16| game.text(id).replace("\\t", "\t");
        let country = team.country();
        let own = game.club_name(Some(records), country, i16::from(team.number()));
        let matches = statistics::chance_matches(records, team);
        let mut rows: Vec<TableRow> = Vec::new();
        let (mut chances, mut goals) = ([0i32; 2], [0i32; 2]);
        for m in &matches {
            let other = game.club_name(Some(records), country, m.opponent);
            let [own_chances, their_chances] = m.chances;
            let (sides, side_chances, ink) = if m.home {
                ((&own, &other), (own_chances, their_chances), HOME_MATCH_INK)
            } else {
                ((&other, &own), (their_chances, own_chances), INK)
            };
            let [h, a] = m.goals;
            let text = format!(
                "{}.\t{}\t-\t{}\t{}\t: {}\t{h}\t: {a}",
                m.matchday, sides.0, sides.1, side_chances.0, side_chances.1
            );
            rows.push(TableRow { text, ink, mark: None, columns: None });
            let (own_goals, their_goals) = if m.home { (h, a) } else { (a, h) };
            chances[0] += i32::from(own_chances);
            chances[1] += i32::from(their_chances);
            goals[0] += i32::from(own_goals);
            goals[1] += i32::from(their_goals);
        }
        rows.push(TableRow { text: String::new(), ink: INK, mark: None, columns: None });
        let rate = |side: usize| tenths(f64::from(goals[side]) * 100.0 / f64::from(chances[side]));
        let printf =
            |id: u16, arg: &dyn std::fmt::Display| super::finances::printf(&text(id), &[arg]);
        let own_line = if chances[0] == 0 {
            format!("{}{}%", printf(OWN_CHANCES, &0), text(OWN_NO_RATE))
        } else {
            let rate = text(OWN_RATE).replace("%3.1f", &rate(0));
            format!("{}{rate}%", printf(OWN_CHANCES, &chances[0]))
        };
        let their_line = if chances[1] == 0 {
            format!("{}%", text(THEIR_NO_RATE))
        } else {
            let rate = text(THEIR_RATE).replace("%3.1f", &rate(1));
            format!("{}{rate}%", printf(THEIR_CHANCES, &chances[1]))
        };
        for line in [own_line, their_line] {
            let columns = Some(CHANCES_TOTAL_COLUMNS.to_vec());
            rows.push(TableRow { text: line, ink: INK, mark: None, columns });
        }
        let played = matches.len() as i32;
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: CHANCES_ROW_COLUMNS.to_vec(),
                centred: if played == 0 { 0 } else { played + 3 },
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Tabellenplatzentwicklung (`FUN_0047004e`): a line for each
    /// human manager's club, its league place after each matchday played,
    /// the first place at the top — the graph's height shared out among
    /// the places of the biggest league, a float, and both ends cut
    /// (`ftol` without rounding) — the club's name its legend.
    fn places_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(c) = control(dialog, TABLE) else { return false };
        set_header(view, dialog, game.text(PLACES_HEADER).to_owned(), &[(0, 0)]);
        let rect = dialog.controls[c].rect;
        let size = (rect.width() as i32, rect.height() as i32);
        let clubs: Vec<(u8, &Team)> = statistics::human_clubs(records)
            .into_iter()
            .filter_map(|(country, club)| {
                Some((country, records.team(country.into(), u16::try_from(club).ok()?)?))
            })
            .collect();
        let most = clubs
            .iter()
            .map(|&(country, team)| statistics::league_size(records, country.into(), team.level()))
            .max()
            .unwrap_or(0)
            - 1;
        let high = PLACES_HEIGHT.min(size.1 - 16);
        let share = high as f32 / most as f32;
        let top = (f64::from(most) * f64::from(share)) as i32;
        let lines: Vec<GraphLine> = clubs
            .iter()
            .enumerate()
            .map(|(k, &(country, team))| GraphLine {
                values: statistics::places_by_matchday(team)
                    .iter()
                    .take_while(|&&place| place != 0)
                    .map(|&place| top - (f64::from(place - 1) * f64::from(share)) as i32)
                    .collect(),
                colour: CLUB_INKS[k % CLUB_INKS.len()],
                label: game.club_name(Some(records), country, i16::from(team.number())),
            })
            .collect();
        let labels = (game.text(MATCHDAY_LABEL), "");
        let shapes = if most > 0 {
            line_graph(game, size, PLACES_HEIGHT, most, &lines, labels)
        } else {
            Vec::new()
        };
        if let Some(table) = view.table_mut(c) {
            *table = TableState {
                columns: Vec::new(),
                centred: -1,
                rows: Vec::new(),
                width: ROW_WIDTH,
                scroll: 0,
                cross: None,
                graph: Some(Graph { bars: Vec::new(), steps: 0, shapes, bar: None }),
            };
        }
        true
    }

    /// The Trefferverteilung (`FUN_004704ea`): no table, but the graph
    /// window as a canvas — for the club's goals over the upper axis and
    /// the goals against over the lower (half the height below its top
    /// twenty pixels, and twenty above its bottom), a bar two pixels wide
    /// for each of a hundred minutes, ten pixels a goal, the home goals red
    /// and the away goals blue on top; each axis from 20 to 440 with a
    /// tick and its minute every ten ("1" for the first), "Min." at its
    /// end, its caption 119 above; and at the top the goals of either
    /// half, minutes 1 to 45 and the rest, and the two colours' key.
    fn goal_minutes_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        let Some(c) = control(dialog, TABLE) else { return false };
        set_header(view, dialog, game.text(GOAL_MINUTES_HEADER).to_owned(), &[(0, 0)]);
        let bottom = dialog.controls[c].rect.height() as i32;
        let axes = [(bottom - 20) / 2 + 20, bottom - 20];
        let minutes = statistics::goal_minutes(records, team);
        let mut shapes = Vec::new();
        let mut halves = [[0u16; 2]; 2];
        for m in 0..minutes[0].len() {
            let x = 20 + 4 * m as i32;
            for (side, &y) in axes.iter().enumerate() {
                let [home, away] = minutes[side][m].map(i32::from);
                if home != 0 {
                    let colour = HOME_GOALS_INK;
                    let (at, size) = ((x, y - home * 10), (2, home * 10));
                    shapes.push(Shape::Fill { at, size, colour });
                }
                if away != 0 {
                    let at = (x, y - (home + away) * 10);
                    shapes.push(Shape::Fill { at, size: (2, away * 10), colour: AWAY_GOALS_INK });
                }
                halves[side][usize::from(m >= FIRST_HALF_MINUTES)] += (home + away) as u16;
            }
        }
        for &y in &axes {
            shapes.push(Shape::Line { from: (20, y), to: (440, y), colour: INK });
        }
        let text = |at: (i32, i32), text: String| Shape::Text { at, text, colour: INK };
        for m in (0..=100).step_by(10) {
            let x = 4 * m + 20;
            for &y in &axes {
                shapes.push(Shape::Line { from: (x, y + 1), to: (x, y + 8), colour: INK });
            }
            let label = if m == 0 { "1".to_owned() } else { m.to_string() };
            for &y in &axes {
                shapes.push(text((x + 3, y + 1), label.clone()));
            }
        }
        for &y in &axes {
            shapes.push(text((447, y + 1), game.text(MINUTES_TEXT).to_owned()));
        }
        shapes.push(text((171, axes[0] - 119), game.text(TEAM_TEXT).to_owned()));
        shapes.push(text((181, axes[1] - 119), game.text(OPPONENTS_TEXT).to_owned()));
        let goals = [halves[0][0], halves[0][1], halves[1][0], halves[1][1]];
        for (k, (&id, n)) in HALF_TEXTS.iter().zip(goals).enumerate() {
            let at = (if k < 2 { 1 } else { 151 }, if k % 2 == 0 { 21 } else { 41 });
            shapes.push(text(at, format!("{}{n}", game.text(id))));
        }
        for (y, id, colour) in
            [(21, HOME_GOALS_TEXT, HOME_GOALS_INK), (41, AWAY_GOALS_TEXT, AWAY_GOALS_INK)]
        {
            shapes.push(text((351, y), game.text(id).to_owned()));
            shapes.push(Shape::Fill { at: (342, y), size: (5, 12), colour });
        }
        if let Some(table) = view.table_mut(c) {
            *table = TableState {
                columns: Vec::new(),
                centred: -1,
                rows: Vec::new(),
                width: ROW_WIDTH,
                scroll: 0,
                cross: None,
                graph: Some(Graph { bars: Vec::new(), steps: 0, shapes, bar: None }),
            };
        }
        true
    }

    /// The Angstgegner (`FUN_0046cb0e`) or the Lieblingsgegner
    /// (`FUN_0046c518`): for the matches at home, away and in all the
    /// club's record against each club (`FUN_0040c97b`), and of those it
    /// played, not itself, up to five who took more points than they gave
    /// from the table's end (`FUN_0046c8d8`) — or who gave more, among the
    /// table's first ten (`FUN_0046c2e6`). A section stands after a
    /// separator under its heading in COLORREF 0xaa,
    /// "%s\t%d\t%d\t%d\t%d\t:\t%d" its rows, an empty row after; with no
    /// section at all one column says so. From the top of the window.
    fn opponents_page(&self, cx: &Cx, view: &mut View, foes: bool) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        let header = if foes { FOES_HEADER } else { FAVOURITES_HEADER };
        set_header(view, dialog, game.text(header).to_owned(), &OPPONENTS_HEADER_COLUMNS);
        let own = i16::from(team.number());
        let country = team.country();
        let mut rows = Vec::new();
        for &(mode, heading) in &OPPONENT_SECTIONS {
            let table = statistics::opponents(records, &game.calendar, team, mode);
            let counts = |e: &&statistics::Opponent| e.games != 0 && e.team != own;
            let picked: Vec<statistics::Opponent> = if foes {
                table
                    .iter()
                    .rev()
                    .filter(counts)
                    .filter(|e| e.given > e.points)
                    .take(OPPONENTS_SHOWN)
                    .copied()
                    .collect()
            } else {
                table
                    .iter()
                    .take(FAVOURITES_SOUGHT)
                    .filter(counts)
                    .filter(|e| e.points > e.given)
                    .take(OPPONENTS_SHOWN)
                    .copied()
                    .collect()
            };
            if picked.is_empty() {
                continue;
            }
            let text = game.text(heading).to_owned();
            let columns = Some(GROUP_COLUMNS.to_vec());
            rows.push(TableRow { text, ink: HEADING_INK, mark: None, columns });
            for e in &picked {
                let club = game.club_name(Some(records), country, e.team);
                let text = format!(
                    "{club}\t{}\t{}\t{}\t{}\t:\t{}",
                    e.games, e.points, e.diff, e.goals, e.against
                );
                rows.push(TableRow { text, ink: INK, mark: None, columns: None });
            }
            rows.push(TableRow { text: String::new(), ink: INK, mark: None, columns: None });
        }
        let mut columns = OPPONENTS_ROW_COLUMNS.to_vec();
        if rows.is_empty() {
            let text = game.text(if foes { NO_FOES } else { NO_FAVOURITES }).to_owned();
            rows.push(TableRow { text, ink: INK, mark: None, columns: None });
            rows.push(TableRow { text: String::new(), ink: INK, mark: None, columns: None });
            columns = GROUP_COLUMNS.to_vec();
        }
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns,
                centred: -1,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Spielertrends (`FUN_0046dbe2`): the squad in five groups, almost
    /// surely down first, each that has players under its heading and in
    /// the squad's order, "%s\t%d\t%s\t%s\t%d\t%2.1f\t%s" — the age on
    /// the day, the position's abbreviation, the strength or "?", the form
    /// today, the D-Form as a tenth, and the goals for an upgrade, "-" for
    /// a keeper or from strength 12; the window handed headings and rows.
    fn trends_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(today) = cx.state.season.as_ref().map(|s| s.date()) else { return false };
        let Some(team) = own_team(cx) else { return false };
        set_header(view, dialog, game.text(TRENDS_HEADER).to_owned(), &TRENDS_HEADER_COLUMNS);
        let length = statistics::league_length(records, team);
        let table = &game.rules.statistics.upgrade;
        let squad: Vec<(u16, &Player, usize)> = team
            .players()
            .into_iter()
            .filter_map(|id| {
                let p = records.player(id)?;
                let age = statistics::age_on(p, today);
                Some((id, p, statistics::trend_group(p, age, statistics::d_form(p))))
            })
            .collect();
        let mut rows = Vec::new();
        for (group, &heading) in TREND_HEADINGS.iter().enumerate() {
            if !squad.iter().any(|&(_, _, g)| g == group) {
                continue;
            }
            let text = game.text(heading).to_owned();
            rows.push(TableRow { text, ink: HEADING_INK, mark: None, columns: None });
            for &(id, p, _) in squad.iter().filter(|&&(_, _, g)| g == group) {
                let name = records.known_as(id).unwrap_or_default();
                let kind = p.positions()[0];
                let position = game.rules.pitch.abbreviations.get(usize::from(kind));
                let position = game.text(position.copied().unwrap_or(0) as u16);
                let strength = if p.picks() & STRENGTH_HIDDEN != 0 {
                    "?".to_owned()
                } else {
                    p.strength().to_string()
                };
                let d_form = statistics::d_form(p);
                let goals = match statistics::goals_to_upgrade(table, p, length) {
                    Some(n) if kind != KEEPER => n.to_string(),
                    _ => "-".to_owned(),
                };
                let text = format!(
                    "{name}\t{}\t{position}\t{strength}\t{}\t{}\t{goals}",
                    statistics::age_on(p, today),
                    statistics::form_today(p),
                    tenths(f64::from(d_form) / 10.0)
                );
                rows.push(TableRow { text, ink: INK, mark: None, columns: None });
            }
        }
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: TRENDS_ROW_COLUMNS.to_vec(),
                centred: rows.len() as i32,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Wer kam? Wer ging? (`FUN_0046ad89`): the club's transfers under
    /// its manager, the last ninety, oldest first, from the top of the
    /// window, "%d\t%s\t%s\t%s\t%s\t%d\t%s\t%s" — the week one more than
    /// stored, the names spelled out, the club as the kind has it, the fee
    /// a thousand times as money. A separator, then the total, the fees of
    /// sales and loans out less all others; without a transfer an empty
    /// row before the separator and a line that says so.
    fn transfers_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        set_header(
            view,
            dialog,
            game.text(TRANSFERS_HEADER).to_owned(),
            &TRANSFERS_HEADER_COLUMNS,
        );
        let transfers = statistics::transfers(records, team);
        let first = transfers.len().saturating_sub(TRANSFERS_SHOWN);
        let mut total = 0i64;
        let mut rows: Vec<TableRow> = transfers[first..]
            .iter()
            .map(|t| {
                if TRANSFER_INCOME.contains(&t.kind) {
                    total += i64::from(t.fee);
                } else {
                    total -= i64::from(t.fee);
                }
                let week = if t.week == SEASON_START_WEEK {
                    game.text(SEASON_START).to_owned()
                } else {
                    (u16::from(t.week) + 1).to_string()
                };
                let kind = TRANSFER_KINDS.get(usize::from(t.kind)).map_or("", |&id| game.text(id));
                let club = match LEFT_FOR.iter().find(|(k, _)| *k == t.kind) {
                    Some(&(_, id)) => game.text(id).to_owned(),
                    None if t.country == 0 => game.text(ABROAD).to_owned(),
                    None => game.club_name(Some(records), t.country, t.club),
                };
                let position = game.rules.pitch.abbreviations.get(usize::from(t.position));
                let position = game.text(position.copied().unwrap_or(0) as u16);
                let fee = if t.fee == 0 {
                    game.text(FREE_TRANSFER).to_owned()
                } else {
                    super::playerinfo::money(game, i64::from(t.fee) * 1000)
                };
                let text = format!(
                    "{}\t{week}\t{kind}\t{}\t{club}\t{}\t{position}\t{fee}",
                    t.year,
                    spelled(records, t.first, t.last),
                    t.strength
                );
                TableRow { text, ink: INK, mark: None, columns: None }
            })
            .collect();
        let closing = if transfers.is_empty() {
            rows.push(TableRow { text: String::new(), ink: INK, mark: None, columns: None });
            game.text(NO_TRANSFER).to_owned()
        } else {
            let sum = super::playerinfo::money(game, total * 1000);
            super::finances::printf(game.text(TRANSFER_TOTAL), &[&sum])
        };
        // `FUN_006df44c`: the closing line in a column of its own.
        let columns = Some(GROUP_COLUMNS.to_vec());
        rows.push(TableRow { text: closing, ink: INK, mark: None, columns });
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: TRANSFERS_ROW_COLUMNS.to_vec(),
                centred: -1,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Notenschnitt Spieler (`FUN_0046e649`): the squad by its mean
    /// grade, the best first, "%s\t%s\t%s\t%d\t%s\t%d" — each grade
    /// halved and "%1.1f", "-" without one. The page keeps a record of 20
    /// bytes a player — id, mean, sums and counts — and its sort swaps the
    /// records whole (Ghidra shows them as two arrays 8 bytes apart).
    fn grades_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        set_header(view, dialog, game.text(GRADES_HEADER).to_owned(), &GRADES_HEADER_COLUMNS);
        let grades = statistics::squad_grades(records, &game.calendar, team);
        let mut sorted = grades;
        for i in 0..sorted.len() {
            for j in i + 1..sorted.len() {
                if sorted[j].1.mean() < sorted[i].1.mean() {
                    sorted.swap(i, j);
                }
            }
        }
        let half = |sum: i16, n: i16| tenths(f64::from(f32::from(sum) / f32::from(n)) / 2.0);
        let rows: Vec<TableRow> = sorted
            .iter()
            .filter(|(id, _)| *id != 0)
            .map(|&(id, g)| {
                let name = records.known_as(id).unwrap_or_default();
                let all =
                    if g.sum == 0 { "-".to_owned() } else { tenths(f64::from(g.mean()) / 2.0) };
                let home = if g.home[1] < 1 { "-".to_owned() } else { half(g.home[0], g.home[1]) };
                let away = if g.away[1] < 1 { "-".to_owned() } else { half(g.away[0], g.away[1]) };
                let text = format!("{name}\t{all}\t{home}\t{}\t{away}\t{}", g.home[1], g.away[1]);
                TableRow { text, ink: INK, mark: None, columns: None }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: GRADES_ROW_COLUMNS.to_vec(),
                centred: team.players().len() as i32,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Vereinsresultate (`FUN_0046ce89`): the club's league fixtures
    /// through the season (`FUN_0062af24`), a blank row before the second
    /// half's first, each "%s\t%s" of the date split after its comma, then
    /// "%s\t%s\t-\t%s\t" with the home side first, and "%d\t:\t%d" once
    /// the matchday is played; black, the window handed one row more than
    /// there are fixtures.
    fn fixtures_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        set_header(view, dialog, game.text(FIXTURES_HEADER).to_owned(), &FIXTURES_HEADER_COLUMNS);
        let country = team.country();
        let fixtures = a3_core::opponent::fixture_days(records, &game.calendar, team);
        let league = League::of(records, country.into(), team.level());
        let place = usize::from(team.league_place());
        let own = game.club_name(Some(records), country, i16::from(team.number()));
        let mut rows = Vec::new();
        for (i, f) in fixtures.iter().enumerate() {
            if i == fixtures.len() / 2 {
                rows.push(TableRow { text: String::new(), ink: INK, mark: None, columns: None });
            }
            let date = records.globals.date_of(f.day as i16).map_or_else(String::new, |d| {
                use chrono::Datelike;
                let weekday = game.text(WEEKDAY_TEXTS + (f.day % 7) as u16);
                game.text(DATE_TEXT)
                    .replace("%1", weekday)
                    .replace("%2", &d.day().to_string())
                    .replace("%3", &d.month().to_string())
                    .replace("%4", &d.year().to_string())
            });
            // `CString::Left` up to the comma and `Mid` past its space.
            let date = match date.find(',') {
                Some(k) => format!("{}\t{}", &date[..=k], date.get(k + 2..).unwrap_or("")),
                None => format!("{date}\t"),
            };
            let other = game.club_name(Some(records), country, f.opponent);
            let (home, away) = if f.home { (&own, &other) } else { (&other, &own) };
            let mut text = format!("{date}\t{home}\t-\t{away}\t");
            let result = league.as_ref().and_then(|l| l.result(i + 1, place));
            if let Some((_, _, Some((h, a)))) = result {
                text.push_str(&format!("{h}\t:\t{a}"));
            }
            rows.push(TableRow { text, ink: INK, mark: None, columns: None });
        }
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: FIXTURES_ROW_COLUMNS.to_vec(),
                centred: fixtures.len() as i32 + 1,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Torjäger ewig (`FUN_0046a366`): the club's scorers under its
    /// manager as stored, ranked by a counter,
    /// "%d.\t%s\t%d\t%d\t%s\t%d\t%d\t%d\t%d\t%d\t%d" — the names
    /// spelled out, the goals in all and the matches, their mean as
    /// "%2.1f" of a double (none without a match), the goals by level and
    /// by season; the window handed one row more than there are. Without
    /// one the page says so in a single row and sets no columns.
    fn all_time_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        set_header(view, dialog, game.text(ALL_TIME_HEADER).to_owned(), &ALL_TIME_HEADER_COLUMNS);
        let scorers = statistics::all_time_scorers(records, team);
        let mut rows: Vec<TableRow> = scorers
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let name = spelled(records, s.first, s.last);
                let total: i16 = s.goals.iter().sum();
                let mean = if s.matches == 0 {
                    String::new()
                } else {
                    tenths(f64::from(total) / f64::from(s.matches))
                };
                let [first_level, second_level, third_level] = s.goals;
                let [now, last_season, before] = s.seasons;
                let text = format!(
                    "{}.\t{name}\t{total}\t{}\t{mean}\t{first_level}\t{second_level}\t\
                     {third_level}\t{now}\t{last_season}\t{before}",
                    i + 1,
                    s.matches
                );
                TableRow { text, ink: INK, mark: None, columns: None }
            })
            .collect();
        let mut columns = ALL_TIME_ROW_COLUMNS.to_vec();
        if rows.is_empty() {
            let text = NO_ALL_TIME_SCORER.to_owned();
            rows.push(TableRow { text, ink: INK, mark: None, columns: None });
            columns.clear();
        }
        let centred = if scorers.is_empty() { 1 } else { scorers.len() as i32 + 1 };
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns,
                centred,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Einsätze ewig (`FUN_00468695`): the club's most fielded players
    /// under its manager as stored, ranked by a counter,
    /// "%d.\t%s\t%d\t%d\t%d\t%d\t%d\t%d" with the names spelled out;
    /// the window handed the entries holding an id.
    fn all_time_matches_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        let header = game.text(ALL_TIME_MATCHES_HEADER).to_owned();
        set_header(view, dialog, header, &ALL_TIME_MATCHES_HEADER_COLUMNS);
        let (held, entries) = statistics::all_time_matches(records, team);
        let rows: Vec<TableRow> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let name = spelled(records, e.first, e.last);
                let [first_level, second_level, third_level] = e.levels;
                let text = format!(
                    "{}.\t{name}\t{}\t{first_level}\t{second_level}\t{third_level}\t{}\t{}",
                    i + 1,
                    e.matches,
                    e.cup,
                    e.internationals
                );
                TableRow { text, ink: INK, mark: None, columns: None }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: ALL_TIME_MATCHES_ROW_COLUMNS.to_vec(),
                centred: held as i32,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Spielereigenschaften (`FUN_00471109`): the squad by name,
    /// "%s\t%s\t%s\t%s" — the name, the abbreviation of his position's
    /// kind (`FUN_00731356`), his strength or "?" where it is hidden
    /// (`FUN_006ff294`), and the first four of his traits by bit of
    /// R+0x1c8, joined by ", " — black, the window handed the squad's
    /// size.
    fn traits_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        set_header(view, dialog, game.text(TRAITS_HEADER).to_owned(), &TRAITS_HEADER_COLUMNS);
        let words = &game.rules.words;
        let rows: Vec<TableRow> = statistics::squad_by_name(records, team)
            .iter()
            .filter_map(|&id| {
                let p = records.player(id)?;
                let name = records.known_as(id).unwrap_or_default();
                let kind = p.positions()[0];
                let abbreviation =
                    game.rules.pitch.abbreviations.get(usize::from(kind)).copied().unwrap_or(0);
                let strength = if p.picks() & STRENGTH_HIDDEN != 0 {
                    "?".to_owned()
                } else {
                    p.strength().to_string()
                };
                let traits = p.traits()[0];
                let named: Vec<&str> = (0..16)
                    .filter(|b| traits & 1 << b != 0)
                    .take(TRAITS_SHOWN)
                    .map(|b| game.text(words.traits[0].get(b).copied().unwrap_or(0)))
                    .collect();
                let text = format!(
                    "{name}\t{}\t{strength}\t{}",
                    game.text(abbreviation as u16),
                    named.join(", ")
                );
                Some(TableRow { text, ink: INK, mark: None, columns: None })
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: TRAITS_ROW_COLUMNS.to_vec(),
                centred: team.players().len() as i32,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Tore pro Spieltag (`FUN_00455d6c`): the league of the strip,
    /// each matchday played with its goals and their mean a pair —
    /// "%d.\t%d\t%2.1f", the goals over half the clubs, cut — and, once a
    /// matchday is played, "Schnitt/Spieltag %3.1f" in two columns of its
    /// own; black, from the top of the window.
    fn goals_per_matchday_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        let Some(league) = League::of(records, country, level) else { return false };
        let header = game.text(GOALS_PER_MATCHDAY_HEADER).to_owned();
        set_header(view, dialog, header, &GOALS_PER_MATCHDAY_HEADER_COLUMNS);
        let goals = statistics::goals_per_matchday(&league);
        let pairs = (league.teams() / 2) as i32;
        let mut rows: Vec<TableRow> = goals
            .iter()
            .enumerate()
            .map(|(md, &g)| {
                let mean = tenths(f64::from(g) / f64::from(pairs));
                let text = format!("{}.\t{g}\t{mean}", md + 1);
                TableRow { text, ink: INK, mark: None, columns: None }
            })
            .collect();
        if !goals.is_empty() {
            let total: i32 = goals.iter().sum();
            let mean = tenths(f64::from(total) / goals.len() as f64);
            let text = game.text(MATCHDAY_MEAN).replace("%3.1f", &mean);
            let columns = Some(MATCHDAY_MEAN_COLUMNS.to_vec());
            rows.push(TableRow { text, ink: INK, mark: None, columns });
        }
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: GOALS_PER_MATCHDAY_ROW_COLUMNS.to_vec(),
                centred: -1,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The europ. Vereinsrangliste (`FUN_00457b04`): the European club
    /// ranking the game keeps (`DAT_00b080d8`), up to 25 until an entry
    /// without a country, "%d.\t%s\t%3.2f\t%3.2f\t%3.2f\t%3.2f" of the
    /// points in all, from the league, the European cups and the cup, the
    /// own club red; the window handed 25 rows.
    fn club_ranking_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let header = game.text(CLUB_RANKING_HEADER).to_owned();
        set_header(view, dialog, header, &CLUB_RANKING_HEADER_COLUMNS);
        let own = own_team(cx).map(|t: &Team| (t.country(), i16::from(t.number())));
        let rows: Vec<TableRow> = statistics::club_ranking(records)
            .iter()
            .take(CLUB_RANKING_SHOWN)
            .take_while(|e| e.country != 0)
            .enumerate()
            .map(|(i, e)| {
                let club = game.club_name(Some(records), e.country, e.club);
                let [total, league, europe, cup] = e.points.map(|v| fixed(f64::from(v), 2));
                let text = format!("{}.\t{club}\t{total}\t{league}\t{europe}\t{cup}", i + 1);
                let ink = if own == Some((e.country, e.club)) { OWN_INK } else { INK };
                TableRow { text, ink, mark: None, columns: None }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: CLUB_RANKING_ROW_COLUMNS.to_vec(),
                centred: CLUB_RANKING_SHOWN as i32,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Qualifikationsgruppen (`FUN_004555ee`): the strip's group, its
    /// pairs of the matchday its leader has last played — "%s\t- %s\t%d\t:%d",
    /// or "%s\t- %s" before the match, in four columns of their own — and
    /// its table, six lines "%d.\t%s\t%d\t%d\t%d\t%d\t:\t%d"; red where a
    /// human manager coaches the country; from the top of the window.
    fn qualifiers_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let world_cup = matches!(records.globals.start_year() % 4, 0 | 1);
        let header = if world_cup { WORLD_CUP_QUALIFIERS } else { EURO_QUALIFIERS };
        set_header(view, dialog, game.text(header).to_owned(), &[(0, 0)]);
        let Some(group) = statistics::NationsGroup::of(records, self.chosen.min(6)) else {
            return false;
        };
        let nation = |c: u16| game.world.nation(c).to_owned();
        let ink = |coached: bool| if coached { OWN_INK } else { INK };
        let table = group.table();
        let leader_games = table.first().map_or(0, |l| i32::from(l.games));
        let md = (leader_games - 1).clamp(0, QUALIFIER_LAST_MATCHDAY as i32) as usize;
        let mut rows = Vec::new();
        let mut sides = (0u16, 0u16);
        for g in 0..QUALIFIER_PAIRS {
            // A pair past the group's countries leaves the last sides.
            if let Some((h, a)) = group.pair(md, g) {
                sides = (group.country(h), group.country(a));
            }
            let (home, away) = sides;
            let slot = |c: u16| (0..group.count()).find(|&k| group.country(k) == c);
            let result = slot(home).zip(slot(away)).and_then(|(h, a)| group.result(h, a));
            let text = match result {
                Some((gh, ga)) => format!("{}\t- {}\t{gh}\t:{ga}", nation(home), nation(away)),
                None => format!("{}\t- {}", nation(home), nation(away)),
            };
            let coached = [home, away].iter().any(|&c| statistics::coached_nation(records, c));
            let columns = Some(QUALIFIER_FIXTURE_COLUMNS.to_vec());
            rows.push(TableRow { text, ink: ink(coached), mark: None, columns });
        }
        for (i, l) in table.iter().take(QUALIFIER_LINES).enumerate() {
            let text = format!(
                "{}.\t{}\t{}\t{}\t{}\t{}\t:\t{}",
                i + 1,
                nation(l.country),
                l.games,
                l.points,
                l.goals - l.against,
                l.goals,
                l.against
            );
            let coached = statistics::coached_nation(records, l.country);
            rows.push(TableRow { text, ink: ink(coached), mark: None, columns: None });
        }
        if let Some(t) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *t = TableState {
                columns: QUALIFIER_TABLE_COLUMNS.to_vec(),
                centred: -1,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Serien (`FUN_0044ffb3`): a sentence for each of the club's runs
    /// that has reached its mark, and its biggest wins and defeats, black,
    /// in no columns — the page sets only a tab stop at 140 its texts
    /// never use (`FUN_006df3e7`); from the top of the window.
    fn series_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(team) = own_team(cx) else { return false };
        set_header(view, dialog, game.text(SERIES_HEADER).to_owned(), &[(0, 0)]);
        let printf = super::finances::printf;
        let rows: Vec<TableRow> = statistics::runs(records, team)
            .iter()
            .map(|run| {
                let text = match *run {
                    statistics::Run::Count(id, n) => printf(game.text(id), &[&n]),
                    statistics::Run::Record(id, club, [own, other]) => {
                        let name = game.club_name(Some(records), team.country(), club);
                        printf(game.text(id), &[&name, &own, &other])
                    }
                };
                TableRow { text, ink: INK, mark: None, columns: None }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: Vec::new(),
                centred: -1,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Letzter Spieltag (`FUN_0045491f`): the league of the strip's
    /// last matchday played, each pair "%s %s\t- %s %s\t%d\t:%d\t(%d\t:%d)\t%s"
    /// — the sides with their places before it, "(%d.)" from the second
    /// matchday on (`FUN_0069105e`), the result and half time as the matrix
    /// holds them, and the home club's last gate (`FUN_00474005`) — red
    /// where the own club plays; from the top of the window.
    fn last_matchday_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        let Some(league) = League::of(records, country, level) else { return false };
        let header = game.text(LAST_MATCHDAY_HEADER).to_owned();
        set_header(view, dialog, header, &LAST_MATCHDAY_HEADER_COLUMNS);
        let own = own_team(cx).map(|t: &Team| (u16::from(t.country()), i16::from(t.number())));
        let md = league.played().saturating_sub(1);
        let place = |slot: usize| match league.place_before(slot) {
            0 => String::new(),
            p => format!("({p}.)"),
        };
        let gate = |team: i16| {
            let g = u16::try_from(team)
                .ok()
                .and_then(|t| records.team(country, t))
                .map_or(0, statistics::last_gate);
            match g {
                ..1000 => g.to_string(),
                1000..1_000_000 => format!("{}.{:03}", g / 1000, g % 1000),
                _ => format!("{}.{:03}.{:03}", g / 1_000_000, g % 1_000_000 / 1000, g % 1000),
            }
        };
        let rows: Vec<TableRow> = league
            .games(md)
            .iter()
            .map(|&(h, a, result)| {
                let (home, away) = (league.team(h), league.team(a));
                let [gh, ga, hh, ha] = result.unwrap_or([0xff; 4]);
                let text = format!(
                    "{} {}\t- {} {}\t{gh}\t:{ga}\t({hh}\t:{ha})\t{}",
                    game.club_name(Some(records), country as u8, home),
                    place(h),
                    game.club_name(Some(records), country as u8, away),
                    place(a),
                    gate(home)
                );
                let ours = own == Some((country, home)) || own == Some((country, away));
                TableRow { text, ink: if ours { OWN_INK } else { INK }, mark: None, columns: None }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: LAST_MATCHDAY_ROW_COLUMNS.to_vec(),
                centred: -1,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The country a countries' strip stands at: the main country, then
    /// the more countries in their order.
    fn strip_country(&self, records: &Records) -> u16 {
        let globals = &records.globals;
        let more = usize::try_from(globals.more_count()).unwrap_or(0);
        (match self.chosen {
            0 => globals.main_country(),
            k if k > more => globals.main_country(),
            k => globals.more_countries()[k - 1],
        }) as u16
    }

    /// The Elf des Tages (`FUN_00451e0d`): the league's team of the day on
    /// the canvas.
    fn day_eleven_page(&self, cx: &Cx, view: &mut View) -> bool {
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        let eleven = statistics::day_eleven(records, country, level).map(|(id, t)| (id, t, None));
        eleven_page(cx, view, DAY_ELEVEN_HEADER, country, &eleven)
    }

    /// The Elf des Jahres (`FUN_004523ce`): the country's team of the
    /// season so far, each name with how often he made the day's.
    fn year_eleven_page(&self, cx: &Cx, view: &mut View) -> bool {
        let Some(records) = cx.state.records.as_ref() else { return false };
        let country = self.strip_country(records);
        let eleven = statistics::year_eleven(records, country).map(|(id, t, n)| (id, t, Some(n)));
        eleven_page(cx, view, YEAR_ELEVEN_HEADER, country, &eleven)
    }

    /// The Spielbericht (`FUN_004529f3`): the own club's last match, row
    /// by row in black, each row with columns of its own. The competition,
    /// the pairing with the league places and the score, the first leg,
    /// the goals, the other events, the attendance, the weather, the
    /// comparison of the sides, the money, the rating, the referee, the
    /// television's share, the playing time and the best player. Before
    /// a first match only the header changes.
    fn report_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        set_header(view, dialog, game.text(REPORT_HEADER).to_owned(), &[(0, 0)]);
        let Some(report) = own_team(cx).and_then(|t| statistics::match_report(records, t)) else {
            return true;
        };
        let text = |id: u16| game.text(id).replace("\\t", "\t");
        // Each part in for the next "%s" or "%d", in their order.
        let fill = |format: String, parts: &[&str]| {
            let mut out = String::new();
            let mut rest = format.as_str();
            for part in parts {
                let Some(at) = rest.find("%s").into_iter().chain(rest.find("%d")).min() else {
                    break;
                };
                out.push_str(&rest[..at]);
                out.push_str(part);
                rest = &rest[at + 2..];
            }
            out + rest
        };
        let money = |v: i32| super::playerinfo::money(game, i64::from(v));
        let name = |id: u16| records.known_as(id).unwrap_or_default();
        let minute = |m: u8| format!("{m}.");
        let mut rows = Vec::new();
        let mut push = |text: String, columns: &[(u16, u16)]| {
            rows.push(TableRow { text, ink: INK, mark: None, columns: Some(columns.to_vec()) });
        };
        let [(hc, hn), (ac, an)] = report.sides();
        let club = |c: u8, n: i16| game.club_name(Some(records), c, n);
        let (home, away) = (club(hc, hn), club(ac, an));
        let competition = report.competition();
        let pairing = if competition < 0x3f {
            let team = |c: u8, n: i16| records.team(c.into(), n as u16);
            let league = team(hc, hn).and_then(|t| League::of(records, hc.into(), t.level()));
            let played = league.as_ref().map_or(0, League::played);
            let league_name = league.map(|l| l.name()).unwrap_or_default();
            let matchday = fill(text(REPORT_LEAGUE), &[&league_name, &minute(played as u8)]);
            push(format!("\t{matchday}"), &REPORT_TWO);
            let place = |c: u8, n: i16| {
                let p = team(c, n).map_or(0, Team::table_place);
                text(PLACE_TEXT).replacen("%1", &p.to_string(), 1).replacen("%2", ".", 1)
            };
            format!("{home} ({}) - {away} ({})", place(hc, hn), place(ac, an))
        } else {
            let [champions, uefa] = records.countries.cup_names();
            let cup = match CUP_NAMES.iter().find(|&&(bits, _)| competition & 0x7f00_0000 == bits) {
                Some(&(_, id)) => text(id),
                None => match competition & 0x7f00_0700 {
                    0x5000_0400 | 0x5000_0700 => champions,
                    0x5000_0100 => uefa,
                    0x5000_0200 => text(2481),
                    0x7000_0000 => text(2483),
                    0x6000_0000 => text(2484),
                    _ => String::new(),
                },
            };
            push(format!("\t{cup}"), &REPORT_TWO);
            format!("{home} - {away}")
        };
        let [gh, ga] = report.goals();
        let suffix = match report.extra() {
            (true, _) => text(EXTRA_TIME),
            (_, true) => text(PENALTIES),
            _ => String::new(),
        };
        push(format!("{}{pairing}   {gh}:{ga} {suffix}", text(REPORT_MATCH)), &REPORT_TWO);
        if let Some([h, a]) = report.first_leg() {
            push(fill(text(REPORT_FIRST_LEG), &[&h.to_string(), &a.to_string()]), &REPORT_TWO);
        }
        // The label goes with the first goal, or with nothing where its
        // scorer is none.
        for (k, goal) in report.scorers().into_iter().enumerate() {
            let label = if k == 0 { text(REPORT_GOALS) } else { "\t".to_owned() };
            if goal.id == 0 {
                continue;
            }
            let [h, a] = goal.score.map(|g| g.to_string());
            let row = match goal.kind {
                5 => fill(text(REPORT_OWN_GOAL), &[&h, &a, &name(goal.id), &minute(goal.minute)]),
                0 => fill(text(REPORT_PENALTY), &[&h, &a, &name(goal.id), &minute(goal.minute)]),
                ..100 => format!("{h}\t:\t{a}\t{}\t({})", name(goal.id), minute(goal.minute)),
                100 => fill(text(REPORT_MISSED), &[&name(goal.id), ""]),
                _ => format!("{h}\t:\t{a}\t{}", name(goal.id)),
            };
            push(label + &row, &REPORT_GOAL);
        }
        for (k, event) in report.events().into_iter().enumerate() {
            let label = if k == 0 { text(REPORT_EVENTS) } else { "\t".to_owned() };
            let format = EVENT_TEXTS.get(usize::from(event.kind)).map(|&id| text(id));
            let [first, second] = event.ids;
            let at = minute(event.minute);
            let row = match (event.kind, format) {
                (1, Some(f)) if first != 0 && second != 0 => {
                    fill(f, &[&name(first as u16), &name(second as u16), &at])
                }
                (2 | 3, Some(f)) if first != 0 => fill(f, &[&name(first as u16), &at]),
                (4, Some(f)) if first != 0 => fill(f, &[&name(first as u16)]),
                (5, Some(f)) => {
                    let slot = usize::try_from(first - 1).ok();
                    let slots = records.globals.slots();
                    let held = slot.filter(|&s| slots.get(s).is_some_and(|&n| n > 0));
                    let manager = match held.and_then(|s| records.managers.get(s)) {
                        Some(m) => format!("{} {}", m.first_name(), m.last_name()),
                        None => text(NO_MANAGER),
                    };
                    fill(f, &[&manager, &at])
                }
                _ => String::new(),
            };
            if !row.is_empty() {
                push(label + &row, &REPORT_TWO);
            }
        }
        let (country, number) = report.venue();
        let stadium = a3_core::opponent::stadium(records, country, number);
        let capacity = stadium.map_or(0, |s| s.capacity);
        let gate = if report.attendance() < capacity { REPORT_GATE } else { REPORT_SOLD_OUT };
        push(fill(text(gate), &[&report.attendance().to_string()]), &REPORT_TWO);
        let weather = match report.weather() {
            0 | 1 => WEATHERS + 1,
            w => WEATHERS + u16::from(w),
        };
        push(text(REPORT_WEATHER) + &text(weather), &REPORT_TWO);
        push(text(REPORT_COMPARISON), &REPORT_THREE);
        for (k, [h, a]) in report.comparison().into_iter().enumerate() {
            let format = text(REPORT_COMPARISON + 1 + k as u16);
            push(fill(format, &[&h.to_string(), &a.to_string()]), &REPORT_THREE);
        }
        push(fill(text(REPORT_INCOME), &[&money(report.income())]), &REPORT_TWO);
        for (k, [h, a]) in report.money().into_iter().enumerate() {
            if h == 0 && a == 0 {
                continue;
            }
            let side = |v: i32| if v == 0 { "-".to_owned() } else { money(v) };
            push(fill(text(REPORT_MONEY + k as u16), &[&side(h), &side(a)]), &REPORT_TWO);
        }
        let rating = usize::try_from(report.rating()).ok().and_then(|r| RATINGS.get(r));
        let rating = rating.map_or(String::new(), |&id| text(id));
        push(fill(text(REPORT_RATING), &[&rating]), &REPORT_TWO);
        let referee = records.referee(report.referee()).map_or("", |(name, ..)| name);
        let verdict = text(VERDICTS + report.verdict().max(0) as u16);
        let judged = fill(text(REPORT_REFEREE), &[referee, &verdict]);
        push(text(REPORT_REFEREE_LABEL) + &judged, &REPORT_TWO);
        if report.tv_share() > 0 {
            let share = text(REPORT_TV).replacen("%d", &report.tv_share().to_string(), 1);
            push(share.replace("%%", "%"), &REPORT_TWO);
        }
        push(fill(text(REPORT_TIME), &[&report.minutes().to_string(), ""]), &REPORT_TWO);
        if report.best() != 0 {
            push(fill(text(REPORT_BEST), &[&name(report.best()), ""]), &REPORT_TWO);
        }
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: Vec::new(),
                centred: -1,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Spielplan (`FUN_0044edba`): the league's pairs two matchdays at
    /// a time, in a window of its own with its bar (`CScrSpielplanWnd`,
    /// `FUN_006e0649`): from the two the last matchday played falls in,
    /// or where the bar stands (`FUN_006e09e6`). Each matchday's title,
    /// then a row for each pair, the rows closer the more clubs the
    /// league has, and a row's gap after.
    fn plan_page(&self, cx: &Cx, view: &mut View, at: Option<usize>) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        let Some(league) = League::of(records, country, level) else { return false };
        let Some(c) = control(dialog, TABLE) else { return false };
        set_header(view, dialog, game.text(PLAN_HEADER).to_owned(), &PLAN_HEADER_COLUMNS);
        let count = league.matchdays();
        let played = league.played().clamp(1, count.max(1));
        let at = at.unwrap_or((played - 1) / 2);
        let step = match league.teams() {
            24 => 13,
            22 => 14,
            20 => 15,
            _ => 16,
        };
        let name = |place| game.club_name(Some(records), country as u8, league.team(place));
        let (black, title) = ([0, 0, 0], game.text(PLAN_TITLE));
        let mut shapes = Vec::new();
        let mut y = 0;
        for md in 2 * at..2 * at + 2 {
            let text = title.replacen("%d", &(md + 1).to_string(), 1).replacen("%s", ".", 1);
            shapes.push(Shape::Text { at: (PLAN_CELLS[0], y), text, colour: black });
            y += 20;
            for (home, away) in league.pairs(md) {
                let cells = [name(home), "-".to_owned(), name(away)];
                for (x, text) in PLAN_CELLS.into_iter().zip(cells) {
                    shapes.push(Shape::Text { at: (x, y), text, colour: black });
                }
                y += step;
            }
            y += step;
        }
        if let Some(table) = view.table_mut(c) {
            let bar = Some((count / 2).saturating_sub(1));
            *table = TableState {
                columns: Vec::new(),
                centred: -1,
                rows: Vec::new(),
                width: ROW_WIDTH,
                scroll: at,
                cross: None,
                graph: Some(Graph { bars: Vec::new(), steps: 0, shapes, bar }),
            };
        }
        true
    }

    /// The Tab. sonstige Länder (`FUN_00457449`): for each country of its
    /// list the game does not play in full, its name, an empty row, its
    /// top three "%d.\t%s\t%d" — the first red — and another empty row;
    /// from the top of the window.
    fn other_tables_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let header = game.text(OTHER_TABLES_HEADER).to_owned();
        set_header(view, dialog, header, &OTHER_TABLES_HEADER_COLUMNS);
        let line = |text: String, ink| TableRow { text, ink, mark: None, columns: None };
        let mut rows = Vec::new();
        for &country in &game.rules.statistics.other_countries {
            if records.globals.played(country as i16) {
                continue;
            }
            rows.push(line(game.world.nation(country).to_owned(), INK));
            rows.push(line(String::new(), INK));
            let table = statistics::other_table(records, country);
            for (i, &(club, points)) in table.iter().take(OTHER_TABLES_SHOWN).enumerate() {
                let name = game.club_name(Some(records), country as u8, club);
                let ink = if i == 0 { OWN_INK } else { INK };
                rows.push(line(format!("{}.\t{name}\t{points}", i + 1), ink));
            }
            rows.push(line(String::new(), INK));
        }
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: OTHER_TABLES_ROW_COLUMNS.to_vec(),
                centred: -1,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Zeitungsmeldungen (`FUN_004571ee`): the newspaper's last
    /// headlines the game keeps, newest first — the date "%1.%2.%3" and
    /// the first line, then the second under it — or, without one, a
    /// single empty row and no columns; from the top of the window.
    fn newspaper_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        set_header(view, dialog, game.text(NEWSPAPER_HEADER).to_owned(), &[(0, 0)]);
        let line = |text: String| TableRow { text, ink: INK, mark: None, columns: None };
        let news = statistics::newspaper(records);
        let (rows, columns) = if news.is_empty() {
            (vec![line(String::new())], Vec::new())
        } else {
            let rows = news
                .iter()
                .flat_map(|n| {
                    let date = game
                        .text(SHORT_DATE)
                        .replace("%1", &n.date.0.to_string())
                        .replace("%2", &n.date.1.to_string())
                        .replace("%3", &n.date.2.to_string());
                    [line(format!("{date}\t{}", n.lines[0])), line(format!("\t{}", n.lines[1]))]
                })
                .collect();
            (rows, NEWSPAPER_COLUMNS.to_vec())
        };
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns,
                centred: -1,
                rows,
                width: ROW_WIDTH,
                scroll: self.scrolled,
                cross: None,
                graph: None,
            };
        }
        true
    }

    /// The Beste Schiris (`FUN_00457711`): the referees of the strip's
    /// country as the game keeps them, the worst first — under "Beste
    /// Schiedsrichter" up to three from the last entry down, under
    /// "Schlechtester Schiedsrichter" after an empty row up to three from
    /// the first up, each "%d.\t%s\t%d\t%1.1f" of the count, name,
    /// matches and the grade over 20; entries without an id left out.
    fn referees_page(&self, cx: &Cx, view: &mut View) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let country = self.strip_country(records);
        set_header(view, dialog, game.text(REFEREES_HEADER).to_owned(), &REFEREES_HEADER_COLUMNS);
        let referees = statistics::referees(records, country);
        let line = |text: String| TableRow { text, ink: INK, mark: None, columns: None };
        let listed = |order: &mut dyn Iterator<Item = usize>| -> Vec<TableRow> {
            order
                .filter_map(|k| referees.get(k).filter(|r| r.id != 0))
                .take(REFEREES_SHOWN)
                .enumerate()
                .map(|(n, r)| {
                    let grade = tenths(f64::from(r.grade) / 20.0);
                    line(format!("{}.\t{}\t{}\t{grade}", n + 1, r.name, r.matches))
                })
                .collect()
        };
        let mut rows = vec![line(game.text(BEST_REFEREES).to_owned())];
        rows.extend(listed(&mut (BEST_DOWN_TO..=BEST_LAST).rev()));
        rows.push(line(String::new()));
        rows.push(line(game.text(WORST_REFEREES).to_owned()));
        rows.extend(listed(&mut (0..WORST_UP_TO)));
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: REFEREES_ROW_COLUMNS.to_vec(),
                centred: -1,
                rows,
                width: ROW_WIDTH,
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
    /// own club red: the Mannschaftsübersicht (`FUN_0044c9c7`), the
    /// strongest first, "%d.\t%s\t%3.1f" of the tenths; the
    /// Durchschnittsalter (`FUN_0044cd71`), the youngest squad first,
    /// "%d.\t%s\t%3.1f\t%d\t%d"; the Ausländer (`FUN_0044d117`), the
    /// most foreigners first, "%d.\t%s\t%d\t%d"; the Eingesetzte Spieler
    /// (`FUN_0044d508`), the most players fielded first,
    /// "%d.\t%s\t%d\t%d\t%d\t%3.1f" with the share a float.
    fn squad_page(&self, cx: &Cx, view: &mut View, code: i16) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(today) = cx.state.season.as_ref().map(|s| s.date()) else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        let (header, header_columns, columns): (u16, &[(u16, u16)], &[(u16, u16)]) = match code {
            statistics::OVERVIEW_PAGE => {
                (OVERVIEW_HEADER, &OVERVIEW_HEADER_COLUMNS, &OVERVIEW_ROW_COLUMNS)
            }
            statistics::AGE_PAGE => (AGE_HEADER, &AGE_COLUMNS, &AGE_COLUMNS),
            statistics::USED_PAGE => (USED_HEADER, &USED_HEADER_COLUMNS, &USED_ROW_COLUMNS),
            _ => (FOREIGN_HEADER, &FOREIGN_HEADER_COLUMNS, &FOREIGN_ROW_COLUMNS),
        };
        set_header(view, dialog, game.text(header).to_owned(), header_columns);
        let own = own_team(cx).map(|t: &Team| (u16::from(t.country()), i16::from(t.number())));
        let figures = statistics::club_figures(records, country, level, today);
        let sorted = match code {
            statistics::OVERVIEW_PAGE => statistics::by_strength(figures),
            statistics::AGE_PAGE => statistics::by_age(figures),
            statistics::USED_PAGE => statistics::by_fielded(figures),
            _ => statistics::by_foreigners(figures),
        };
        let rows = sorted
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let club = game.club_name(Some(records), country as u8, row.team);
                let text = match code {
                    statistics::OVERVIEW_PAGE => format!(
                        "{}.\t{club}\t{}.{}",
                        i + 1,
                        row.strength / 10,
                        row.strength % 10
                    ),
                    statistics::AGE_PAGE => format!(
                        "{}.\t{club}\t{}\t{}\t{}",
                        i + 1,
                        one_place(row.average_age),
                        row.under_22,
                        row.over_30
                    ),
                    statistics::USED_PAGE => {
                        // x87's quotient stored as a float.
                        let share = if row.players > 0 {
                            (f64::from(row.fielded_in_squad) * 100.0 / f64::from(row.players))
                                as f32
                        } else {
                            0.0
                        };
                        format!(
                            "{}.\t{club}\t{}\t{}\t{}\t{}",
                            i + 1,
                            row.fielded,
                            row.players,
                            row.fielded_in_squad,
                            one_place(share)
                        )
                    }
                    _ => format!("{}.\t{club}\t{}\t{}", i + 1, row.foreigners, row.nations),
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

    /// The pages of the club lists the game rebuilds for each league
    /// (`FUN_00724739`), drawn as saved, from the top of the window, the
    /// own club red: the Schirilieblinge (`FUN_0044c57e`), "%d.\t%s\t%d";
    /// the Plätze über Saisonziel (`FUN_0044e23b`),
    /// "%d.\t%s\t%d\t" with a rank of its own that counts the rows
    /// drawn; and "%d.\t%s\t%d\t%d", the Erhaltene Elfmeter
    /// (`FUN_0044df6d`), the Häufigste Einwechsl. (`FUN_0044d9d1`, its
    /// format with a tab more at the end) and Auswechsl. (`FUN_0044dc9f`),
    /// and the Tore nach Einwechsl. (`FUN_0044fa30`), which says when
    /// there is none.
    fn club_list_page(&self, cx: &Cx, view: &mut View, code: i16) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        let (header_columns, columns): (&[(u16, u16)], &[(u16, u16)]) = match code {
            statistics::TARGET_PAGE => (&TARGET_HEADER_COLUMNS, &TARGET_ROW_COLUMNS),
            statistics::REFEREE_PAGE => (&REFEREE_HEADER_COLUMNS, &REFEREE_ROW_COLUMNS),
            statistics::SUB_GOALS_PAGE => (&SUB_GOALS_HEADER_COLUMNS, &SUB_GOALS_ROW_COLUMNS),
            _ => (&PENALTIES_HEADER_COLUMNS, &PENALTIES_ROW_COLUMNS),
        };
        let (header, list, end) = match code {
            statistics::TARGET_PAGE => {
                (TARGET_HEADER, statistics::above_target(records, country, level), "")
            }
            statistics::REFEREE_PAGE => {
                (REFEREE_HEADER, statistics::referees_favourites(records, country, level), "")
            }
            statistics::SUBS_IN_PAGE => {
                (SUBS_IN_HEADER, statistics::substitutions_in(records, country, level), "\t")
            }
            statistics::SUBS_OUT_PAGE => {
                (SUBS_OUT_HEADER, statistics::substitutions_out(records, country, level), "")
            }
            statistics::SUB_GOALS_PAGE => {
                (SUB_GOALS_HEADER, statistics::substitute_goals(records, country, level), "")
            }
            _ => (PENALTIES_HEADER, statistics::penalties_awarded(records, country, level), ""),
        };
        set_header(view, dialog, game.text(header).to_owned(), header_columns);
        let own = own_team(cx).map(|t: &Team| (u16::from(t.country()), i16::from(t.number())));
        let mut rows: Vec<TableRow> = list
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let club = game.club_name(Some(records), row.country as u8, row.team);
                let [first, second] = row.values;
                let text = match code {
                    statistics::TARGET_PAGE => format!("{}.\t{club}\t{first}\t", i + 1),
                    statistics::REFEREE_PAGE => format!("{}.\t{club}\t{first}", i + 1),
                    _ => format!("{}.\t{club}\t{first}\t{second}{end}", i + 1),
                };
                let ink = if own == Some((row.country, row.team)) { OWN_INK } else { INK };
                TableRow { text, ink, mark: None, columns: None }
            })
            .collect();
        let mut columns = columns.to_vec();
        if code == statistics::SUB_GOALS_PAGE && rows.is_empty() {
            let text = game.text(NO_SUB_GOALS).to_owned();
            rows = vec![TableRow { text, ink: INK, mark: None, columns: None }];
            columns.clear();
        }
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns,
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

    /// The Häufigste Einw. or Ausw. Spieler (`FUN_0044e53f`,
    /// `FUN_0044e839`): the league's list of players most often brought on
    /// or taken off, "%d.\t%s\t%s\t%d" with the last name only, the own
    /// club red. The window is handed the matchdays played and one more as
    /// its row count (the league copy's +0x1a), so the rows stand centred
    /// while they are no more than that.
    fn substitutes_page(&self, cx: &Cx, view: &mut View, out: bool) -> bool {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return false };
        let Some(&(country, level)) = self.leagues.get(self.chosen) else { return false };
        let header = if out { PLAYERS_OUT_HEADER } else { PLAYERS_IN_HEADER };
        set_header(view, dialog, game.text(header).to_owned(), &PLAYERS_HEADER_COLUMNS);
        let own = own_team(cx).map(|t: &Team| (u16::from(t.country()), i16::from(t.number())));
        let rows = statistics::player_substitutions(records, country, level, out)
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let club = game.club_name(Some(records), row.country, row.team);
                let text =
                    format!("{}.\t{}\t{club}\t{}", i + 1, records.name(row.last), row.count);
                let ink =
                    if own == Some((u16::from(row.country), row.team)) { OWN_INK } else { INK };
                TableRow { text, ink, mark: None, columns: None }
            })
            .collect();
        let played = League::of(records, country, level).map_or(0, |l| l.played() as i32);
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: PLAYERS_ROW_COLUMNS.to_vec(),
                rows,
                width: ROW_WIDTH,
                centred: played + 1,
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
            // the page keeps it when it draws anew. The Spielplan's moved
            // two matchdays on, which its window draws anew.
            Target::Scroll { .. } => {
                if let Some(c) = control(cx.dialog, TABLE)
                    && let Some(t) = view.table(c)
                {
                    self.scrolled = t.scroll;
                }
                let pages = &cx.game.rules.statistics.categories[self.category];
                let code = self.page.and_then(|e| pages.get(e)).map(|p| p.code);
                if self.category == AKTUELLES && code == Some(statistics::PLAN_PAGE) {
                    self.plan_page(cx, view, Some(self.scrolled));
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
