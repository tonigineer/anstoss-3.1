//! What a player makes of his contract (VERLAENGERUNG-ABWERBEN.md §1 in the
//! RE repo): the salary he would ask (`FUN_00708012`), how well his
//! contract suits him (`FUN_00708ca5`, about 100 when it is what he
//! asks), how well new terms would (`FUN_00709299`, `FUN_00709b5f`), and
//! whether he takes a year more on his terms (`FUN_004b784c`). Also the
//! talks the human managers have booked with their boards about their own
//! contracts (VEREINSFUEHRUNG.md §4).

use std::io;

use a3_files::exe::Exe;

use crate::records::{Appointment, Records};
use crate::value::{ValueTables, Worth};
use crate::world::GOALKEEPER;

const DEMAND_AT: u32 = 0x8a7700;
const DEMANDS: usize = 16;
const POSITION_AT: u32 = 0x8a7658;
const POSITIONS: usize = 11;
const ABILITIES_AT: u32 = 0x8a76b0;
const CHARACTER_AT: [u32; 2] = [0x8a76d0, 0x8a76f0];
const BITS: usize = 16;
const KEEPER_BITS: usize = 9;
/// The index's weights by age class: salary, bonuses, salary again, fee.
const WEIGHTS_AT: u32 = 0x8a77c8;
const AGE_CLASSES: usize = 4;
/// How the later years of new terms count, by age class (f32).
const YEARS_AT: u32 = 0x8a7934;
const YEAR_CLASSES: usize = 3;
const MONTHS_AT: u32 = 0x89e220;
const MONTHS: usize = 13;
/// What contentment adds to the will to stay, by its level (the table at
/// 0x8819b4).
const CONTENT: [i32; 4] = [15, 0, -15, -35];
/// The units of the demand and the bonuses; the demand's steps.
const THOUSAND: i32 = 1000;
const HUNDRED: i32 = 100;
const SMALL_STEP: i32 = 5000;
const BIG_STEP: i32 = 10_000;
const BIG: f64 = 50_000.0;

/// The terms' bits: a player's option, a club's option, a regular place,
/// the halving spared.
pub const PLAYER_OPTION: u16 = 0x1000;
pub const CLUB_OPTION: u16 = 0x2000;
pub const REGULAR_PLACE: u16 = 0x4000;
const UNHALVED: u16 = 0x20;
/// The knocks word's bits the bonuses read.
const KNOCK_UP: u16 = 2;
const KNOCK_DOWN: u16 = 4;
/// A bad character trait (traits[1] bit 6): no extension by luck.
const DIFFICULT: u16 = 0x40;
/// The record's bit of a player ending his career.
const RETIRING: u8 = 0x80;

pub struct ContractTables {
    demand: Vec<i16>,
    position: Vec<f64>,
    abilities: Vec<i8>,
    character: [Vec<i8>; 2],
    weights: Vec<[f64; 4]>,
    years: Vec<[f32; 4]>,
    months: Vec<u8>,
}

impl ContractTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let f64s = |at: u32, n: usize| -> io::Result<Vec<f64>> {
            Ok(exe
                .bytes(at, 8 * n)?
                .chunks(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                .collect())
        };
        let f32s = |at: u32, n: usize| -> io::Result<Vec<f32>> {
            Ok(exe
                .bytes(at, 4 * n)?
                .chunks(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                .collect())
        };
        let signed = |at: u32, n: usize| -> io::Result<Vec<i8>> {
            Ok(exe.bytes(at, n)?.iter().map(|&b| b as i8).collect())
        };
        let weights = f64s(WEIGHTS_AT, 4 * AGE_CLASSES)?;
        let years = f32s(YEARS_AT, 4 * YEAR_CLASSES)?;
        Ok(Self {
            demand: exe.u16s(DEMAND_AT, DEMANDS)?.into_iter().map(|v| v as i16).collect(),
            position: f64s(POSITION_AT, POSITIONS)?,
            abilities: signed(ABILITIES_AT, 2 * BITS)?,
            character: [signed(CHARACTER_AT[0], 2 * BITS)?, signed(CHARACTER_AT[1], BITS)?],
            weights: weights.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect(),
            years: years.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect(),
            months: exe.bytes(MONTHS_AT, MONTHS)?.to_vec(),
        })
    }
}

