//! What a club's tickets will bring for the rest of the season, as the
//! forecasts reckon it (ZUSCHAUERPROGNOSE.md in the RE repo): the gates of
//! its home league matches still to come — the attendance split over the
//! stadium's four classes by their prices, what does not fit spilling into
//! the next, each class paying its price —, and the cup, European and other
//! gates.

use std::io;

use a3_files::exe::Exe;

use crate::calendar::{Calendar, DAYS};
use crate::finances::CLUB_TO_TEAM;
use crate::records::{Records, Team};

/// How much more a match of each mark draws (`DAT_008a7b30`).
const MARKS_AT: u32 = 0x8a7b30;
/// The cups' rounds by the cup's type: the first pairing of a round and
/// how many (`DAT_008a5e10`, `DAT_008a5e14`); whether a country's cup final
/// has two legs, by its cup system (`DAT_008a611c`).
const ROUNDS_AT: u32 = 0x8a5e10;
const ROUNDS_SIZE: usize = 0x400;
const ROUND: usize = 0x10;
const CUP_TYPE: usize = 0x80;
const FINAL_ROUND: usize = 6;
const FINAL_LEGS_AT: u32 = 0x8a611c;
const CUP_SYSTEMS: usize = 0x80;

/// The tables of the attendance model.
pub struct TicketTables {
    marks: [f32; 4],
    rounds: Vec<u8>,
    final_legs: Vec<u16>,
}

impl TicketTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let marks = exe.u32s(MARKS_AT, 4)?;
        Ok(Self {
            marks: std::array::from_fn(|i| f32::from_bits(marks[i])),
            rounds: exe.bytes(ROUNDS_AT, ROUNDS_SIZE)?.to_vec(),
            final_legs: exe.u16s(FINAL_LEGS_AT, CUP_SYSTEMS)?,
        })
    }

    /// A cup type's round: its first pairing and how many.
    fn round(&self, cup_type: usize, round: usize) -> (i32, i32) {
        let at = round * ROUND + cup_type * CUP_TYPE;
        (i32::from(word(&self.rounds, at)), long(&self.rounds, at + 4))
    }

    /// The final's pairing, read as the game reads it there.
    fn final_pairing(&self, cup_type: usize) -> i32 {
        long(&self.rounds, FINAL_ROUND * ROUND + cup_type * CUP_TYPE)
    }
}

/// The club's fields the model reads, by their club offsets (in the team
/// record 0x278 further on): the plan's attendance, its season tickets and
/// its gates so far, the cup, European and other gates planned and booked;
/// the prices of the four classes, the season tickets' discount and a
/// deduction in percent, the matches' marks.
const PLAN_ATTENDANCE: usize = 0x120;
const PLAN_SEASON_TICKETS: usize = 0x124;
const PLAN_START: usize = 0x128;
const PLAN_CUP: usize = 0x12c;
const PLAN_EUROPE: usize = 0x130;
const PLAN_OTHER: usize = 0x134;
const BOOKED_CUP: usize = 0x234;
const BOOKED_EUROPE: usize = 0x238;
const BOOKED_OTHER: usize = 0x23c;
const PRICES: [usize; 4] = [0x4ba, 0x4b0, 0x4a6, 0x49c];
const DISCOUNT: usize = 0x490;
const DEDUCTION: usize = 0x494;
const MARKS: usize = 0x4d8;
const MARKED_MATCHDAYS: i32 = 46;
/// The club's class, which shifts the demand to the dearer seats (the club
/// object's +0x11e0, a byte of the team).
const CLASS: usize = 0x875;
/// The prices each class is reckoned against (statics set at start-up).
const REFERENCE_PRICES: [i32; 4] = [150, 40, 20, 15];
/// A league's record: its teams, matchdays, matchdays played, and the
/// pairings of every matchday, home and away slots.
const LEAGUE_AT: u32 = 0x1a09d48;
const LEAGUE_TEAMS: usize = 0x30;
const LEAGUE_MATCHDAYS: usize = 0x32;
const LEAGUE_PLAYED: usize = 0x46;
const FIXTURES: usize = 0x53a;
const FIXTURE_ROW: usize = 0x18;
/// Leagues this low, or of countries not played, count eighteen teams.
const LOWEST_LEAGUE: u8 = 6;
const DEFAULT_TEAMS: i16 = 18;
/// A stadium: its capacity, the places of its four classes.
const STADIUM_AT: u32 = 0x1a48138;
const CAPACITY: usize = 0x3c;
const CLASS_PLACES: [&[usize]; 4] =
    [&[0x40, 0x42, 0x44, 0x46], &[0x50, 0x52], &[0x54, 0x56], &[0x48, 0x4a, 0x4c, 0x4e]];
