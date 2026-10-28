//! The statistics archive (dialog 18, `CArchivTabControl`; STATISTIKEN.md
//! in the RE repo): its pages by category, which of them a club sees, and
//! the league tables its first pages show.
//!
//! The page table (`0x87a818`) holds eight categories of entries: the
//! statistics department's level a page needs, its condition, its name,
//! the strip it steps with and the code its category draws it by. Which
//! pages are listed is `FUN_00458d3d`'s: the level, then the condition —
//! the saved table at `0xb5e026` the day's run sets, else a few rules of
//! their own —, the countries played, an own club.

use std::io;

use a3_files::exe::Exe;

use chrono::NaiveDate;

use crate::opponent::{CUPS_AT, League, PAIRING, PAIRINGS};
use crate::random::Random;
use crate::records::{Manager, Player, Records, Team};

pub const CATEGORIES: usize = 8;
/// The categories' names: Tabellen, Mannschaft, Liga, Aktuelles,
/// Historisches, Stadion, Finanzen, Spiel (`0x87ca10`).
pub const CATEGORY_NAMES: [u16; CATEGORIES] = [2000, 2001, 2002, 2003, 2004, 2005, 2006, 4718];

const PAGES_AT: u32 = 0x87a818;
const CATEGORY_BYTES: usize = 0x3c0;
const ENTRY: usize = 12;

/// A page of the archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    /// The statistics department's level it needs.
    pub level: i16,
    /// Its condition (& 0x7ff), a country slot (& 0x7000) and whether it
    /// needs an own club (0x8000).
    pub flags: u16,
    /// Its name's text; with 0x10000 a European cup's name, the Champions
    /// League's for 1.
    pub name: u32,
    /// The strip it steps through ([`Strip`]).
    pub selector: i16,
    /// What its category draws.
    pub code: i16,
}

pub const CONDITION: u16 = 0x7ff;
pub const SLOT: u16 = 0x7000;
pub const OWN_CLUB: u16 = 0x8000;
/// A name that is a European cup's.
pub const CUP_NAME: u32 = 0x10000;
/// The codes that name a past season: the season so many back + 100.
pub const PAST_SEASONS: std::ops::RangeInclusive<u16> = 0x65..=0x6e;

/// The archive's pages, category by category.
pub struct Pages {
    pub categories: Vec<Vec<Page>>,
}

impl Pages {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let bytes = exe.bytes(PAGES_AT, CATEGORIES * CATEGORY_BYTES)?;
        let word = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        let categories = (0..CATEGORIES)
            .map(|c| {
                (0..CATEGORY_BYTES / ENTRY)
                    .map(|i| c * CATEGORY_BYTES + i * ENTRY)
                    .map(|at| Page {
                        level: word(at) as i16,
                        flags: word(at + 2),
                        name: u32::from(word(at + 4)) | u32::from(word(at + 6)) << 16,
                        selector: word(at + 8) as i16,
                        code: word(at + 10) as i16,
                    })
                    .take_while(|p| p.name != 0)
                    .collect()
            })
            .collect();
        Ok(Self { categories })
    }
}

/// What the conditions ask of the club and its manager.
pub struct Club<'a> {
    pub records: &'a Records,
    /// Its team and its manager; none for a national team.
    pub team: Option<&'a Team>,
    pub manager: Option<&'a Manager>,
    /// Its squad's ids.
    pub squad: &'a [u16],
}

/// The statistics department's level, 0 to 3 (club +0x5e1); 2 without a
/// club.
pub fn level(team: Option<&Team>) -> i16 {
    const DEPARTMENT: usize = 0x5e1 + crate::finances::CLUB_TO_TEAM;
    team.map_or(2, |t| i16::from(t.get::<i8>(DEPARTMENT)))
}

/// The day's table of conditions (`DAT_00b5e026`, `FUN_0045ae9c`).
const CONDITIONS_AT: usize = 0xb5e026;
const CONDITIONS: u16 = 121;
/// Leagues from this level on keep no record of their own.
const RECORDED_LEAGUES: u8 = 6;
const LAST_MATCHDAYS: usize = 5;
const SMALL_LEAGUE: usize = 18;
/// Club +0x5a9: the club's tickets left to the agency.
const AGENCY: usize = 0x5a9 + crate::finances::CLUB_TO_TEAM;
const FIRST_WEEK: u16 = 6;
const STATUS_BIT: u8 = 2;
/// The manager object's +0x94, which `FUN_006a426b` copies from the
/// record's +0x180.
const MANAGER_SEASONS: usize = 0x180;
const ENGLAND: i16 = 16;
/// The stored statistics of each human's last match.
const MATCH_STATISTICS: &str = "0x1c26dd8";

/// Whether a page's condition holds (`FUN_00458d3d`): by the day's table,
/// else by its own rule; codes without one never hold.
pub fn condition(club: &Club, code: u16) -> bool {
    let records = club.records;
    let globals = &records.globals;
    if code < CONDITIONS && globals.get::<i16>(CONDITIONS_AT + 2 * usize::from(code)) != 0 {
        return true;
    }
    let league = club.team.and_then(|t| {
        (t.level() < RECORDED_LEAGUES).then(|| League::of(records, t.country().into(), t.level()))?
    });
    let squad = |f: &dyn Fn(&crate::records::Player) -> bool| {
        club.squad.iter().filter_map(|&id| records.player(id)).any(f)
    };
    let games_at = |home: bool| {
        let (Some(team), Some(league)) = (club.team, league.as_ref()) else { return false };
        let place = usize::from(team.league_place());
        (0..league.played()).flat_map(|md| league.games(md)).any(|(h, a, result)| {
            result.is_some() && if home { h == place } else { a == place }
        })
    };
    match code {
        6 => club.manager.is_some_and(|m| m.status() & STATUS_BIT != 0),
        7 => league.as_ref().is_some_and(|l| {
            l.played() + LAST_MATCHDAYS >= l.matchdays() && l.played() < l.matchdays()
        }),
        8 => squad(&|p| p.own_goals() != 0),
        9 => squad(&|p| p.joker_goals() != 0),
        0xa => squad(&|p| p.goals()[0] != 0 || p.assists() != 0),
        0xb => squad(&|p| p.assists() != 0),
        0xf => {
            let teams = league.as_ref().map_or(SMALL_LEAGUE, League::teams);
            globals.seasons() >= 2 || league.as_ref().map_or(0, League::played) >= teams
        }
        0x10 => {
            club.team.is_some_and(|t| t.get::<u8>(AGENCY) == 0) && globals.day() > FIRST_WEEK
        }
        0x11 => club.manager.is_some_and(|m| m.get::<i16>(MANAGER_SEASONS) > 1),
        0x12 | 0x14 => games_at(true),
        0x13 => games_at(false),
        0x26 => club.team.is_some(),
        // The last match's statistics hold players (`FUN_00721968`: the
        // human's block at `0x1c26dd8`, its count first).
        0x27 => club.team.is_some_and(|t| {
            let block = usize::from(t.human()).checked_sub(1).and_then(|h| {
                records.block(&format!("{MATCH_STATISTICS}[{h}]"))
            });
            let count = block.and_then(|b| b.get(..4)).map(|b| [b[0], b[1], b[2], b[3]]);
            count.is_some_and(|b| i32::from_le_bytes(b) > 0)
        }),
        0x28 => globals.main_country() == ENGLAND,
        0x29..=0x2c => globals.more_countries()[usize::from(code - 0x29)] == ENGLAND,
        c if PAST_SEASONS.contains(&c) => c - 100 < globals.seasons(),
        _ => false,
    }
}

/// Whether a page is listed for the club at its department's `level`.
pub fn listed(club: &Club, level: i16, page: &Page) -> bool {
    let slot = (page.flags & SLOT) >> 12;
    page.level <= level
        && condition(club, page.flags & CONDITION)
        && i32::from(slot) <= i32::from(club.records.globals.more_count()) + 1
        && (page.flags & OWN_CLUB == 0 || club.team.is_some())
}

/// The kinds of league table (`FUN_007293ea`): all games, at home, away,
/// the first and the second half of the season, the first and the second
/// half of the games, and with two points for a win.
pub const ALL: u8 = 0;
pub const HOME: u8 = 1;
pub const AWAY: u8 = 2;
pub const FIRST_ROUND: u8 = 3;
pub const SECOND_ROUND: u8 = 4;
pub const FIRST_HALF: u8 = 5;
pub const SECOND_HALF: u8 = 6;
pub const TWO_POINTS: u8 = 7;

/// A row of a league table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Row {
    /// The team's place in the league, and its number.
    pub place: usize,
    pub team: i16,
    pub games: i32,
    pub points: i32,
    pub goals_for: i32,
    pub goals_against: i32,
    pub won: i32,
    pub drawn: i32,
    pub lost: i32,
    /// Its flags (team +0x12a): its penalty points (& 3), 0x80 sent to
    /// the bottom.
    flags: u16,
}