/// A contract's terms: salary (in 1000s), appearance and goal bonuses (in
/// 100s), release fee (in 10.000s), years left, the terms' bits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Terms {
    pub salary: u16,
    pub appearance: u16,
    pub goal: u16,
    pub fee: u16,
    pub years: i8,
    pub flags: u16,
}

/// A player as his contract's reckoning sees him.
#[derive(Clone, Copy, Debug)]
pub struct Signee {
    pub id: u16,
    pub strength: u8,
    pub position: u8,
    pub talent: i8,
    pub age: i32,
    pub good: u16,
    pub bad: u16,
    pub traits: [u16; 2],
    pub knocks: u16,
    pub terms: Terms,
    /// The least his will to stay must reach (R+0x160).
    pub floor: u8,
    /// His season's form summed, and the weeks summed.
    pub season_form: i16,
    pub season_weeks: u8,
    pub retiring: bool,
    pub form: i32,
    pub content_level: usize,
    /// What he is worth a month, as the market value counts it.
    pub monthly: i64,
}

/// The club around him: its league level, its squad's positions and
/// strengths (his place among them), the matchdays of its league and
/// those played, the month.
pub struct Club<'a> {
    pub level: u8,
    pub squad: &'a [(u16, u8, u8)],
    pub matchdays: i32,
    pub played: i32,
    pub month: usize,
}

impl Signee {
    /// From his record, with what his market value needs.
    pub fn of(
        r: &crate::records::Player,
        age: i32,
        form: i32,
        values: &ValueTables,
        star: bool,
        start_year: u16,
    ) -> Self {
        let [main, ..] = r.positions();
        let years = ((i32::from(r.contract() & 0xfff) - i32::from(start_year)) as i8).max(0);
        let worth = Worth {
            strength: r.strength(),
            position: main,
            talent: r.talent(),
            age,
            good: r.good(),
            character: r.traits(),
            qualities: r.qualities(),
            star,
            kind_12_matches: r.kind_12_matches(),
        };
        Self {
            id: r.id(),
            strength: r.strength(),
            position: main,
            talent: r.talent(),
            age,
            good: r.good(),
            bad: r.bad(),
            traits: r.traits(),
            knocks: r.knocks(),
            terms: Terms {
                salary: r.salary(),
                appearance: r.appearance_bonus(),
                goal: r.goal_bonus(),
                fee: r.fixed_fee(),
                years,
                flags: r.terms(),
            },
            floor: r.get::<u8>(0x160),
            season_form: r.season_form(),
            season_weeks: r.season_form_weeks(),
            retiring: r.retiring() & RETIRING != 0,
            form,
            content_level: crate::mood::content_level(crate::mood::parts(r)),
            monthly: values.monthly(&worth),
        }
    }

    fn age_class(&self) -> usize {
        match self.age {
            ..24 => 0,
            24..=30 => 1,
            31..=36 => 2,
            _ => 3,
        }
    }
}

/// `_ftol`: towards zero.
fn ftol(v: f64) -> i32 {
    v as i64 as i32
}

impl ContractTables {
    /// The yearly salary he would ask (`FUN_00708012`).
    pub fn demand(&self, p: &Signee) -> i32 {
        let position = if (1..=10).contains(&p.position) {
            self.position[usize::from(p.position)]
        } else {
            1.0
        };
        let strength = self.demand.get(usize::from(p.strength)).copied().unwrap_or(0);
        let mut x = f64::from(i32::from(strength) * THOUSAND) * position;
        let (a, s) = (p.age, p.strength);
        x *= match p.talent {
            0 if s <= 11 => match a {
                ..=20 => 1.5,
                21..=25 => 1.35,
                26..=29 => 1.2,
                _ => 1.0,
            },
            0 => 1.0,
            1 if s <= 10 => match a {
                ..=20 => 1.35,
                21..=25 => 1.2,
                _ => 1.0,
            },
            1 => 1.0,
            3 if s <= 6 && a <= 25 => 0.9,
            3 => 1.0,
            4 if s <= 5 && a <= 25 => 0.75,
            4 => 1.0,
            _ if s <= 9 => 1.1,
            _ => 1.0,
        };
        match a {
            29 => x *= 0.99,
            30 => x *= 0.95,
            31 => x *= 0.9,
            32 => x *= 0.85,
            33 => x *= 0.8,
            34 => x *= 0.75,
            35 => x *= 0.7,
            36.. => x *= 0.6,
            ..25 => x += f64::from(ftol(f64::from(25 - a) * 0.05 * x)),
            _ => {}
        }
        let keeper = usize::from(p.position == GOALKEEPER);
        let mut sum = 0i32;
        for b in 0..BITS {
            if p.good & (1 << b) != 0 {
                sum += i32::from(self.abilities[BITS * keeper + b]);
            }
            if p.traits[0] & (1 << b) != 0 {
                sum += i32::from(self.character[0][BITS * keeper + b]);
            }
            if p.traits[1] & (1 << b) != 0 {
                sum += i32::from(self.character[1][b]);
            }
        }
        x = (f64::from(sum) + 100.0) * x / 100.0;
        let step = if x < BIG { SMALL_STEP } else { BIG_STEP };
        (ftol(x) / step * step).max(SMALL_STEP)
    }