/// A cup: its type, its pairings (country, number, home then away), the
/// final's flags and winner.
const CUP_AT: u32 = 0x1ba5390;
const CUP_COUNTRY: usize = 0;
const CUP_TYPE_AT: usize = 4;
const PAIRINGS: usize = 0xc;
const PAIRING: usize = 0xe;
const FINAL_FLAGS: usize = 0x18;
const FINAL_WINNER: usize = 0x19;
const PLAYED_ONE_LEG: u8 = 0x20;
const PLAYED_TWO_LEGS: u8 = 0x30;
/// The European competitions' blocks, which a club taking part appears in.
const EUROPE_AT: [u32; 3] = [0x8e7c88, 0x8e8940, 0x8e91e0];
/// The calendar's codes: what kind of day, a cup round's number, a one-leg
/// tie, the last European rounds.
const KIND: u32 = 0x7f00_0000;
const NATIONAL_CUP: u32 = 0x4000_0000;
const EUROPEAN: u32 = 0x5000_0000;
const LEAGUE_CODES: u32 = 0x3c;
const CUP_ROUND: u32 = 0x3f;
const ONE_LEG: u32 = 0x8_0000;
const FINAL_ONE_LEG: u32 = 0x8008_0000;
const TURNED: u32 = 0x8000_0000;
/// A second leg is played two weeks after the first.
const SECOND_LEG: usize = 14;
/// What the gates come to per match: a part of it, and percent.
const GATE_SHARE: f64 = 0.86;
const PERCENT: f64 = 100.0;
/// Below these weights the demand of a class stays (`FUN_0078aa63`).
const LEAST_WEIGHT: f32 = 0.125;

/// A club as the model sees it.
struct Club<'a> {
    records: &'a Records,
    team: &'a Team,
    country: u8,
    number: u8,
}

impl Club<'_> {
    fn i32_(&self, club: usize) -> i32 {
        self.team.get::<i32>(club + CLUB_TO_TEAM)
    }

    fn i16_(&self, club: usize) -> i16 {
        self.team.get::<i16>(club + CLUB_TO_TEAM)
    }

    fn played(&self) -> bool {
        self.records.globals.played(i16::from(self.country))
    }

    fn league(&self) -> Option<&[u8]> {
        let li = self.records.countries.first_league(usize::from(self.country))
            + i16::from(self.team.level());
        self.records.block(&format!("{LEAGUE_AT:#x}[{li}]"))
    }

    fn stadium(&self) -> Option<&[u8]> {
        let index = self.records.team_index(self.country.into(), self.number.into())?;
        self.records.block(&format!("{STADIUM_AT:#x}[{index}]"))
    }

    fn cup(&self) -> Option<&[u8]> {
        let slot = self.records.countries.slot(usize::from(self.country));
        self.records.block(&format!("{CUP_AT:#x}[{slot}]"))
    }

    /// Whether a side of a pairing is this club.
    fn is(&self, country: u8, number: u16) -> bool {
        country == self.country && number == u16::from(self.number)
    }
}

fn word(block: &[u8], at: usize) -> i16 {
    block.get(at..at + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]))
}

fn long(block: &[u8], at: usize) -> i32 {
    block.get(at..at + 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()))
}

/// `_ftol`: towards zero, the low 32 bits.
fn ftol(v: f64) -> i32 {
    v as i64 as i32
}

