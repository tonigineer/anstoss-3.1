//! A player's market value (`FUN_00708836`): what he is worth a month
//! (`FUN_007083fd`) — by strength and position, talent and age, abilities
//! and character — summed over the months his contract still runs, later
//! years counting less, and a fifth taken off.

use std::io;

use a3_files::exe::Exe;

use crate::world::GOALKEEPER;

const STRENGTH_AT: u32 = 0x8a7748;
const POSITION_AT: u32 = 0x8a7608;
/// What each ability and each bit of character adds in percent: a field
/// player's abilities, a keeper's, the two words of character.
const ABILITIES_AT: u32 = 0x8a76b0;
const KEEPER_AT: u32 = 0x8a76c0;
const CHARACTER_AT: [u32; 2] = [0x8a76d0, 0x8a76f0];
/// How many months of a year each contract year counts, the first twelve.
const YEARS_AT: u32 = 0x8a77b8;
/// The months of a contract's first year still to come, by this month.
const MONTHS_AT: u32 = 0x89e220;
const STRENGTHS: usize = 14;
const POSITIONS: usize = 11;
const BITS: usize = 16;
const KEEPER_BITS: usize = 9;
const YEARS: usize = 6;
const MONTHS: usize = 13;
/// A worth of a million a year and more is rounded down to five thousand a
/// month, less to a thousand; no one is worth less than that.
const ROUND_BIG: i64 = 5000;
const ROUND: i64 = 1000;
const BIG: i64 = 10_000;
const MOST: i64 = 250_000_000;

pub struct ValueTables {
    strength: Vec<f64>,
    position: Vec<f64>,
    abilities: Vec<i8>,
    keeper: Vec<i8>,
    character: [Vec<i8>; 2],
    years: Vec<u16>,
    months: Vec<u8>,
}

impl ValueTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let doubles = |at: u32, n: usize| -> io::Result<Vec<f64>> {
            Ok(exe
                .bytes(at, 8 * n)?
                .chunks(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                .collect())
        };
        let signed = |at: u32, n: usize| -> io::Result<Vec<i8>> {
            Ok(exe.bytes(at, n)?.iter().map(|&b| b as i8).collect())
        };
        Ok(Self {
            strength: doubles(STRENGTH_AT, STRENGTHS)?,
            position: doubles(POSITION_AT, POSITIONS)?,
            abilities: signed(ABILITIES_AT, BITS)?,
            keeper: signed(KEEPER_AT, KEEPER_BITS)?,
            character: [signed(CHARACTER_AT[0], BITS)?, signed(CHARACTER_AT[1], BITS)?],
            years: exe.u16s(YEARS_AT, YEARS)?,
            months: exe.bytes(MONTHS_AT, MONTHS)?.to_vec(),
        })
    }
}

/// What the market value goes by.
pub struct Worth {
    pub strength: u8,
    pub position: u8,
    /// 0 huge to 4 none.
    pub talent: i8,
    pub age: i32,
    /// The abilities he is good at, his character's two words, and the bits
    /// that add five percent each.
    pub good: u16,
    pub character: [u16; 2],
    pub qualities: u16,
    /// The game's player of his position, worth a fifth more.
    pub star: bool,
    /// Matches of the competitions of the kinds 12 to 15 this season.
    pub kind_12_matches: u8,
}

/// A loan's fair prices: the fee without an option, the fee with one, the
/// option's price.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LoanPrices {
    pub fee: i32,
    pub with_option: i32,
    pub option: i32,
}

/// The loan prices of a monthly worth with `months` of the season left.
fn loan_prices(monthly: i64, months: u8, years: i32) -> LoanPrices {
    let f = f64::from(monthly as f32);
    let fee = round_price((f64::from(months) * f / 3.5) as i64 as i32);
    let with_option = round_price(((fee as u32).wrapping_mul(3) >> 1) as i32);
    let option = round_price((f64::from(years - 1) * f * 12.0) as i64 as i32);
    LoanPrices { fee, with_option, option }
}

/// A price rounded down as the loans round it (`FUN_00622f0d`): to
/// thousands below 100,000, to ten thousands below a million, else to
/// 50,000 — compared unsigned, divided toward zero.
fn round_price(x: i32) -> i32 {
    let unit = match x as u32 {
        0..100_000 => 1000,
        100_000..1_000_000 => 10_000,
        _ => 50_000,
    };
    x / unit * unit
}