    /// The months a contract of so many years still runs
    /// (`FUN_00707f50`).
    fn months(&self, years: i8, month: usize) -> i32 {
        let first = i32::from(self.months.get(month).copied().unwrap_or(0));
        match years {
            ..=0 => 0,
            1 => first,
            y => (i32::from(y) - 1) * 12 + first,
        }
    }

    /// Rivals at his position at least as strong, and players elsewhere
    /// at least as strong.
    fn rivals(p: &Signee, club: &Club) -> (i32, i32) {
        let same = club
            .squad
            .iter()
            .filter(|&&(id, pos, s)| pos == p.position && s >= p.strength && id != p.id)
            .count() as i32;
        let better =
            club.squad.iter().filter(|&&(_, pos, s)| pos != p.position && s >= p.strength).count()
                as i32;
        (same, better)
    }

    /// The bonuses' part: the appearance bonus over the matches it is
    /// paid for, against the demand, the less the more rivals he has.
    fn bonuses(&self, p: &Signee, club: &Club, appearance: u16, matches: f64) -> (f64, i32) {
        let b = self.demand(p) / HUNDRED;
        if b == 0 {
            return (1.0, b);
        }
        let w = self.weights[p.age_class()][1];
        let div = match club.level {
            0 => 2.0,
            1 => 3.0,
            _ => 4.0,
        };
        let mut x = f64::from(appearance) * matches * w / (f64::from(b) / div);
        let (same, better) = Self::rivals(p, club);
        if same > 0 {
            x *= if same == 1 {
                0.85
            } else if same + better < 16 {
                0.75
            } else {
                0.6
            };
        }
        if p.knocks & KNOCK_UP != 0 {
            x *= 1.1;
        } else if p.knocks & KNOCK_DOWN != 0 {
            x *= 0.8;
        }
        (x, b)
    }

    /// The release fee's part: the more of his worth it asks, the less.
    fn fee(&self, p: &Signee, fee: u16, years: i8, month: usize, w3: f64, s: f64) -> f64 {
        let rv = f64::from(self.months(years, month) - 12) * p.monthly as f64 * 1.2;
        let f = f64::from(fee);
        if fee == 0 || 3.0 * rv < f {
            s + w3
        } else if 1.8 * rv < f {
            s + w3 * 0.2
        } else if 1.4 * rv < f {
            s + w3 * 0.3
        } else if 1.3 * rv < f {
            s + w3 * 0.4
        } else if 1.1 * rv < f {
            s + w3 * 0.6
        } else if f >= rv {
            s + w3
        } else if f >= 0.8 * rv {
            s + w3 * 1.2
        } else {
            s + w3 * 1.5
        }
    }

    /// The goal bonus's part, and the options' and a regular place's.
    fn finish(&self, p: &Signee, club: &Club, t: &Terms, b: i32, mut s: f64) -> f64 {
        let (same, _) = Self::rivals(p, club);
        if t.flags & REGULAR_PLACE != 0 {
            s *= if same == 0 { 1.05 } else { 1.2 };
        }
        if b != 0 {
            if p.position == 10 {
                s *= f64::from(t.goal) / f64::from(b) + 1.0;
            } else if p.position != GOALKEEPER {
                s *= f64::from(t.goal) / f64::from(2 * b) + 1.0;
            }
        }
        s
    }