const PENALTY: u16 = 3;
const PENALTY_POINTS: i32 = 3;
const LAST: u16 = 0x80;
/// The rule that gives a point more for three goals.
const THREE_GOALS: u32 = 0x80;
const BONUS_GOALS: i32 = 3;
const TEAM_FLAGS: usize = 0x12a;
/// The tie-break by goals scored first.
const GOALS_FIRST: i16 = 1;

/// A league's table of `kind` as the archive reckons it from the results
/// (`FUN_007293ea` with all penalties, sorted by `FUN_0072b9b9`).
pub fn table(records: &Records, league: &League, kind: u8) -> Vec<Row> {
    reckon(records, league, kind, true)
}

/// A league's games in the season results' cross-table (`FUN_00445951`'s
/// cells into `CTabellenWnd`): row and column by the league place the
/// schedule plays by, from zero — the season's starting order, not
/// today's standings; the cell of a row and a column holds the game the
/// row's club had at home against the column's, its two scores, and none
/// while it was not played.
pub fn cross_table(league: &League) -> Vec<Vec<Option<(u8, u8)>>> {
    let mut cells = vec![vec![None; league.teams()]; league.teams()];
    for md in 0..league.played() {
        for (hp, ap, result) in league.games(md) {
            let Some([home, away, _, _]) = result else { continue };
            if let Some(row) = cells.get_mut(hp)
                && let Some(cell) = row.get_mut(ap)
            {
                *cell = Some((home, away));
            }
        }
    }
    cells
}

/// The goal pages' codes: Trefferübersicht, kassierte Tore, Tordifferenz
/// and Tore total — all four `FUN_0044917a` draws as one graph under the
/// header text 0xa50, only its array differing.
pub const GOAL_PAGES: [i16; 4] = [0xa, 0x1e, 0x1f, 0x20];

/// The four goal pages' sums (`FUN_00448c76`): over the league's played
/// games, each club's goals scored, conceded, the difference and both
/// together — the four short arrays its `local_a8`, `local_1234`,
/// `local_78` and `local_44` fill, by the league's own places.
pub fn goal_sums(league: &League) -> Vec<[i16; 4]> {
    let mut sums = vec![[0i16; 4]; league.teams()];
    for md in 0..league.played() {
        for (hp, ap, result) in league.games(md) {
            let Some([home, away, _, _]) = result else { continue };
            let (home, away) = (i16::from(home), i16::from(away));
            for (p, own, other) in [(hp, home, away), (ap, away, home)] {
                if let Some(s) = sums.get_mut(p) {
                    s[0] = s[0].wrapping_add(own);
                    s[1] = s[1].wrapping_add(other);
                    s[2] = s[2].wrapping_add(own.wrapping_sub(other));
                    s[3] = s[3].wrapping_add(own.wrapping_add(other));
                }
            }
        }
    }
    sums
}

/// `FUN_0044917a`'s graph order out of `goal_sums`' array `mode`: the
/// clubs by descending value as its exchange sort leaves them — every
/// place from the first up takes the first of the rest that beats it,
/// swapping with it, so a tie keeps its place but a beaten club gets
/// shuffled down — and the axis' end, the next full score of 25 over
/// the greatest value, 25 where there is none. Its plot of 376 pixels
/// stands against that end.
pub fn goal_graph(sums: &[[i16; 4]], mode: usize) -> (Vec<(usize, i16)>, i32) {
    let mut order: Vec<usize> = (0..sums.len()).collect();
    for i in 0..sums.len().saturating_sub(1) {
        for j in i + 1..sums.len() {
            if sums[order[i]][mode] < sums[order[j]][mode] {
                order.swap(i, j);
            }
        }
    }
    let rows: Vec<(usize, i16)> =
        order.iter().map(|&p| (p, sums[p][mode])).collect();
    let top = i32::from(rows.first().map_or(0, |&(_, value)| value));
    let axis = ((top + 24) / 25).max(1) * 25;
    (rows, axis)
}

/// The Mannschaft category's pages over the own squad (`FUN_004677f5`):
/// the Torjägerliste under text 2413 (`FUN_00469f05(0)`).
pub const SQUAD_SCORERS_PAGE: i16 = 0;

/// The own squad in a page's order (`FUN_0046767e`'s exchange sort over
/// the indexes `FUN_00467645` lays out): the players as the club lists
/// them, every place from the first up swapping with the first of the
/// rest whose key is greater.
pub fn squad_by(records: &Records, team: &Team, key: impl Fn(&Player) -> i32) -> Vec<u16> {
    let ids = team.players();
    let keys: Vec<i32> = ids.iter().map(|&id| records.player(id).map_or(0, &key)).collect();
    let mut order: Vec<usize> = (0..ids.len()).collect();
    for i in 0..order.len() {
        for j in i + 1..order.len() {
            if keys[order[i]] < keys[order[j]] {
                order.swap(i, j);
            }
        }
    }
    order.iter().map(|&k| ids[k]).collect()
}

/// The Liga category's first two pages: the Torjägerliste and the
/// Scorer, under the single header texts 0xa99 and 0xa9b; behind them
/// the Vorlagen, the Freistoßschützen and the Elfmeterschützen, under
/// 0xa9a, 0xa9d and 0xa9e (`FUN_0044a02e`, `FUN_0044ab7a`,
/// `FUN_0044af3b`).
pub const TOR_PAGE: i16 = 0;
pub const SCORER_PAGE: i16 = 1;
pub const VORLAGEN_PAGE: i16 = 2;
pub const FREISTOSS_PAGE: i16 = 3;
pub const ELFMETER_PAGE: i16 = 4;
/// The Elfmeterkiller and the Eigentore, under 0xa9f and 0xa9c
/// (`FUN_0044b2fc`, `FUN_0044a810`); the Fairneß and the
/// Chancenverwertung between them list clubs instead, under 0xaa1 and
/// 0xaa2 (`FUN_0044bb6d`, `FUN_0044c08e`).
pub const KILLER_PAGE: i16 = 5;
/// The Treter under 0xaa0 (`FUN_0044b6e0`), shown from the second
/// department on.
pub const TRETER_PAGE: i16 = 9;
/// The Schirilieblinge under 0xb6f (`FUN_0044c57e`), from the second
/// department on.
pub const REFEREE_PAGE: i16 = 0xa;
pub const FAIRNESS_PAGE: i16 = 6;
pub const CHANCEN_PAGE: i16 = 7;
pub const EIGENTORE_PAGE: i16 = 8;

/// One player of the Liga lists: the name pool's ids of the name as the
/// list holds it, his club's number, with the figures the page shows in
/// its columns' order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LigaRow {
    /// His first and last name's ids in the pool.
    pub first: u16,
    pub last: u16,
    /// The club's number in its country.
    pub team: i16,
    pub values: [u16; 6],
}

/// The block holding this season's player lists (`DAT_00b0b6b8`): the
/// game keeps one list per league as the cycle plays and the archive
/// draws just that — `FUN_00721dce` copies a league's list out, and the
/// sorting itself, done by a threaded engine, belongs to the live game
/// updating the lists match by match, never to the page. A top scorer's
/// entry of 0x10 at `DAT_00b0b6f4` holds his id, his names, a flag, his
/// club's number, his goals, a counter the page never shows, and the
/// columns' free kicks, penalties, headers, goals from distance and cup
/// goals; a scorer's entry of 0xc at `DAT_00b0fd46` holds his names, his
/// club's number and the columns' point, goals, matchdays he scored and
/// assisted on and his assists. A row is drawn once its first column
/// stands above 0.
const LISTS: &str = "0xb0b6b8";
const TOR_LIST: usize = 0x3c;
const TOR_LEAGUE: usize = 0x640;
const SCORER_LIST: usize = 0x468e;
const SCORER_LEAGUE: usize = 0xf0;

/// Which league's list a page reads (`FUN_00677b07`): the main country's
/// by its level, and the more countries' first leagues, behind the main
/// country's six.
fn league_of(records: &Records, country: u16, level: u8) -> Option<usize> {
    let country = i16::try_from(country).ok()?;
    let globals = &records.globals;
    if globals.main_country() == country {
        return (level < 6).then_some(usize::from(level));
    }
    let more = usize::try_from(globals.more_count()).unwrap_or(0);
    (0..more)
        .find(|&k| globals.more_countries()[k] == country)
        .map(|k| k + 6)
}

/// A league's list in the block: its base, row and entry widths.
fn list_bytes(
    records: &Records,
    country: u16,
    level: u8,
    at: usize,
    row: usize,
    entry: usize,
    count: usize,
) -> Option<&[u8]> {
    let league = league_of(records, country, level)?;
    let from = at + league * row;
    records.block(LISTS)?.get(from..from + count * entry)
}

