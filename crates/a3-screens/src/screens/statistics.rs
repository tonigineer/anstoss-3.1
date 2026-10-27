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
//! champion (`FUN_00445ded`); and the rounds of the national cups
//! (`FUN_004469c0`). The others are crossed out.

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
use crate::controls::table::{TableRow, TableState};
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
/// The tables' category and their codes; the forecast's.
const TABLES: usize = 0;
const TABLE_CODES: std::ops::RangeInclusive<i16> = 0..=7;
const FORECAST: i16 = 8;
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
            Some(code) if self.category == TABLES && statistics::CUP_PAGES.contains(&code) => {
                self.cup_page(cx, view, false)
            }
            Some(code)
                if self.category == TABLES && statistics::LEAGUE_CUP_PAGES.contains(&code) =>
            {
                self.cup_page(cx, view, true)
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
                TableRow { text, ink, mark }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: columns.to_vec(),
                rows,
                width: ROW_WIDTH,
                centred: -1,
                scroll: self.scrolled,
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
                TableRow { text, ink, mark: None }
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
                };
        }
        true
    }
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
                TableRow { text, ink, mark: None }
            })
            .collect();
        if let Some(table) = control(dialog, TABLE).and_then(|c| view.table_mut(c)) {
            *table = TableState {
                columns: FORECAST_COLUMNS.to_vec(),
                rows,
                width: FORECAST_WIDTH,
                centred: -1,
                scroll: 0,
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