    /// How well his contract suits him, about 100 when it is what he
    /// would ask (`FUN_00708ca5`).
    pub fn index(&self, p: &Signee, club: &Club) -> i32 {
        let w = self.weights[p.age_class()];
        let d = self.demand(p) / THOUSAND;
        let t = &p.terms;
        let mut s = if d != 0 { f64::from(t.salary) * w[0] / f64::from(d) } else { 1.0 };
        let (x, b) = self.bonuses(p, club, t.appearance, 40.0);
        s += x;
        let paid = if d != 0 { f64::from(t.salary) / f64::from(d) } else { 1.0 };
        s += paid * w[2];
        s = self.fee(p, t.fee, t.years, club.month, w[3], s);
        s = self.finish(p, club, t, b, s);
        if t.flags & PLAYER_OPTION != 0 {
            s *= 1.05;
        }
        ftol(s)
    }

    /// How well new terms would suit him (`FUN_00709299`).
    fn terms(&self, p: &Signee, club: &Club, t: &Terms, a: i32) -> i32 {
        let w = self.weights[p.age_class()];
        let d = self.demand(p) / THOUSAND;
        let mut s = if d != 0 { f64::from(t.salary) * w[0] / f64::from(d) } else { 1.0 };
        let last = (t.flags & UNHALVED == 0 && p.terms.years == 1) || p.terms.years < 1;
        if last {
            if d != 0 {
                let y = (f64::from(a) * 15.0 / f64::from(2 * d)).min(45.0);
                s = s / 2.0 + y;
            }
        } else if d != 0 {
            s += (f64::from(a) * 15.0 / f64::from(d)).min(30.0);
        }
        let (x, b) = self.bonuses(p, club, t.appearance, f64::from(club.matchdays));
        s += x;
        s = self.fee(p, t.fee, t.years, club.month, w[3], s);
        s = self.finish(p, club, t, b, s);
        if d > 0 && d > 2 * i32::from(t.salary) {
            s /= 2.0;
        }
        if t.flags & PLAYER_OPTION != 0 {
            s *= 1.05;
        } else if t.flags & CLUB_OPTION != 0 {
            s *= 0.92;
        }
        ftol(s)
    }

    /// The same over the new contract's years, the later counting by his
    /// age (`FUN_00709b5f`).
    pub fn satisfaction(&self, p: &Signee, club: &Club, t: &Terms) -> i32 {
        let v = self.terms(p, club, t, 0) as f32;
        let n = i32::from(t.years);
        if n < 1 {
            return v as i32;
        }
        let row = &self.years[match p.age {
            ..25 => 0,
            25..32 => 1,
            _ => 2,
        }];
        let sum = (0..n).fold(0.0f32, |sum, k| v * row[(k as usize).min(3)] + sum);
        (f64::from(sum) / f64::from(n)) as i32
    }

    /// Whether he takes one year more on his terms (`FUN_004b784c`): sure,
    /// never, or by luck below `chance` in a hundred.
    pub fn extension(&self, p: &Signee, club: &Club) -> Extension {
        let count = if p.position == GOALKEEPER { KEEPER_BITS } else { BITS };
        let bits = |w: u16| (0..count).filter(|&b| w & (1 << b) != 0).count() as i32;
        let mut need = i32::from(p.strength) + 100 + bits(p.good) + bits(p.bad);
        if p.form > 12 {
            need += 8;
        }
        if p.form > 15 {
            need += 2;
        }
        if club.played > 10 {
            let g = if p.season_weeks == 0 {
                100
            } else {
                i32::from(p.season_form) * 10 / i32::from(p.season_weeks)
            };
            let g = (f64::from(g) / 10.0) as f32;
            if g > 12.0 {
                need += ftol((f64::from(g) - 12.0) * 5.0);
            }
        }
        if p.age < 25 {
            need += (25 - p.age) * 3;
        }
        if p.retiring {
            need += 20;
        }
        if p.traits[1] & DIFFICULT != 0 {
            need += 20;
        }
        let m = CONTENT.get(p.content_level).copied().unwrap_or(0);
        let t = Terms { years: p.terms.years + 1, ..p.terms };
        let w = self.satisfaction(p, club, &t);
        if need <= w + m && i32::from(p.floor) * 8 / 10 <= w {
            return Extension::Yes;
        }
        if need <= w + m.min(0) && p.traits[1] & DIFFICULT == 0 {
            return Extension::ByLuck(if p.age > 30 { 35 } else { 15 });
        }
        Extension::No
    }
}

/// A player's answer to a year more.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Extension {
    Yes,
    No,
    /// Yes if a draw below 100 falls under this.
    ByLuck(u32),
}