/// The league's top scorers this season (`FUN_00449bd6`): the saved
/// list's hundred entries; his goals, free kicks, penalties, headers,
/// goals from distance and cup goals stand in the columns.
pub fn torjaeger(records: &Records, country: u16, level: u8) -> Vec<LigaRow> {
    let Some(b) = list_bytes(records, country, level, TOR_LIST, TOR_LEAGUE, 0x10, 100) else {
        return Vec::new();
    };
    (0..100)
        .filter_map(|i| {
            let e = &b[i * 0x10..i * 0x10 + 0x10];
            (e[8] != 0).then(|| LigaRow {
                first: u16::from_le_bytes([e[2], e[3]]),
                last: u16::from_le_bytes([e[4], e[5]]),
                team: i16::from(e[7]),
                values: [e[8], e[10], e[11], e[12], e[13], e[14]].map(u16::from),
            })
        })
        .collect()
}

/// The three saved lists that share one shape (`DAT_00b0f576`,
/// `DAT_00b10ce6` and `DAT_00b114b6`, inside the same block): twenty
/// entries of 10 with the names' pool ids, the club's number, and from
/// the gate byte on the figures of their columns.
const SHORT_LISTS: [(usize, usize); 3] = [(0x3ebe, 200), (0x562e, 200), (0x5dfe, 200)];

/// One of those twenty-entry lists.
fn short_list(records: &Records, country: u16, level: u8, which: usize) -> Vec<LigaRow> {
    let (at, row) = SHORT_LISTS[which];
    let Some(b) = list_bytes(records, country, level, at, row, 10, 0x14) else {
        return Vec::new();
    };
    (0..0x14)
        .filter_map(|i| {
            let e = &b[i * 10..i * 10 + 10];
            (e[5] != 0).then(|| LigaRow {
                first: u16::from_le_bytes([e[0], e[1]]),
                last: u16::from_le_bytes([e[2], e[3]]),
                team: i16::from(e[4]),
                values: [e[5], e[6], e[7], e[8], e[9], 0].map(u16::from),
            })
        })
        .collect()
}

/// The league's assisters this season (`FUN_0044a02e`): his assists and
/// the matchdays on which they came.
pub fn vorlagen(records: &Records, country: u16, level: u8) -> Vec<LigaRow> {
    short_list(records, country, level, 0)
}

/// The league's free-kick takers (`FUN_0044ab7a`): the ones they took,
/// the converted and the missed.
pub fn freistosse(records: &Records, country: u16, level: u8) -> Vec<LigaRow> {
    short_list(records, country, level, 1)
}

/// The league's penalty takers (`FUN_0044af3b`): the ones they got, the
/// converted and the missed.
pub fn elfmeter(records: &Records, country: u16, level: u8) -> Vec<LigaRow> {
    short_list(records, country, level, 2)
}

/// The Elfmeterkiller's list (`DAT_00b11c86`): twenty entries of 10 as
/// the short lists', drawn once one of the two figures stands — the
/// penalties he saved and those he let in.
pub fn elfmeterkiller(records: &Records, country: u16, level: u8) -> Vec<LigaRow> {
    let Some(b) = list_bytes(records, country, level, 0x65ce, 200, 10, 0x14) else {
        return Vec::new();
    };
    (0..0x14)
        .filter_map(|i| {
            let e = &b[i * 10..i * 10 + 10];
            (e[5] != 0 || e[6] != 0).then(|| LigaRow {
                first: u16::from_le_bytes([e[0], e[1]]),
                last: u16::from_le_bytes([e[2], e[3]]),
                team: i16::from(e[4]),
                values: [e[5], e[6], 0, 0, 0, 0].map(u16::from),
            })
        })
        .collect()
}

/// The eigen goal scorers (`FUN_0044a810` from `DAT_00b106aa`): twenty
/// entries of 8 — the club's number, his own goals, and his names' pool
/// ids behind both.
pub fn eigentore(records: &Records, country: u16, level: u8) -> Vec<LigaRow> {
    let Some(b) = list_bytes(records, country, level, 0x4ff2, 0xa0, 8, 0x14) else {
        return Vec::new();
    };
    (0..0x14)
        .filter_map(|i| {
            let e = &b[i * 8..i * 8 + 8];
            (e[1] != 0).then(|| LigaRow {
                first: u16::from_le_bytes([e[4], e[5]]),
                last: u16::from_le_bytes([e[6], e[7]]),
                team: i16::from(e[0]),
                values: [e[1], 0, 0, 0, 0, 0].map(u16::from),
            })
        })
        .collect()
}

/// A club of the Fairneß table: its number, its points and the red,
/// yellow-red and yellow cards they count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FairRow {
    pub team: i16,
    pub points: i16,
    pub cards: [u8; 3],
}

/// The Fairneß page's data: the clubs, the matchdays played to average
/// over, and the season's record for one match, a club and its points.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fairness {
    pub rows: Vec<FairRow>,
    pub played: i16,
    pub record: Option<(i16, i16)>,
}

/// The league's fair-play table in the same block (`DAT_00b12454`): a
/// hundred bytes a league, a club's number and its red, yellow-red and
/// yellow cards of the season in four bytes each, then the record's
/// club and points (+0x60, +0x62).
const FAIR_LIST: usize = 0x6d9c;
const FAIR_LEAGUE: usize = 100;
const FAIR_RECORD: usize = 0x60;

/// The Fairneß Mannschaft (`FUN_0044bb6d`): the first as many of the
/// list's clubs as the league has, in the order the game keeps them —
/// `FUN_00743b29` moves a club up past those with fewer points after
/// each match — each worth five points a red card, three a yellow-red
/// and one a yellow; the matchdays played come from the league
/// (`FUN_00692045`'s copy, its +0x1a); the record stands once its
/// points do.
pub fn fairness(records: &Records, country: u16, level: u8) -> Fairness {
    let Some(league) = League::of(records, country, level) else { return Fairness::default() };
    let Some(b) = list_bytes(records, country, level, FAIR_LIST, FAIR_LEAGUE, 1, FAIR_LEAGUE)
    else {
        return Fairness::default();
    };
    let rows = (0..league.teams().min(FAIR_RECORD / 4))
        .map(|i| {
            let e = &b[i * 4..i * 4 + 4];
            let points = u16::from(e[1]) * 5 + u16::from(e[2]) * 3 + u16::from(e[3]);
            FairRow { team: i16::from(e[0]), points: points as i16, cards: [e[1], e[2], e[3]] }
        })
        .collect();
    let at = |k: usize| i16::from_le_bytes([b[k], b[k + 1]]);
    let points = at(FAIR_RECORD + 2);
    let record = (points != 0).then(|| (at(FAIR_RECORD), points));
    Fairness { rows, played: league.played() as i16, record }
}

/// The Liga's club pages from Mannschaftsübersicht on, by their codes:
/// Mannschaftsübersicht under 0xaa3 (`FUN_0044c9c7`), Durchschnittsalter
/// under 0xdd9 (`FUN_0044cd71`), Ausländer under 0xe75 (`FUN_0044d117`).
pub const OVERVIEW_PAGE: i16 = 0xb;
pub const AGE_PAGE: i16 = 0xc;
pub const FOREIGN_PAGE: i16 = 0xe;
/// Eingesetzte Spieler under 0xe7b (`FUN_0044d508`), from the second
/// department on.
pub const USED_PAGE: i16 = 0xf;
/// Plätze über Saisonziel under 0x1288 (`FUN_0044e23b`); Erhaltene
/// Elfmeter under 0x1287 (`FUN_0044df6d`); Häufigste
/// Einwechsl. and Auswechsl. under 0x1285 and 0x1286 (`FUN_0044d9d1`,
/// `FUN_0044dc9f`).
pub const TARGET_PAGE: i16 = 0x10;
/// Häufigste Einw. and Ausw. Spieler under 0x1289 and 0x1292
/// (`FUN_0044e53f`, `FUN_0044e839`).
pub const PLAYERS_IN_PAGE: i16 = 0x11;
pub const PLAYERS_OUT_PAGE: i16 = 0x12;
pub const PENALTIES_PAGE: i16 = 0x13;
pub const SUBS_IN_PAGE: i16 = 0x1e;
pub const SUBS_OUT_PAGE: i16 = 0x1f;
/// Tore nach Einwechsl. under 0x1291 (`FUN_0044fa30`).
pub const SUB_GOALS_PAGE: i16 = 0x1a;

/// A club's figures as `FUN_00642cd4` gathers them for the club pages,
/// an entry of 0x1c bytes each: its number, its strength in tenths (+2),
/// its squad, their mean age (a float at +8), how many are under 22 and
/// over 30 (+0xe, +0x10), the foreigners among them (+0x14), the other
/// nations they come from (+0x16), the players it fielded this season
/// (+0x18) and how many of them are still in its squad (+0x1a).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClubFigures {
    pub team: i16,
    pub strength: i16,
    pub players: i16,
    pub average_age: f32,
    pub under_22: i16,
    pub over_30: i16,
    pub foreigners: i16,
    pub nations: i16,
    pub fielded: i16,
    pub fielded_in_squad: i16,
}