/// How much a class's demand changes by its price against the reference
/// one (`FUN_0078aa63`): less when dearer, more when cheaper.
fn weight(price: i16, reference: i32) -> f32 {
    let d = ((i32::from(price) - reference) * 100 / reference) as i16;
    let d = d.clamp(-50, 100);
    let e = 2.71f64.powf(-(f64::from(i32::from(d) * i32::from(d)) / 4900.0)) as f32;
    let e = e.max(LEAST_WEIGHT);
    if d < 0 { (1.0 / f64::from(e)) as f32 } else { e }
}

/// The attendance split over the four classes: season tickets and day
/// tickets (`FUN_007193b2`).
fn split(club: &Club, attendance: i32, season_tickets: i32) -> [[i32; 2]; 4] {
    let k = f64::from(club.team.get::<u8>(CLASS));
    let c1 = (2.0 * k + 3.0) as f32;
    let c2 = (2.0 * k + 30.0) as f32;
    let c3 = (2.0 * k + 10.0) as f32;
    let c4 = (((100.0 - f64::from(c1)) - f64::from(c3)) - f64::from(c2)) as f32;
    let mut c = [c1, c2, c3, c4];
    for (i, c) in c.iter_mut().enumerate() {
        let w = weight(club.i16_(PRICES[i]), REFERENCE_PRICES[i]);
        *c = (f64::from(*c) * f64::from(w)) as f32;
    }
    let s = ((((f64::from(c[0]) + f64::from(c[1])) + f64::from(c[2])) + f64::from(c[3])) / 100.0)
        as f32;
    if s > 0.0 {
        for c in &mut c {
            *c = (f64::from(*c) / f64::from(s)) as f32;
        }
    }
    let rest = attendance.wrapping_sub(season_tickets);
    c.map(|c| {
        [
            ftol(f64::from(season_tickets) * f64::from(c) / 100.0),
            ftol(f64::from(rest) * f64::from(c) / 100.0),
        ]
    })
}

/// What does not fit into a class goes on to the next.
fn spill(b: &mut [[i32; 2]; 4], caps: &[i32; 4], i: usize, j: usize) {
    let [a, d] = b[i];
    if a.wrapping_add(d) > caps[i] {
        if a >= caps[i] {
            b[j][1] = b[j][1].wrapping_add(d);
            b[i][1] = 0;
            b[j][0] = b[j][0].wrapping_add(a - caps[i]);
            b[i][0] = caps[i];
        } else {
            b[j][1] = b[j][1].wrapping_add(d - (caps[i] - a));
            b[i][1] = caps[i] - a;
        }
    }
}