/// The talks about their contracts the human managers book with their
/// boards (VEREINSFUEHRUNG.md §4): all share the save block `0x1ba0438`,
/// whose data begins at the object's +0x20 — an i16 count up to 28, then
/// ten bytes a talk: the manager's slot from one, the day, the club's
/// country and number, and nought.
const TALKS_AT: u32 = 0x1ba0438;
const TALKS_COUNT: usize = 0;
const TALKS_FIRST: usize = 2;
const TALK: usize = 10;
const TALKS_MOST: usize = 28;
/// The planner's kind of a contract talk, which names a club: its country
/// in the entry's player field, its number after (`439`'s booking and the
/// market's negotiations share it).
pub const TALK_KIND: u8 = crate::records::PLANNER_CLUB_KIND;

/// One talk of the list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Talk {
    pub slot: i16,
    pub day: i16,
    pub club: (u8, i16),
}

fn talks(records: &Records) -> Vec<Talk> {
    let Some(b) = records.block(&format!("{TALKS_AT:#x}")) else { return Vec::new() };
    let count = word(b, TALKS_COUNT).max(0) as usize;
    (0..count)
        .map(|k| {
            let e = TALKS_FIRST + k * TALK;
            Talk {
                slot: word(b, e),
                day: word(b, e + 2),
                club: (b.get(e + 4).copied().unwrap_or(0), word(b, e + 5)),
            }
        })
        .collect()
}

fn word(b: &[u8], at: usize) -> i16 {
    match (b.get(at), b.get(at + 1)) {
        (Some(lo), Some(hi)) => i16::from_le_bytes([*lo, *hi]),
        _ => 0,
    }
}

fn put16(b: &mut [u8], at: usize, v: i16) {
    if let Some(slot) = b.get_mut(at..at + 2) {
        slot.copy_from_slice(&v.to_le_bytes());
    }
}

/// The manager's slot in the list: his place among the humans, from one.
fn talk_slot(index: usize) -> i16 {
    i16::try_from(index + 1).unwrap_or(0)
}

/// Whether the club's talk is booked with this manager, whenever it is
/// (`FUN_006a8ce2`).
pub fn talk_booked(records: &Records, index: usize, club: (u8, i16)) -> bool {
    talks(records).iter().any(|t| t.slot == talk_slot(index) && t.club == club)
}

/// Whether this manager has a talk booked in this week (`FUN_006a8804`):
/// from the week's first day to its last.
pub fn talk_in_week(records: &Records, index: usize) -> bool {
    let start = records.globals.week_start();
    talks(records)
        .iter()
        .any(|t| t.slot == talk_slot(index) && (start..start + 7).contains(&t.day))
}

/// Whether this club's board has an appointment in this week already
/// (`FUN_006a8c36`): any booked talk naming it with its day in the week.
pub fn board_busy(records: &Records, club: (u8, i16)) -> bool {
    let start = records.globals.week_start();
    talks(records)
        .iter()
        .any(|t| t.club == club && (start..start + 7).contains(&t.day))
}

/// The talk booked (`FUN_006a8d67`): only if the planner's day is free,
/// the manager has no talk booked for that day, and the list has room —
/// then the entry and the planner's kind 0x14 entry naming the club.
/// Whether it was booked.
pub fn book_talk(records: &mut Records, index: usize, day: i16, club: (u8, i16)) -> bool {
    let free = records.planners.get(index).is_some_and(|p| p.day(day as usize).kind == 0);
    let list = talks(records);
    if !free
        || list.len() >= TALKS_MOST
        || list.iter().any(|t| t.slot == talk_slot(index) && t.day == day)
    {
        return false;
    }
    let Some(b) = records.block_mut(&format!("{TALKS_AT:#x}")) else { return false };
    let e = TALKS_FIRST + list.len() * TALK;
    if b.len() < e + TALK {
        return false;
    }
    b[e..e + TALK].fill(0);
    put16(b, e, talk_slot(index));
    put16(b, e + 2, day);
    b[e + 4] = club.0;
    put16(b, e + 5, club.1);
    put16(b, TALKS_COUNT, i16::try_from(list.len() + 1).unwrap_or(i16::MAX));
    if let Some(planner) = records.planners.get_mut(index) {
        planner.book(
            day as usize,
            Appointment { kind: TALK_KIND, flag: ONE_DAY, player: u16::from(club.0), club: club.1 },
        );
    }
    true
}

/// A single day's appointment, as the planner books a talk.
const ONE_DAY: u8 = 1;