/// The nations `FUN_00676e4c` takes as one: England, Wales, Scotland and
/// Northern Ireland.
const BRITISH: [u8; 4] = [0x10, 0x2d, 0x39, 0x49];
/// The nations the count of nations runs over (its array of 0x75).
const NATIONS: usize = 0x75;
/// The players each club fielded this season (`DAT_00a7598a`, read by
/// `FUN_006403d2`): fifty ids a club by its index among all, a 0 ending
/// them early. The save keeps the object from its +0x20 on, a stamp
/// (`DAT_00a75988`) and then the lists.
const FIELDED: &str = "0xa75968";
const FIELDED_AT: usize = 2;
const FIELDED_MOST: usize = 50;

/// The league's clubs in the order of its table (`FUN_00642cd4`): for
/// each its squad as the club lists it, every player's age on `today`
/// (`FUN_0067582a`), whether he is a foreigner — of another nation than
/// `country`, a Briton only outside Britain — and how many nations
/// besides the country's own the squad brings.
pub fn club_figures(records: &Records, country: u16, level: u8, today: NaiveDate) -> Vec<ClubFigures> {
    let Some(league) = League::of(records, country, level) else { return Vec::new() };
    (0..league.teams())
        .map(|place| {
            let team = league.team(place);
            let mut figures = ClubFigures { team, ..ClubFigures::default() };
            let Some(record) = u16::try_from(team).ok().and_then(|t| records.team(country, t)) else {
                return figures;
            };
            figures.strength = strength(records, record);
            let ids = record.players();
            let mut sum = 0i16;
            let mut seen = [false; NATIONS];
            for player in ids.iter().filter_map(|&id| records.player(id)) {
                let age = crate::autolineup::age(player.birthday(), today) as i16;
                sum = sum.wrapping_add(age);
                figures.under_22 += i16::from(age < 22);
                figures.over_30 += i16::from(age > 30);
                let nation = player.nation();
                let foreign = if BRITISH.iter().any(|&b| u16::from(b) == country) {
                    !BRITISH.contains(&nation)
                } else {
                    u16::from(nation) != country
                };
                figures.foreigners += i16::from(foreign);
                if let Some(seen) = seen.get_mut(usize::from(nation)) {
                    *seen = true;
                }
            }
            figures.players = ids.len() as i16;
            if figures.players >= 1 {
                figures.average_age = f32::from(sum) / f32::from(figures.players);
            }
            figures.nations = (0..NATIONS)
                .filter(|&n| seen[n] && n != usize::from(country))
                .count() as i16;
            // `FUN_006404ff`: the fielded players up to the first 0, and
            // how many of them the squad holds.
            let fielded = fielded(records, country, team);
            figures.fielded = fielded.len() as i16;
            figures.fielded_in_squad = fielded.iter().filter(|id| ids.contains(id)).count() as i16;
            figures
        })
        .collect()
}

/// How many of each kind of position a team's strength counts
/// (`FUN_007779e4`): its best two keepers, six defenders, five
/// midfielders and three strikers, by their main position — 1 the keeper,
/// 2 to 5 the defence, 6 to 9 the midfield, 10 the attack.
const STRENGTH_COUNTS: [usize; 4] = [2, 6, 5, 3];

/// A team's strength in tenths (`FUN_007779e4`): every player put into
/// his kind's list by his value (`FUN_00777544`) — ten times his strength,
/// five more for the traits of +0x1ca bit 1 or +0x1c8 bits 0x101, one more
/// for each of the eleven abilities he is good at — and the mean of each
/// list's best, ten times over, cut to a whole number; 1 when no list has
/// one. A list's slots start empty at 0, so a value of 0 never counts.
pub fn strength(records: &Records, team: &Team) -> i16 {
    let mut kinds: [Vec<i16>; 4] = Default::default();
    for player in team.players().iter().filter_map(|&id| records.player(id)) {
        let kind = match player.positions()[0] {
            1 => 0,
            2..=5 => 1,
            6..=9 => 2,
            10 => 3,
            _ => continue,
        };
        let [first, second] = player.traits();
        let mut value = u16::from(player.strength()) * 10;
        if second & 1 != 0 || first & 0x101 != 0 {
            value += 5;
        }
        value += (player.good() & 0x7ff).count_ones() as u16;
        kinds[kind].push(value as i16);
    }
    let (mut sum, mut count) = (0i16, 0i32);
    for (values, &best) in kinds.iter_mut().zip(&STRENGTH_COUNTS) {
        values.sort_unstable_by(|a, b| b.cmp(a));
        for &v in values.iter().filter(|&&v| v > 0).take(best) {
            sum = sum.wrapping_add(v);
            count += 1;
        }
    }
    if count < 1 { 1 } else { (i32::from(sum) * 10 / count) as i16 }
}

/// The Mannschaftsübersicht's order (`FUN_0044c9c7`): the strongest team
/// first, by the page's exchange sort.
pub fn by_strength(mut rows: Vec<ClubFigures>) -> Vec<ClubFigures> {
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            if rows[i].strength < rows[j].strength {
                rows.swap(i, j);
            }
        }
    }
    rows
}

/// The ids of the players a club fielded this season (`FUN_006403d2`),
/// none for a number past the country's clubs.
fn fielded(records: &Records, country: u16, team: i16) -> Vec<u16> {
    let c = usize::from(country);
    if team < 0 || team >= records.countries.team_count(c) {
        return Vec::new();
    }
    let index = i32::from(records.countries.first_team(c)) + i32::from(team);
    let Some(b) = usize::try_from(index).ok().and_then(|i| {
        let at = FIELDED_AT + i * 2 * FIELDED_MOST;
        records.block(FIELDED)?.get(at..at + 2 * FIELDED_MOST)
    }) else {
        return Vec::new();
    };
    b.chunks_exact(2).map(|w| u16::from_le_bytes([w[0], w[1]])).take_while(|&id| id != 0).collect()
}

/// The Eingesetzte Spieler's order (`FUN_0044d508`): the most players
/// fielded first, of equal ones the fewer still in the squad, by the
/// same exchange sort.
pub fn by_fielded(mut rows: Vec<ClubFigures>) -> Vec<ClubFigures> {
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            let (a, b) = (&rows[i], &rows[j]);
            if a.fielded < b.fielded
                || a.fielded == b.fielded && b.fielded_in_squad < a.fielded_in_squad
            {
                rows.swap(i, j);
            }
        }
    }
    rows
}

/// The Durchschnittsalter's order (`FUN_0044cd71`): the youngest squad
/// first, by the page's exchange sort — every place from the first up
/// swapping with the first of the rest that is younger.
pub fn by_age(mut rows: Vec<ClubFigures>) -> Vec<ClubFigures> {
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            if rows[j].average_age < rows[i].average_age {
                rows.swap(i, j);
            }
        }
    }
    rows
}

/// The Ausländer's order (`FUN_0044d117`): the most foreigners first,
/// on a tie the more nations, by the same exchange sort.
pub fn by_foreigners(mut rows: Vec<ClubFigures>) -> Vec<ClubFigures> {
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            let (a, b) = (&rows[i], &rows[j]);
            if a.foreigners < b.foreigners || a.foreigners == b.foreigners && a.nations < b.nations {
                rows.swap(i, j);
            }
        }
    }
    rows
}

/// A club of the club lists the game rebuilds for each league as the
/// season goes on (`FUN_00724739`): its country and number, and its
/// figures.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClubCount {
    pub country: u16,
    pub team: i16,
    pub values: [i16; 2],
}

/// The penalties a club was awarded and scored (`DAT_00aeab60`, a list
/// object of 0xf4 a league whose 24 entries of 8 the save keeps): the
/// club's country and number, then its club's +0xdea and +0xdeb, the
/// most awarded first.
const PENALTY_CLUBS: &str = "0xaeab40";
/// The substitutions a club made (`DAT_00ae9850` and `DAT_00aea1d8`, of
/// the same shape): in — its team +0x339, and the goals its substitutes
/// scored, +0x33b — and out, +0x33a, its second figure always 0
/// (`FUN_00722dda` writes it so); the most first.
const SUBS_IN_CLUBS: &str = "0xae9830";
/// How many places a club stands above its season's target (`DAT_00aec230`,
/// 0xc4 a league, 24 entries of 6): its country and number, and the
/// places `FUN_00775fa0` reckons, below the target negative, the most
/// first; `FUN_00722fa2` clears the list to −100 before each rebuild.
const TARGET_CLUBS: &str = "0xaec210";
/// The goals a club's substitutes scored (`DAT_00af1140`, of the
/// penalties' shape): its team +0x33b, the most first, and +0x33c, those
/// of the last matchday.
const SUB_GOALS_CLUBS: &str = "0xaf1120";
const NO_TARGET: i16 = -100;
const SUBS_OUT_CLUBS: &str = "0xaea1b8";