/// The gates from `start`, of the club's home league matches from the
/// matchday after `played` on, at an attendance (`FUN_007195a6`).
fn gates(club: &Club, tables: &TicketTables, start: i32, attendance: i32, played: i16) -> i32 {
    let Some(stadium) = club.stadium() else { return 0 };
    if long(stadium, CAPACITY) <= 0 {
        return 0;
    }
    let mut b = split(club, attendance, club.i32_(PLAN_SEASON_TICKETS));
    let caps: [i32; 4] = CLASS_PLACES
        .map(|places| places.iter().map(|&at| i32::from(word(stadium, at) as u16)).sum());
    spill(&mut b, &caps, 0, 1);
    spill(&mut b, &caps, 1, 2);
    spill(&mut b, &caps, 2, 3);
    // From the dearest back: the game stores the day tickets that do not
    // fit instead of adding them.
    let [a, d] = b[3];
    if a.wrapping_add(d) > caps[3] {
        if a >= caps[3] {
            b[2][1] = b[2][1].wrapping_add(d);
            b[3][1] = 0;
            b[2][0] = b[2][0].wrapping_add(a - caps[3]);
            b[3][0] = caps[3];
        } else {
            b[2][1] = d - (caps[3] - a);
            b[3][1] = caps[3] - a;
        }
    }
    spill(&mut b, &caps, 2, 1);
    spill(&mut b, &caps, 1, 0);
    let [a, d] = b[0];
    if a.wrapping_add(d) > caps[0] {
        if a >= caps[0] {
            b[0][1] = 0;
            b[0][0] = caps[0];
        } else {
            b[0][1] = caps[0] - a;
        }
    }
    let prices = PRICES.map(|at| i32::from(club.i16_(at)));
    let paid = |k: usize| {
        (0..4).rev().fold(0i32, |sum, i| sum.wrapping_add(b[i][k].wrapping_mul(prices[i])))
    };
    let discount = f64::from(100 - i32::from(club.i16_(DISCOUNT)));
    let season = ftol(f64::from(paid(0)) * discount / PERCENT).max(0);
    let day = paid(1).max(0);
    let x = season.wrapping_add(day);
    let Some(league) = club.league() else { return start };
    let (teams, matchdays) = (word(league, LEAGUE_TEAMS), word(league, LEAGUE_MATCHDAYS));
    let slot = club.team.league_place();
    let deduction = f64::from(100 - i32::from(club.i16_(DEDUCTION)));
    let mut total = start;
    for m in played.max(0)..matchdays {
        for k in (0..teams.max(0)).step_by(2) {
            let at = FIXTURES + m as usize * FIXTURE_ROW + k as usize;
            if league.get(at) != Some(&slot) {
                continue;
            }
            let mark = mark(club, i32::from(m) + 1);
            let d = f64::from(x as f32) * f64::from(tables.marks[mark]);
            total = total.wrapping_add(ftol(deduction * d / PERCENT * GATE_SHARE));
        }
    }
    total
}

/// A matchday's mark, 0 to 3 (`FUN_00671b5a`).
fn mark(club: &Club, matchday: i32) -> usize {
    matchday_mark(club.team, matchday)
}

/// A matchday's mark, 0 to 3 (`FUN_00671b5a`): normal, +50%, +25%,
/// halved.
pub fn matchday_mark(team: &Team, matchday: i32) -> usize {
    if !(1..=MARKED_MATCHDAYS).contains(&matchday) {
        return 0;
    }
    let m = (matchday - 1) as usize;
    usize::from((team.get::<u8>(MARKS + CLUB_TO_TEAM + m / 4) >> ((m % 4) * 2)) & 3)
}

/// The opponent of the club in a round of its country's cup, if it plays
/// in it, and whether at home (`FUN_006d1104`); a code with the top bit
/// has the sides the other way round (`FUN_00676b58`).
fn cup_pairing(club: &Club, tables: &TicketTables, code: u32) -> Option<(u8, u16, bool)> {
    let cup = club.cup()?;
    let cup_type = usize::try_from(word(cup, CUP_TYPE_AT)).unwrap_or(0);
    let (first, count) = tables.round(cup_type, (code & CUP_ROUND) as usize);
    let turned = code & TURNED != 0;
    (first..first.saturating_add(count)).find_map(|q| {
        let at = PAIRINGS + usize::try_from(q).unwrap_or(0) * PAIRING;
        let side = |o: usize| (cup.get(at + o).copied().unwrap_or(0), word(cup, at + o + 2) as u16);
        let ((hc, hn), (ac, an)) = (side(0), side(4));
        if club.is(hc, hn) {
            Some((ac, an, !turned))
        } else if club.is(ac, an) {
            Some((hc, hn, turned))
        } else {
            None
        }
    })
}

/// The club's cup tie on a cup day, if it plays one (`FUN_0062d181`): not
/// when the international row or a league matchday has the day.
fn cup_tie_on(
    club: &Club,
    tables: &TicketTables,
    calendar: &Calendar,
    row: usize,
    day: usize,
) -> Option<(u8, u16, bool)> {
    if calendar.entry(0, day) != 0 {
        return None;
    }
    let league = calendar.row(club.country.into(), club.team.level().into());
    if league.is_some_and(|r| {
        let e = calendar.entry(r, day);
        e != 0 && e < LEAGUE_CODES
    }) {
        return None;
    }
    let e = calendar.entry(row, day);
    if e & KIND != NATIONAL_CUP {
        return None;
    }
    cup_pairing(club, tables, e)
}

