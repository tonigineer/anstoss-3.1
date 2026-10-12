//! The game's year: 364 days from the start of July, and for every league
//! which of them are its matchdays.
//!
//! A season starts on the first day of July the game takes for a Sunday — its
//! weekday sum is off by one, so in 1998 that is Monday, 6 July — and the game
//! names every seventh day from there a Sunday.
//!
//! The matchdays are a table in the executable: one row of 364 days per
//! league, the rows of a country's leagues in a run that starts at the
//! country's own entry in a second table. Row 0 holds the international and
//! European dates. A day's value from 1 to 59 is the league matchday played
//! on it; other values name other competitions in their top bits.

use std::io;

use chrono::{Days, NaiveDate};

use a3_files::exe::Exe;

use crate::world::World;

pub const DAYS: usize = 364;
/// Addresses of the tables in `anstoss3.exe`: the days of every row, the
/// first row of every nation's leagues, by nation code.
const ROWS_AT: u32 = 0x8914e0;
const ROWS: usize = 34;
const FIRST_ROWS_AT: u32 = 0x8315c8;
const NATIONS: usize = 117;
const LAST_MATCHDAY: u32 = 0x3b;
/// A nation's leagues are counted up to here.
const MAX_LEAGUES: usize = 10;
/// The bonus country plays in the row after its number of clubs and ten, or
/// one further with a flag of its `BONUSLAND`. A second flag says whether it
/// has a league cup, whose final days the game then writes into that row.
const BONUS_ROW_AFTER: i64 = 10;
const BONUS_LATER_ROW: i64 = 1 << 16;
const BONUS_LEAGUE_CUP: i64 = 2;
const BONUS_CUP_DAYS: [(usize, [u32; 2]); 2] =
    [(45, [0x3008_0004, 0x3000_0004]), (55, [0, 0xb008_0004])];
/// The weekday arithmetic counts days from this year.
const EPOCH: i32 = 1995;
const MONTH_DAYS: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const JULY: u32 = 7;

pub struct Calendar {
    rows: Vec<Vec<u32>>,
    /// By nation code: the row of its first league.
    first_rows: Vec<u16>,
}

impl Calendar {
    pub fn load(exe: &Exe, world: &World) -> io::Result<Self> {
        let days = exe.u32s(ROWS_AT, ROWS * DAYS)?;
        let mut rows: Vec<Vec<u32>> = days.chunks(DAYS).map(<[u32]>::to_vec).collect();
        let mut first_rows = exe.u16s(FIRST_ROWS_AT, NATIONS)?;
        if let Some((nation, (clubs, flags))) = world
            .countries
            .iter()
            .flatten()
            .find_map(|country| Some((country.nation, country.bonus?)))
        {
            let row = clubs + BONUS_ROW_AFTER + i64::from(flags & BONUS_LATER_ROW != 0);
            if let (Some(first), Some(days)) =
                (first_rows.get_mut(usize::from(nation)), rows.get_mut(row as usize))
            {
                *first = row as u16;
                let cup = usize::from(flags & BONUS_LEAGUE_CUP != 0);
                for (day, values) in BONUS_CUP_DAYS {
                    days[day] = values[cup];
                }
            }
        }
        Ok(Self { rows, first_rows })
    }

    /// The row of a nation's league, counted from its first.
    pub fn row(&self, nation: u16, league: usize) -> Option<usize> {
        let first = *self.first_rows.get(usize::from(nation))?;
        (league < MAX_LEAGUES).then(|| usize::from(first) + league).filter(|&r| r < self.rows.len())
    }

    /// The league matchday a row plays on `day`, counted from 1.
    pub fn matchday(&self, row: usize, day: usize) -> Option<usize> {
        let value = *self.rows.get(row)?.get(day)?;
        (1..=LAST_MATCHDAY).contains(&value).then_some(value as usize)
    }

    /// The first day from `day` on with a league match in `row`.
    pub fn next_matchday(&self, row: usize, day: usize) -> Option<(usize, usize)> {
        (day..DAYS).find_map(|d| Some((d, self.matchday(row, d)?)))
    }
}

/// The day a season starts in July of `year`.
pub fn season_start(year: i32) -> NaiveDate {
    let offset = (7 - weekday(1, JULY, year)) % 7;
    NaiveDate::from_ymd_opt(year, JULY, 1 + offset as u32).expect("a day early in July")
}

/// The date of a day of the season.
pub fn date(year: i32, day: usize) -> NaiveDate {
    season_start(year) + Days::new(day as u64)
}

/// The game's weekday of a date, 0 for what it takes for Sunday: it counts
/// the days since 1995 with the leap years of the Gregorian calendar, but
/// starts one day late.
fn weekday(day: i32, month: u32, year: i32) -> i32 {
    let years = year - EPOCH;
    let mut days = years * 365 + years / 4 - (year - 1901) / 100 + (year - 1601) / 400;
    days += MONTH_DAYS[..month as usize - 1].iter().sum::<i32>();
    let leap = year % 4 == 0 && (year % 400 == 0 || year % 100 != 0);
    if leap && month > 2 {
        days += 1;
    }
    (days - 1 + day) % 7
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_season_starts_on_the_sixth_of_july_1998() {
        assert_eq!(season_start(1998), NaiveDate::from_ymd_opt(1998, 7, 6).unwrap());
        assert_eq!(date(1998, 48), NaiveDate::from_ymd_opt(1998, 8, 23).unwrap());
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_bundesliga_starts_on_day_48() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let world = World::load(&dir.join("Data.a3")).unwrap();
        let exe = Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let calendar = Calendar::load(&exe, &world).unwrap();
        let bundesliga = calendar.row(14, 0).unwrap();
        assert_eq!(calendar.next_matchday(bundesliga, 0), Some((48, 1)));
        assert_eq!(calendar.matchday(bundesliga, 321), Some(34));
    }
}