/// A league's rebuilt club list as the page copies it out (`FUN_00721008`
/// and its siblings, by the league's list index `FUN_00677b07`): its
/// entries of `entry` bytes, as long as one names a country, 24 at most.
fn club_list(records: &Records, country: u16, level: u8, list: &str, entry: usize) -> Vec<ClubCount> {
    let Some(index) = league_of(records, country, level) else { return Vec::new() };
    let Some(b) = records.block(&format!("{list}[{index}]")) else { return Vec::new() };
    let word = |at: usize| b.get(at..at + 2).map_or(0, |w| i16::from_le_bytes([w[0], w[1]]));
    (0..24)
        .map(|i| i * entry)
        .take_while(|&at| word(at) != 0)
        .map(|at| ClubCount {
            country: word(at) as u16,
            team: word(at + 2),
            values: [word(at + 4), if entry >= 8 { word(at + 6) } else { 0 }],
        })
        .collect()
}

/// The Erhaltene Elfmeter (`FUN_0044df6d`): each club's penalties awarded
/// and scored.
pub fn penalties_awarded(records: &Records, country: u16, level: u8) -> Vec<ClubCount> {
    club_list(records, country, level, PENALTY_CLUBS, 8)
}

/// A player of the rebuilt player lists: his last name's id in the pool,
/// his club's country and number, and his count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerCount {
    pub last: u16,
    pub country: u8,
    pub team: i16,
    pub count: i16,
}

/// The players most often brought on and taken off (`DAT_00aee3a0` and
/// `DAT_00aeef08`, 0x124 a league, 20 entries of 12 the save keeps): his
/// id, his names' pool ids, his count (`L+0x55` and `L+0x56`), his club's
/// country byte and number, the most first. Only players with match bit 2
/// get into the first, with bit 4 and neither of 0x30 into the second.
const PLAYERS_IN: &str = "0xaee380";
const PLAYERS_OUT: &str = "0xaeeee8";

/// The Häufigste Einw. or Ausw. Spieler (`FUN_0044e53f`, `FUN_0044e839`):
/// the league's list as long as an entry holds a player.
pub fn player_substitutions(records: &Records, country: u16, level: u8, out: bool) -> Vec<PlayerCount> {
    let list = if out { PLAYERS_OUT } else { PLAYERS_IN };
    let Some(index) = league_of(records, country, level) else { return Vec::new() };
    let Some(b) = records.block(&format!("{list}[{index}]")) else { return Vec::new() };
    let word = |at: usize| b.get(at..at + 2).map_or(0, |w| u16::from_le_bytes([w[0], w[1]]));
    (0..20)
        .map(|i| i * 12)
        .take_while(|&at| word(at) != 0)
        .map(|at| PlayerCount {
            last: word(at + 4),
            country: b.get(at + 8).copied().unwrap_or(0),
            team: word(at + 0xa) as i16,
            count: word(at + 6) as i16,
        })
        .collect()
}

/// How often the referees favoured each club (`DAT_00b1e290` in the saved
/// lists' block, 0x60 a league): 24 pairs of the club's number and its
/// count, seeded at the season's start in table order; `FUN_007466b2`
/// adds to a club's count after each match, and it can fall below 0.
const REFEREE_LIST: usize = 0x12bd8;

/// The Schirilieblinge (`FUN_0044c57e`): as many of the pairs as the
/// league has clubs, by the page's exchange sort — the most favoured
/// first, every place swapping with the first of the rest that beats it.
pub fn referees_favourites(records: &Records, country: u16, level: u8) -> Vec<ClubCount> {
    let teams = League::of(records, country, level).map_or(0, |l| l.teams()).min(24);
    let Some(b) = list_bytes(records, country, level, REFEREE_LIST, 0x60, 4, 24) else {
        return Vec::new();
    };
    let word = |at: usize| i16::from_le_bytes([b[at], b[at + 1]]);
    let mut rows: Vec<ClubCount> = (0..teams)
        .map(|i| ClubCount { country, team: word(i * 4), values: [word(i * 4 + 2), 0] })
        .collect();
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            if rows[i].values[0] < rows[j].values[0] {
                rows.swap(i, j);
            }
        }
    }
    rows
}

/// The Plätze über Saisonziel (`FUN_0044e23b`): each club's places above
/// its target, the page passing over the entries still at −100 and
/// stopping at the first without a country.
pub fn above_target(records: &Records, country: u16, level: u8) -> Vec<ClubCount> {
    let Some(index) = league_of(records, country, level) else { return Vec::new() };
    let Some(b) = records.block(&format!("{TARGET_CLUBS}[{index}]")) else { return Vec::new() };
    let word = |at: usize| b.get(at..at + 2).map_or(0, |w| i16::from_le_bytes([w[0], w[1]]));
    let mut rows = Vec::new();
    for at in (0..24).map(|i| i * 6) {
        if word(at + 4) == NO_TARGET {
            continue;
        }
        if word(at) == 0 {
            break;
        }
        rows.push(ClubCount { country: word(at) as u16, team: word(at + 2), values: [word(at + 4), 0] });
    }
    rows
}

/// The Tore nach Einwechsl. (`FUN_0044fa30`): each club's goals by
/// substitutes, in all and on the last matchday — no more entries than
/// the league has clubs.
pub fn substitute_goals(records: &Records, country: u16, level: u8) -> Vec<ClubCount> {
    let teams = League::of(records, country, level).map_or(0, |l| l.teams());
    let mut rows = club_list(records, country, level, SUB_GOALS_CLUBS, 8);
    rows.truncate(teams);
    rows
}

/// The Häufigste Einwechsl. (`FUN_0044d9d1`): each club's substitutes
/// brought on and the goals they scored.
pub fn substitutions_in(records: &Records, country: u16, level: u8) -> Vec<ClubCount> {
    club_list(records, country, level, SUBS_IN_CLUBS, 8)
}

/// The Häufigste Auswechsl. (`FUN_0044dc9f`): each club's players taken
/// off.
pub fn substitutions_out(records: &Records, country: u16, level: u8) -> Vec<ClubCount> {
    club_list(records, country, level, SUBS_OUT_CLUBS, 8)
}

/// A club of the Chancenverwertung: its number, its quote in whole
/// percent, its chances and its goals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChanceRow {
    pub team: i16,
    pub quote: i16,
    pub chances: i16,
    pub goals: i16,
}

/// A team's goals this season (a byte at +0xea) and its chances (+0x2c8).
const TEAM_GOALS: usize = 0xea;
const TEAM_CHANCES: usize = 0x2c8;

/// The Chancenverwertung (`FUN_0044c08e`): no saved list, but the
/// league's clubs in the order of its table as `FUN_006430e4` copies
/// them out, each with its goals and chances and the quote of the two,
/// goals·100 / chances cut to a whole number, 0 without a chance — then
/// the page's own exchange sort by the quote, every place from the first
/// up swapping with the first of the rest that beats it, so equal quotes
/// can still change order.
pub fn chancen(records: &Records, country: u16, level: u8) -> Vec<ChanceRow> {
    let Some(league) = League::of(records, country, level) else { return Vec::new() };
    let mut rows: Vec<ChanceRow> = (0..league.teams())
        .map(|place| {
            let team = league.team(place);
            let record = u16::try_from(team).ok().and_then(|t| records.team(country, t));
            let goals = record.map_or(0, |t| i16::from(t.get::<u8>(TEAM_GOALS)));
            let chances = record.map_or(0, |t| t.get::<i16>(TEAM_CHANCES));
            let quote =
                if chances == 0 { 0 } else { (i32::from(goals) * 100 / i32::from(chances)) as i16 };
            ChanceRow { team, quote, chances, goals }
        })
        .collect();
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            if rows[i].quote < rows[j].quote {
                rows.swap(i, j);
            }
        }
    }
    rows
}

/// The card collectors' list (`DAT_00b1283e`): twenty entries of 10 as
/// the keepers', his red, yellow-red and yellow cards behind the club's
/// number (`L+0x33`, `L+0x34`, `L+0x32`).
const TRETER_LIST: usize = 0x7186;

/// The Treter (`FUN_0044b6e0`): the players with a card, their points
/// reckoned on the page — five a red card, three a yellow-red, one a
/// yellow — in the list's order, then the three counts.
pub fn treter(records: &Records, country: u16, level: u8) -> Vec<LigaRow> {
    let Some(b) = list_bytes(records, country, level, TRETER_LIST, 200, 10, 0x14) else {
        return Vec::new();
    };
    (0..0x14)
        .filter_map(|i| {
            let e = &b[i * 10..i * 10 + 10];
            let [red, yellow_red, yellow] = [e[5], e[6], e[7]].map(u16::from);
            let points = red * 5 + yellow_red * 3 + yellow;
            (points != 0).then(|| LigaRow {
                first: u16::from_le_bytes([e[0], e[1]]),
                last: u16::from_le_bytes([e[2], e[3]]),
                team: i16::from(e[4]),
                values: [points, red, yellow_red, yellow, 0, 0],
            })
        })
        .collect()
}