/// Whether the club plays a cup tie on a cup day.
fn plays_cup(
    club: &Club,
    tables: &TicketTables,
    calendar: &Calendar,
    row: usize,
    day: usize,
) -> bool {
    cup_tie_on(club, tables, calendar, row, day).is_some()
}

/// The ties ahead in the country's cup, counted at its next round's day
/// (`FUN_0062b8e0`); the pre-season's tournaments are not counted.
fn cup_ahead(club: &Club, tables: &TicketTables, calendar: &Calendar, from: usize) -> i32 {
    if club.records.countries.kind(usize::from(club.country)) != 1 {
        return 0;
    }
    let Some(row) = calendar.row(club.country.into(), 0) else { return 0 };
    for day in from..DAYS {
        let e = calendar.entry(row, day);
        if e & KIND != NATIONAL_CUP {
            continue;
        }
        let mut k = 0;
        if plays_cup(club, tables, calendar, row, day) {
            if (e & 0x1f) == FINAL_ROUND as u32 && (e & FINAL_ONE_LEG) == ONE_LEG {
                return k + 1;
            }
            k += 1;
            if e & ONE_LEG == 0 {
                k += 1;
            }
        }
        return k;
    }
    0
}

/// Whether European home ties lie ahead (`FUN_0062bd73`). A club in none
/// of the competitions has none; for one that takes part the game's
/// lookup is not traced, and ties are taken to lie ahead while European
/// days do.
fn europe_ahead(club: &Club, calendar: &Calendar, from: usize) -> bool {
    let taking_part = EUROPE_AT.iter().any(|&at| {
        club.records.block(&format!("{at:#x}")).is_some_and(|b| {
            (0..b.len().saturating_sub(3))
                .step_by(2)
                .any(|o| club.is(b[o], u16::from_le_bytes([b[o + 2], b[o + 3]])) && b[o + 1] == 0)
        })
    });
    taking_part && (from..DAYS).any(|day| calendar.entry(0, day) & KIND == EUROPEAN)
}

/// The cup, European and other gates: the plan's where a tie lies ahead
/// and it is more, else as booked.
fn tail(club: &Club, tables: &TicketTables, calendar: &Calendar, from: usize) -> i32 {
    let larger = |plan: usize, booked: usize| club.i32_(plan).max(club.i32_(booked));
    let cup = if cup_ahead(club, tables, calendar, from) != 0 {
        larger(PLAN_CUP, BOOKED_CUP)
    } else {
        club.i32_(BOOKED_CUP)
    };
    let europe = if europe_ahead(club, calendar, from) {
        larger(PLAN_EUROPE, BOOKED_EUROPE)
    } else {
        club.i32_(BOOKED_EUROPE)
    };
    cup.wrapping_add(europe).wrapping_add(larger(PLAN_OTHER, BOOKED_OTHER))
}

/// What a club's tickets bring: carried on, at the plan's attendance
/// (`FUN_0061fdfa`), and extrapolated, at the mean gate so far, rounded
/// (`FUN_00620e9e`); and the plan's league gates of the whole season
/// (`FUN_0061f427`).
pub struct Gates {
    pub carried_on: i32,
    pub extrapolated: i32,
    pub planned: i32,
}