impl ValueTables {
    /// What he is worth a month (`FUN_007083fd`).
    pub fn monthly(&self, w: &Worth) -> i64 {
        let s = usize::from(w.strength).min(STRENGTHS - 1);
        let p = usize::from(w.position).min(POSITIONS - 1);
        let mut x = 100_000.0 * self.strength[s] * self.position[p];
        let (a, strength) = (w.age, i32::from(w.strength));
        // Talent counts while he is young, the more the more he has.
        let talent = match w.talent {
            0 if strength < 12 => match a {
                ..21 => 1.8,
                21..26 => 1.55,
                26..30 => 1.3,
                _ => 1.0,
            },
            1 if strength < 11 => match a {
                ..21 => 1.4,
                21..26 => 1.2,
                _ => 1.0,
            },
            2 if strength < 10 && a < 26 => 1.05,
            3 if strength < 7 && a < 26 => 0.8,
            4 if strength < 6 && a < 26 => 0.6,
            _ => 1.0,
        };
        x *= talent;
        x *= match a {
            29 => 0.96,
            30 => 0.9,
            31 => 0.78,
            32 => 0.66,
            33 => 0.54,
            34 => 0.3,
            35 => 0.22,
            36.. => 0.17,
            _ => 1.0,
        };
        if a < 25 {
            x += ((25 - a) as f64 * 0.07 * x).trunc();
        }
        let bits = |word: u16, table: &[i8], count: usize| -> i32 {
            (0..count).filter(|&b| word & (1 << b) != 0).map(|b| i32::from(table[b])).sum()
        };
        let mut percent = if w.position == GOALKEEPER {
            bits(w.good, &self.keeper, KEEPER_BITS)
        } else {
            bits(w.good, &self.abilities, BITS)
        };
        percent += bits(w.character[0], &self.character[0], BITS);
        percent += bits(w.character[1], &self.character[1], BITS);
        percent += 5 * w.qualities.count_ones() as i32;
        if w.star {
            percent += 20;
        }
        x = x * f64::from(100 + percent) / 100.0;
        if w.strength >= 5 && w.kind_12_matches > 3 {
            x *= 1.1;
        }
        let monthly = (x / 12.0) as i64;
        let round = if monthly >= BIG { ROUND_BIG } else { ROUND };
        (monthly / round * round).max(ROUND)
    }

    /// The fair prices of a loan (`FUN_00622f7d`), by the months left in
    /// the season and the contract's years left: the fee without an option,
    /// two months' worth for every seven months left; with an option, half
    /// as much again; the option, a year's worth for every year after this.
    pub fn loan_prices(&self, w: &Worth, years: i32, month: u32) -> LoanPrices {
        let months = self.months.get(month as usize).copied().unwrap_or(0);
        loan_prices(self.monthly(w), months, years)
    }

    /// His market value with `years` of his contract left in the month
    /// `month` (1 to 12): every month still to run, the later years
    /// counting less, a fifth off (`FUN_00708836`).
    pub fn market_value(&self, w: &Worth, years: i32, month: u32) -> i64 {
        if years < 1 {
            return 0;
        }
        let first = i64::from(self.months.get(month as usize).copied().unwrap_or(0));
        let months = i64::from(years - 1) * 12 + first;
        let m = self.monthly(w);
        let full = (months / 12).min(YEARS as i64 - 1) as usize;
        let weight = |year: usize| i64::from(self.years[year.min(YEARS - 1)]);
        let mut total: i64 = (0..full).map(|y| weight(y) * m).sum();
        total += months % 12 * m * weight(full) / 12;
        (total / 5 * 4).clamp(0, MOST)
    }
}

#[cfg(test)]
mod loan_tests {
    use super::*;

    #[test]
    fn a_loan_costs_what_the_original_asked_fairly() {
        // LEIHE-TEILNAHME.md §1.2 on A.an3 in September (10 months left).
        assert_eq!(loan_prices(15_000, 10, 2).fee, 42_000);
        let tramontana = loan_prices(9000, 10, 4);
        assert_eq!(
            (tramontana.fee, tramontana.with_option, tramontana.option),
            (25_000, 37_000, 320_000)
        );
        // A negative option compares as a large one: to 50,000 toward zero.
        assert_eq!(round_price(-120_000), -100_000);
        assert_eq!(round_price(1_234_567), 1_200_000);
    }
}