/// The league's scorers and assisters this season (`FUN_0044a3fa`): the
/// saved list's twenty entries; his point, goals, the matchdays he
/// scored and assisted on and his assists stand in the columns.
pub fn scorers(records: &Records, country: u16, level: u8) -> Vec<LigaRow> {
    let Some(b) = list_bytes(records, country, level, SCORER_LIST, SCORER_LEAGUE, 0xc, 0x14) else {
        return Vec::new();
    };
    (0..0x14)
        .filter_map(|i| {
            let e = &b[i * 0xc..i * 0xc + 0xc];
            (e[5] != 0).then(|| LigaRow {
                first: u16::from_le_bytes([e[0], e[1]]),
                last: u16::from_le_bytes([e[2], e[3]]),
                team: i16::from(e[4]),
                values: [e[5], e[6], e[7], e[8], e[9], 0].map(u16::from),
            })
        })
        .collect()
}

/// The table: with `rules` the penalty points and the point for three
/// goals count.
fn reckon(records: &Records, league: &League, kind: u8, rules: bool) -> Vec<Row> {
    let n = league.teams();
    let country = league.country();
    let mut rows: Vec<Row> = (0..n)
        .map(|place| {
            let team = league.team(place);
            let flags = u16::try_from(team)
                .ok()
                .and_then(|t| records.team(country, t))
                .map_or(0, |t| t.get::<u16>(TEAM_FLAGS));
            let points = if rules { -PENALTY_POINTS * i32::from(flags & PENALTY) } else { 0 };
            Row { place, team, flags, points, ..Row::default() }
        })
        .collect();
    let (home_win, away_win) = league.win_points();
    let three_goals = rules && league.flags() & THREE_GOALS != 0;
    // One side's game: its goals and the other's, the points of a win.
    let count = |row: &mut Row, own: i32, other: i32, win: i32, bonus: bool| {
        if own > other {
            row.points += win;
            row.won += 1;
        } else if own == other {
            row.points += 1;
            row.drawn += 1;
        } else {
            row.lost += 1;
        }
        row.goals_for += own;
        row.goals_against += other;
        if bonus && own >= BONUS_GOALS {
            row.points += 1;
        }
    };
    for md in 0..league.played() {
        for (h, a, result) in league.games(md) {
            let Some([gh, ga, hh, ha]) = result else { continue };
            let (gh, ga, hh, ha) = (i32::from(gh), i32::from(ga), i32::from(hh), i32::from(ha));
            if h >= n || a >= n {
                continue;
            }
            match kind {
                HOME => {
                    rows[h].games += 1;
                    count(&mut rows[h], gh, ga, home_win, three_goals);
                }
                AWAY => {
                    rows[a].games += 1;
                    count(&mut rows[a], ga, gh, away_win, three_goals);
                }
                TWO_POINTS => {
                    rows[h].games += 1;
                    rows[a].games += 1;
                    count(&mut rows[h], gh, ga, 2, false);
                    count(&mut rows[a], ga, gh, 2, false);
                }
                _ => {
                    // Every game counts as played, whichever half it is in.
                    rows[h].games += 1;
                    rows[a].games += 1;
                    if (kind == FIRST_ROUND && md >= n - 1) || (kind == SECOND_ROUND && md < n - 1)
                    {
                        continue;
                    }
                    let (gh, ga) = match kind {
                        FIRST_HALF => (hh, ha),
                        SECOND_HALF => (gh - hh, ga - ha),
                        _ => (gh, ga),
                    };
                    count(&mut rows[h], gh, ga, home_win, three_goals);
                    count(&mut rows[a], ga, gh, away_win, three_goals);
                }
            }
        }
    }
    for row in &mut rows {
        row.points = row.points.max(0);
    }
    if league.played() != 0 {
        sort(&mut rows, league.tie_break() == GOALS_FIRST);
    }
    rows
}

/// The bubble sort (`FUN_0072b9b9`) keeps ties in the league's order, as a
/// stable sort does: the sunk teams last, then by points and the rest
/// (`FUN_0072bd12` by the goals scored first, else `FUN_0072bc49`).
fn sort(rows: &mut [Row], goals_first: bool) {
    rows.sort_by(|a, b| {
        let key = |r: &Row| {
            let diff = r.goals_for - r.goals_against;
            if goals_first {
                [r.points, r.goals_for, diff, r.games]
            } else {
                [r.points, diff, r.goals_for, r.games]
            }
        };
        (a.flags & LAST).cmp(&(b.flags & LAST)).then_with(|| key(b).cmp(&key(a)))
    });
}

/// "Meisterprognose" (`FUN_00445ded`): the matchdays to come played out by
/// the places of today's table — no penalty counted until the season is
/// over —, each game's goals drawn by how far apart the two stand, with
/// the random numbers started from the matchdays played, so the forecast
/// stays the same until the next matchday.
pub fn forecast(records: &Records, league: &League) -> Vec<Row> {
    const SEED: i64 = 1000;
    let played = league.played();
    let mut rows = reckon(records, league, ALL, played == league.matchdays());
    let start = SEED + played as i64;
    let mut random = Random::from_state(start, start);
    let ranks: Vec<usize> = {
        let mut by_place = vec![0; league.teams()];
        for (rank, row) in rows.iter().enumerate() {
            if let Some(slot) = by_place.get_mut(row.place) {
                *slot = rank;
            }
        }
        by_place
    };
    let (home_win, away_win) = league.win_points();
    let three_goals = league.flags() & THREE_GOALS != 0;
    for md in played..league.matchdays() {
        for (h, a, _) in league.games(md) {
            let (Some(&rh), Some(&ra)) = (ranks.get(h), ranks.get(a)) else { continue };
            let (gh, ga) = goals(&mut random, rh as i32 - ra as i32);
            let side = |row: &mut Row, own: i32, other: i32, win: i32| {
                if own > other {
                    row.points += win;
                    row.won += 1;
                } else if own == other {
                    row.points += 1;
                    row.drawn += 1;
                } else {
                    row.lost += 1;
                }
                row.goals_for += own;
                row.goals_against += other;
                if three_goals && own >= BONUS_GOALS {
                    row.points += 1;
                }
            };
            side(&mut rows[rh], gh, ga, home_win);
            side(&mut rows[ra], ga, gh, away_win);
        }
    }
    sort(&mut rows, false);
    rows
}

/// A forecast game's goals by how far the home side's place lies below
/// the away side's, `d` (the table in `FUN_00445ded`): a home win, a draw
/// or an away win, by a draw of 100 and the odds of the distance.
fn goals(random: &mut Random, d: i32) -> (i32, i32) {
    // The draw of 100 comes first, even where the distance decides alone.
    let r = random.below(100);
    let mut below = |n: u32| random.below(n) as i32;
    if d < -15 {
        let h = below(4) + 3;
        return (h, below(2));
    }
    if d >= 15 {
        let h = below(2);
        return (h, below(4) + 1);
    }
    // The odds of a home win and of no away win, and the goals each draws.
    let (home, draw, (hp, hq), d_goals, (ap, aq)) = match d {
        -15..=-11 => (80, 100, (5, 2), 2, (0, 0)),
        -10..=-6 => (70, 100, (4, 3), 3, (0, 0)),
        -5..=-1 => (60, 85, (3, 2), 2, (2, 3)),
        0..=4 => (40, 70, (3, 3), 3, (3, 3)),
        5..=9 => (20, 60, (2, 4), 2, (4, 2)),
        _ => (10, 30, (2, 5), 2, (5, 2)),
    };
    if r < home {
        won(random, hp, hq)
    } else if r < draw {
        let g = random.below(d_goals) as i32;
        (g, g)
    } else {
        let (a, h) = won(random, ap, aq);
        (h, a)
    }
}

/// Up to ten draws of the winner's goals and the loser's, until the
/// winner has more; the last stands otherwise (`FUN_0044695f`).
fn won(random: &mut Random, p: u32, q: u32) -> (i32, i32) {
    let mut x = (0, 0);
    for _ in 0..10 {
        let w = random.below(p) as i32;
        x = (w, random.below(q) as i32);
        if x.1 < x.0 {
            break;
        }
    }
    x
}

/// The letters a club's name carries in the tables (`FUN_0063e033`, text
/// 0x5d0 "MEANVCQ"): its flags (team +0x110; an amateur club's in its
/// country's block), the first that is set, then "P" or "F".
pub fn letters(records: &Records, country: u16, team: i16) -> u16 {
    const FLAGS: usize = 0x110;
    const AMATEURS: usize = 0x250;
    const AMATEUR: usize = 0x108;
    let Ok(number) = u16::try_from(team) else { return 0 };
    if let Some(t) = records.team(country, number) {
        return t.get::<u16>(FLAGS);
    }
    let count = usize::try_from(records.countries.team_count(country.into())).unwrap_or(0);
    let slot = records.countries.slot(country.into());
    match (records.countries.kind(country.into()), usize::from(number).checked_sub(count)) {
        (1, Some(k)) => records
            .block(&format!("0x19f7fd0[{slot}]"))
            .and_then(|b| b.get(AMATEURS + k * AMATEUR))
            .map_or(0, |&b| u16::from(b)),
        _ => 0,
    }
}