pub fn gates_of(
    records: &Records,
    team: &Team,
    tables: &TicketTables,
    calendar: &Calendar,
) -> Gates {
    let club = Club { records, team, country: team.country(), number: team.number() };
    let played = club.league().map_or(0, |l| word(l, LEAGUE_PLAYED));
    let counted = if team.level() >= LOWEST_LEAGUE || !club.played() { 0 } else { played };
    let from = usize::from(records.globals.day()) + 1;
    let tail = tail(&club, tables, calendar, from);
    let start = club.i32_(PLAN_START);
    let attendance = club.i32_(PLAN_ATTENDANCE);
    let carried_on = gates(&club, tables, start, attendance, counted).wrapping_add(tail);
    let games = team.home_games();
    let mean = if games != 0 {
        let sum =
            team.gates().iter().take(usize::from(games)).fold(0i32, |s, &g| s.wrapping_add(g));
        sum / i32::from(games)
    } else {
        attendance
    };
    let extrapolated =
        gates(&club, tables, start, mean, played).wrapping_add(tail) / 10_000 * 10_000;
    let planned = gates(&club, tables, 0, attendance, 0)
        .wrapping_add(club.i32_(PLAN_CUP))
        .wrapping_add(club.i32_(PLAN_EUROPE))
        .wrapping_add(club.i32_(PLAN_OTHER));
    Gates { carried_on, extrapolated, planned }
}

/// How many teams the club's league has, eighteen for a low or unplayed
/// one (`FUN_00635a35`).
pub fn league_teams(records: &Records, team: &Team) -> i16 {
    let club = Club { records, team, country: team.country(), number: team.number() };
    if !club.played() || team.level() >= LOWEST_LEAGUE {
        return DEFAULT_TEAMS;
    }
    club.league().map_or(DEFAULT_TEAMS, |l| word(l, LEAGUE_TEAMS))
}

/// The number of the club that won this season's cup final of a country,
/// none before it is played (`FUN_006d13bf`).
pub fn cup_winner(records: &Records, country: u8, tables: &TicketTables) -> Option<u16> {
    let slot = records.countries.slot(usize::from(country));
    let cup = records.block(&format!("{CUP_AT:#x}[{slot}]"))?;
    let cup_type = usize::try_from(word(cup, CUP_TYPE_AT)).ok()?;
    let at = usize::try_from(tables.final_pairing(cup_type)).ok()? * PAIRING;
    let system = records.countries.cup_system(usize::try_from(word(cup, CUP_COUNTRY)).ok()?);
    let two_legs = tables.final_legs.get(usize::from(system)).is_some_and(|&l| l != 0);
    let played = if two_legs { PLAYED_TWO_LEGS } else { PLAYED_ONE_LEG };
    if cup.get(FINAL_FLAGS + at)? & played != played {
        return None;
    }
    let winner = if *cup.get(FINAL_WINNER + at)? == 0 { 0xe } else { 0x12 };
    Some(word(cup, winner + at) as u16)
}

/// The matchdays of a club's league, and those played: none for a low or
/// unplayed league (`FUN_00692997`, `FUN_00692744`).
pub fn league_days(records: &Records, team: &Team) -> (i32, i32) {
    let club = Club { records, team, country: team.country(), number: team.number() };
    let Some(league) = club.league() else { return (0, 0) };
    let matchdays = i32::from(word(league, LEAGUE_MATCHDAYS));
    let played = if team.level() >= LOWEST_LEAGUE || !club.played() {
        0
    } else {
        i32::from(word(league, LEAGUE_PLAYED))
    };
    (matchdays, played)
}

/// A tie of a club in its country's cup: the opponent's country and
/// number, whether at home, its day, whether the one-leg final.
#[derive(Clone, Copy, Debug)]
pub struct CupTie {
    pub country: u8,
    pub number: u16,
    pub home: bool,
    pub day: usize,
    pub final_tie: bool,
}

/// The club's ties at the next round of its country's cup from a day on,
/// a second leg two weeks after the first (`FUN_0062b8e0`). The
/// pre-season's tournaments (`FUN_006ccbf0`) are not traced and not
/// listed.
pub fn cup_ties(
    records: &Records,
    team: &Team,
    tables: &TicketTables,
    calendar: &Calendar,
    from: usize,
) -> Vec<CupTie> {
    let club = Club { records, team, country: team.country(), number: team.number() };
    if records.countries.kind(usize::from(club.country)) != 1 {
        return Vec::new();
    }
    let Some(row) = calendar.row(club.country.into(), 0) else { return Vec::new() };
    let mut ties = Vec::new();
    for day in from..DAYS {
        let e = calendar.entry(row, day);
        if e & KIND != NATIONAL_CUP {
            continue;
        }
        if let Some((country, number, home)) = cup_tie_on(&club, tables, calendar, row, day) {
            let final_tie = (e & 0x1f) == FINAL_ROUND as u32 && (e & FINAL_ONE_LEG) == ONE_LEG;
            let first = CupTie { country, number, home, day, final_tie };
            ties.push(first);
            if !final_tie && e & ONE_LEG == 0 {
                ties.push(CupTie { home: !home, day: day + SECOND_LEG, ..first });
            }
        }
        break;
    }
    ties
}

