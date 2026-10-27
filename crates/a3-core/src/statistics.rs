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

use crate::opponent::{CUPS_AT, League, PAIRING, PAIRINGS};
use crate::random::Random;
use crate::records::{Manager, Records, Team};

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