/// The national cups' pages in the tables' category (`FUN_004469c0`), and
/// the kind of strip that steps through their rounds (`FUN_004599d8`'s
/// cases 0x12..0x16, the cup of the slot typed there).
pub const CUP_PAGES: std::ops::RangeInclusive<i16> = 0x13..=0x17;
pub const CUP_STRIPS: std::ops::RangeInclusive<i16> = 0x12..=0x16;
/// The league cups' pages (`FUN_00446c74`) and their strip
/// (`FUN_004599d8`'s cases 0x1e..0x22): every page shows the one cup the
/// object at 0x1ba7488 keeps, the slot's country only naming it.
pub const LEAGUE_CUP_PAGES: std::ops::RangeInclusive<i16> = 0x21..=0x25;
pub const LEAGUE_CUP_STRIPS: std::ops::RangeInclusive<i16> = 0x1e..=0x22;

/// The cups' rounds by mode (the table at `0x8a5e10`, read by `FUN_006d124d`
/// for the start and `FUN_006d1228` for the count): where each round's
/// pairings stand and how many there are. Its seventh row, the final, is
/// the one the opponent's stand never reads — `opponent::ROUNDS` stops
/// before it.
const CUP_ROUNDS: [[(usize, usize); 7]; 6] = [
    [(0, 0), (0, 32), (32, 16), (48, 8), (56, 4), (60, 2), (62, 1)],
    [(0, 0), (0, 0), (32, 16), (48, 8), (56, 4), (60, 2), (62, 1)],
    [(0, 20), (20, 32), (52, 16), (68, 8), (76, 4), (80, 2), (82, 1)],
    [(0, 0), (20, 32), (52, 16), (68, 8), (76, 4), (80, 2), (82, 1)],
    [(0, 35), (35, 25), (60, 16), (76, 8), (84, 4), (88, 2), (90, 1)],
    [(0, 0), (35, 25), (60, 16), (76, 8), (84, 4), (88, 2), (90, 1)],
];

/// The rounds' names as the pages show them (the table at `0x87cacc`).
pub const CUP_ROUND_NAMES: [u16; 7] = [2118, 2119, 2120, 2121, 2122, 2123, 2124];

/// A pairing on the cup pages (the 0xe bytes `FUN_006d1272` copies to the
/// row `FUN_0044893f`): the two sides, the legs' goals where they were
/// played, and how the pairing was decided beyond full time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CupPairing {
    /// The country and the number of the home and of the away club; a
    /// country of 0 means no club there yet, the round not set.
    pub home_country: u8,
    pub home: i16,
    pub away_country: u8,
    pub away: i16,
    /// The first leg's goals (+8 and +9, flag 0x20) and the second's
    /// (+0xa and +0xb, flag 0x10).
    pub first: Option<(u8, u8)>,
    pub second: Option<(u8, u8)>,
    /// Whether penalties (0x40, "n.E.") or extra time (0x80, "n.V.")
    /// decided it.
    pub penalties: bool,
    pub extra_time: bool,
}

/// The cup record of a slot: `0x1ba5390` and 0x698 a slot, the record's
/// own bytes starting the block at 0x20.
fn cup(records: &Records, slot: usize) -> Option<&[u8]> {
    records.block(&format!("{CUPS_AT:#x}[{slot}]"))
}

/// The one cup record of the league cups' pages: the object at
/// `0x1ba7488`, laid out as a slot's.
fn league_cup(records: &Records) -> Option<&[u8]> {
    records.block("0x1ba7488")
}

/// The rounds a cup is played from and to (+0x26 and +0x28): the strip
/// steps through the names from the one to the other.
pub fn cup_rounds(records: &Records, slot: usize) -> (i16, i16) {
    rounds(cup(records, slot))
}

pub fn league_cup_rounds(records: &Records) -> (i16, i16) {
    rounds(league_cup(records))
}

fn rounds(cup: Option<&[u8]>) -> (i16, i16) {
    let Some(cup) = cup else { return (0, 0) };
    let number = |at: usize| {
        cup.get(at..at + 2)
            .map_or(0, |b| i16::from_le_bytes([b[0], b[1]]))
    };
    (number(0x26 - 0x20), number(0x28 - 0x20))
}

/// The pairings of a cup's round, `start..start + count` as the mode's
/// row in the table has them — including the pairings where no club
/// stands yet, which the page shows as "ohne Verein".
pub fn cup_pairings(records: &Records, slot: usize, round: i16) -> Vec<CupPairing> {
    pairings(cup(records, slot), round)
}

pub fn league_cup_pairings(records: &Records, round: i16) -> Vec<CupPairing> {
    pairings(league_cup(records), round)
}

fn pairings(cup: Option<&[u8]>, round: i16) -> Vec<CupPairing> {
    let Some(cup) = cup else { return Vec::new() };
    let Some((start, count)) = usize::try_from(round)
        .ok()
        .and_then(|r| {
            let m = cup.get(0x24 - 0x20..0x26 - 0x20)?;
            let mode = usize::from(u16::from_le_bytes([m[0], m[1]]));
            CUP_ROUNDS.get(mode)?.get(r).copied()
        })
    else {
        return Vec::new();
    };
    (start..start + count)
        .filter_map(|q| cup.get(PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING))
        .map(pairing)
        .collect()
}

/// One pairing's 0xe bytes: the two clubs by country and number, the
/// legs' goals where they were played, and how it was decided.
fn pairing(p: &[u8]) -> CupPairing {
    CupPairing {
        home_country: p[0],
        home: i16::from_le_bytes([p[2], p[3]]),
        away_country: p[4],
        away: i16::from_le_bytes([p[6], p[7]]),
        first: (p[0xc] & 0x20 != 0).then_some((p[8], p[9])),
        second: (p[0xc] & 0x10 != 0).then_some((p[0xa], p[0xb])),
        penalties: p[0xc] & 0x40 != 0,
        extra_time: p[0xc] & 0x80 != 0,
    }
}

/// The Europa League's page (`FUN_004483c7`) and the European Supercup's
/// (`FUN_0044858a`), and the EL's kind of strip (`FUN_004599d8`'s case 8;
/// the Supercup asks for kind 2, none).
pub const UEFA_PAGE: i16 = 0xf;
pub const SUPERCUP_PAGE: i16 = 0x10;
pub const UEFA_STRIP: i16 = 8;

/// The EL rounds' names as the strip shows them (the table at
/// `0x87ca80`; `0x87ca60` beside it counts the rounds played, one more
/// than the round counter).
pub const UEFA_ROUND_NAMES: [u16; 8] = [2118, 2119, 2120, 4287, 2121, 2122, 2123, 2124];

/// Where the EL keeps its rounds' starts and its pairings (the object at
/// `0x8e8940`, the data of `0x8e8960` and `0x8e8980`), and where the
/// Supercup keeps its one pairing (`0x8e83a2`).
const UEFA_AT: &str = "0x8e8940";
const UEFA_PAIRINGS: usize = 0x20;
const SUPERCUP_AT: &str = "0x8e7c88";
const SUPERCUP_PAIRING: usize = 0x6fa;

/// How many names the EL's strip has: as many rounds as are played, one
/// more than the counter.
pub fn uefa_round_count(records: &Records) -> usize {
    usize::try_from(records.globals.uefa_round()).unwrap_or(0).min(7) + 1
}