/// The ticket office's sales (`0x1c5c468`, thirty records of 0x24 bytes):
/// the home side, the away side, the day, the tickets sold, whether either
/// order of the sides matches, the four prices.
const SALES_AT: &str = "0x1c5c468";
const SALES: usize = 30;
const SALE: usize = 0x24;
const SALE_DAY: usize = 0xc;
const SALE_SOLD: usize = 0x14;
const SALE_OPEN: usize = 0x18;
const SALE_PRICES: usize = 0x1a;

/// A match's advance sales: the tickets sold and the prices of the four
/// classes, standing first.
#[derive(Clone, Copy, Debug)]
pub struct Sale {
    pub sold: i32,
    pub prices: [i16; 4],
}

/// The advance sales of a match, if the office has a record of them
/// (`FUN_0077c9c2`): an open record matches the sides either way round.
pub fn sale(records: &Records, home: (u8, i16), away: (u8, i16), day: i16) -> Option<Sale> {
    let block = records.block(SALES_AT)?;
    (0..SALES).find_map(|i| {
        let r = block.get(i * SALE..(i + 1) * SALE)?;
        let side = |o: usize| (r[o], word(r, o + 2));
        if word(r, SALE_DAY) != day {
            return None;
        }
        let (a, b) = (side(0), side(4));
        let open = word(r, SALE_OPEN) != 0;
        let found = (a == home && b == away) || (open && a == away && b == home);
        found.then(|| Sale {
            sold: long(r, SALE_SOLD),
            prices: std::array::from_fn(|k| word(r, SALE_PRICES + 2 * k)),
        })
    })
}

/// Whether more tickets were sold ahead for a home match of the club from
/// today on than `places` (`FUN_0077d354`).
pub fn presold_above(records: &Records, club: (u8, i16), today: i16, places: i32) -> bool {
    let Some(block) = records.block(SALES_AT) else { return false };
    (0..SALES).any(|i| {
        block.get(i * SALE..(i + 1) * SALE).is_some_and(|r| {
            (r[0], word(r, 2)) == club && word(r, SALE_DAY) >= today && long(r, SALE_SOLD) > places
        })
    })
}

/// The prices a class can be given (`FUN_00686684`, the factor 1.0):
/// standing 5 to 40, seats 10 to 90, boxes 50 to 210 by twos.
pub fn price_table(class: usize) -> Vec<i16> {
    match class {
        0 => (5..=40).collect(),
        3 => (50..=210).step_by(2).collect(),
        _ => (10..=90).collect(),
    }
}

/// Where a price stands in its table (`FUN_00678260` and its kin): at the
/// first entry it does not pass, short of the last two; the top two put it
/// below them.
pub fn price_step(table: &[i16], price: i16) -> usize {
    let last = table.len().saturating_sub(1);
    (0..last.saturating_sub(2)).find(|&i| price <= table[i]).unwrap_or(last.saturating_sub(2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_price_stands_at_its_step_but_the_top_two_below() {
        let standing = price_table(0);
        assert_eq!((standing.len(), price_table(1).len(), price_table(3).len()), (36, 81, 81));
        assert_eq!(price_step(&standing, 17), 12);
        assert_eq!(price_step(&standing, 37), 32);
        assert_eq!(price_step(&standing, 39), 33);
        assert_eq!(price_step(&standing, 40), 33);
        assert_eq!(price_step(&price_table(3), 150), 50);
        assert_eq!(price_step(&price_table(2), 43), 33);
    }
}