/// The pairings the EL page shows for a strip entry (`FUN_004483c7`):
/// `FUN_00448aca` rows of the round between the starts saved in the
/// object at `0x8e8940`. The qualification shows the 41 pairs before
/// round 1's start — 44 once the season runs past its first — and the
/// last round its one pairing.
pub fn uefa_pairings(records: &Records, entry: usize) -> Vec<CupPairing> {
    let Some(uefa) = records.block(UEFA_AT) else { return Vec::new() };
    let start = |i: usize| {
        usize::try_from(
            uefa.get(4 * i..4 * i + 4)
                .map_or(0, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        )
        .unwrap_or(0)
    };
    let count = match entry {
        0 => {
            if records.globals.seasons() > 1 {
                44
            } else {
                41
            }
        }
        7 => 1,
        i => start(i + 1).saturating_sub(start(i)),
    };
    (start(entry)..start(entry) + count)
        .filter_map(|q| uefa.get(UEFA_PAIRINGS + q * PAIRING..UEFA_PAIRINGS + (q + 1) * PAIRING))
        .map(pairing)
        .collect()
}

/// The European Supercup's one pairing (`0x8e83a2`): the two cups'
/// winners, met at the season's start.
pub fn supercup_pairing(records: &Records) -> Option<CupPairing> {
    let at = SUPERCUP_PAIRING;
    records.block(SUPERCUP_AT)?.get(at..at + PAIRING).map(pairing)
}

/// The Champions League's page (`FUN_00448131`) and its kind of strip
/// (`FUN_004599d8`'s case 6).
pub const CL_PAGE: i16 = 0xd;
pub const CL_STRIP: i16 = 6;

/// The CL rounds' names as the strip shows them (the table at
/// `0x87ca40`); `0x87ca30` beside it counts the names by the stage the
/// competition has reached.
pub const CL_ROUND_NAMES: [u16; 7] = [2118, 4800, 4801, 4802, 2122, 2123, 2124];
const CL_STAGE_NAMES: [usize; 8] = [1, 3, 4, 4, 4, 5, 6, 7];

/// Where the CL keeps its pairs (the object at `0x8e91e0`; its member at
/// 0x20, the decompile's pairing base, starts the block's data): the
/// groups' seeds — round 1's eight groups at 0x922, round 2's four at
/// 0xaa2, four 0xc-byte slots 0x30 apart —, the five ends of the
/// pairing runs (0xf28: the qualification's, round 1's, round 2's, the
/// semifinals', the final's), the win points home and away, and the goal
/// bonus's flag.
const CL_AT: &str = "0x8e91e0";
const CL_PAIRINGS: usize = 0x0;
const CL_SEEDS: [usize; 2] = [0x922, 0xaa2];
const CL_RUNS: usize = 0xf28;
const CL_WIN: usize = 0xf44;
const CL_BONUS: usize = 0xf48;

/// How many names the CL's strip has: as many as the stage reached.
pub fn cl_strip_count(records: &Records) -> usize {
    let stage = usize::try_from(records.globals.europe_stage()).unwrap_or(0).min(7);
    CL_STAGE_NAMES[stage]
}

/// Where an entry's pairings start (`FUN_0041e4f4`): the qualification
/// before round 1, the groups and their second round between the runs'
/// ends, the quarter-finals, semifinals and final behind them.
fn cl_start(cl: &[u8], i: usize) -> usize {
    let end = |k: usize| {
        usize::from(cl.get(CL_RUNS + 2 * k..CL_RUNS + 2 * k + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]])))
    };
    match i {
        0 => 0,
        1 => end(0),
        2 | 4 => end(1),
        5 => end(2),
        6 => end(3),
        3 | 7 => end(4),
        _ => end(4) + 1,
    }
}

/// The pairings of the CL page's strip entry (`FUN_00448131`): the
/// qualification and the knock-outs as ranges over the pairings, the
/// knock-out rounds one further than the strip names them — the original
/// notes that step in its own copy of the strip's position, but the
/// replica draws the step from the strip as it stands.
pub fn cl_pairings(records: &Records, entry: usize) -> Vec<CupPairing> {
    let Some(cl) = records.block(CL_AT) else { return Vec::new() };
    let (from, to) = match entry {
        0 => (cl_start(cl, 0), cl_start(cl, 1)),
        e @ 4..=6 => (cl_start(cl, e + 1), cl_start(cl, e + 2)),
        _ => return Vec::new(),
    };
    (from..to)
        .filter_map(|q| cl.get(CL_PAIRINGS + q * PAIRING..CL_PAIRINGS + (q + 1) * PAIRING))
        .map(pairing)
        .collect()
}

/// One group as the page draws it: its number, the pairings played on
/// its six days, and its table.
#[derive(Clone, Debug)]
pub struct ClGroup {
    pub number: usize,
    pub pairings: Vec<CupPairing>,
    pub table: Vec<ClRow>,
}

/// A row of a group's table: the club and the figures the page shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClRow {
    pub country: u8,
    pub club: i16,
    pub games: i16,
    pub points: i16,
    pub goals_for: i16,
    pub goals_against: i16,
}

/// The groups the strip entry stands on (`FUN_00448131` with
/// `FUN_00447c15`): round 1's groups 1-4 or 5-8, or the second round's.
/// A group's twelve pairings stand in six days of two, the day's stride
/// being the round's group count doubled.
pub fn cl_groups(records: &Records, entry: usize) -> Vec<ClGroup> {
    let Some(cl) = records.block(CL_AT) else { return Vec::new() };
    let (round, first) = match entry {
        1 => (0, 0),
        2 => (0, 4),
        3 => (1, 0),
        _ => return Vec::new(),
    };
    let total = if round == 0 { 8 } else { 4 };
    let start = cl_start(cl, if round == 0 { 1 } else { 2 });
    let tables = cl_table(cl, round);
    (first..first + 4)
        .map(|g| ClGroup {
            number: g,
            pairings: (0..6)
                .flat_map(|d| [0, 1].map(|k| start + g * 2 + d * total * 2 + k))
                .filter_map(|q| {
                    cl.get(CL_PAIRINGS + q * PAIRING..CL_PAIRINGS + (q + 1) * PAIRING)
                })
                .map(pairing)
                .collect(),
            table: tables[g].clone(),
        })
        .collect()
}

/// The groups' tables as `FUN_0041d6c1`, `FUN_0041d7f8` and `FUN_0041d931`
/// build them and the page never stores them: the seeds' four clubs each,
/// the played games of the round's pairing run added up, and the order by
/// points, goal difference, goals and games (`FUN_0072bc49`, kept by the
/// game's stable bubble sort).
fn cl_table(cl: &[u8], round: usize) -> Vec<Vec<ClRow>> {
    let total = if round == 0 { 8 } else { 4 };
    let start = cl_start(cl, if round == 0 { 1 } else { 2 });
    let days = if round == 0 { 0x60 } else { 0x30 };
    let win = |away: bool| {
        i16::try_from(
            cl.get(CL_WIN + if away { 2 } else { 0 }..CL_WIN + if away { 4 } else { 2 })
                .map_or(0, |b| u16::from_le_bytes([b[0], b[1]])),
        )
        .unwrap_or(3)
    };
    let bonus = cl
        .get(CL_BONUS..CL_BONUS + 4)
        .is_some_and(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) & 0x80 != 0);
    (0..total)
        .map(|g| {
            let mut rows: Vec<ClRow> = (0..4)
                .map(|k| {
                    let at = CL_SEEDS[round] + g * 0x30 + k * 0xc;
                    ClRow {
                        country: cl.get(at).copied().unwrap_or(0),
                        club: cl
                            .get(at + 2..at + 4)
                            .map_or(0, |b| i16::from_le_bytes([b[0], b[1]])),
                        ..Default::default()
                    }
                })
                .collect();
            for q in start..start + days {
                let Some(p) = cl.get(CL_PAIRINGS + q * PAIRING..CL_PAIRINGS + (q + 1) * PAIRING)
                else {
                    continue;
                };
                if p[0xc] & 0x20 == 0 {
                    continue;
                }
                let team = |c: usize| i16::from_le_bytes([p[c + 2], p[c + 3]]);
                let (a, b) = (i16::from(p[8]), i16::from(p[9]));
                for row in rows.iter_mut() {
                    let home = p[0] == row.country && team(0) == row.club;
                    let away = !home && p[4] == row.country && team(4) == row.club;
                    if !home && !away {
                        continue;
                    }
                    let (own, other) = if home { (a, b) } else { (b, a) };
                    row.games += 1;
                    row.goals_for += own;
                    row.goals_against += other;
                    row.points += if own > other {
                        win(!home)
                    } else if own == other {
                        1
                    } else {
                        0
                    };
                    if bonus && a + b > 3 {
                        row.points += 1;
                    }
                }
            }
            rows.sort_by(|x, y| {
                let key = |r: &ClRow| {
                    (i32::from(r.points), i32::from(r.goals_for - r.goals_against), i32::from(r.goals_for), i32::from(r.games))
                };
                key(y).cmp(&key(x))
            });
            rows
        })
        .collect()
}

/// Whether the club is one of the game's human ones (`FUN_00677428`):
/// a national team, number 200, among the four at `0xb5d17a`, other
/// clubs among those at `0xb5cbe2` — up to `0xb5cc02` of them.
pub fn is_human(records: &Records, country: u8, club: i16) -> bool {
    let at = if club == 200 { 0xb5d17a } else { 0xb5cbe2 };
    let count = if club == 200 { 4 } else { usize::from(records.globals.get::<u16>(0xb5cc02)) };
    (0..count).any(|k| {
        records.globals.get::<u8>(at + 8 * k) == country
            && records.globals.get::<i16>(at + 8 * k + 2) == club
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_graph_orders_and_scales() {
        // Two clubs level at the top keep the league's places, as the
        // exchange sort leaves them; the axis ends at the next 25.
        let sums = [[1, 5, -4, 6], [5, 1, 4, 6], [5, 2, 3, 7]];
        let (rows, axis) = goal_graph(&sums, 0);
        assert_eq!(rows, [(1, 5), (2, 5), (0, 1)]);
        assert_eq!(axis, 25);
        // The difference counts the goals against in as well, and a
        // negative best value still fills the first 25.
        let (rows, axis) = goal_graph(&sums, 2);
        assert_eq!(rows, [(1, 4), (2, 3), (0, -4)]);
        assert_eq!(axis, 25);
        let sums = [[-26, 0, -26, -26], [26, 0, 26, 26]];
        assert_eq!(goal_graph(&sums, 2).1, 50);
        assert_eq!(goal_graph(&[[-26, 0, -26, -26]], 2).1, 25);
    }
}
