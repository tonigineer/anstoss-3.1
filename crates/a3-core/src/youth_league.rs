//! The youth leagues (`0xb77522` + age group × 0xd88 + league × 0x2898 +
//! slot × 0x7efc, `FUN_0066fa2c`): per country slot, league and age group
//! the teams, the schedule and the results, as the senior leagues keep
//! them; and per club and age group how far its youth stands
//! (`0xb7eeea`, `FUN_0066fae0`). Their tables are reckoned from the
//! results as the game reckons them (`FUN_00729f8b`) and sorted as it
//! sorts them (`FUN_0072b9b9` with `FUN_0072bc49`).

use crate::records::Records;

/// The objects of the youth leagues, one per country slot; their saved
/// bytes start 0x20 into the object.
const OBJECT: u32 = 0xb77500;
const SAVED_FROM: usize = 0x20;
const LEAGUES_AT: usize = 0x22 - SAVED_FROM;
const AGE_GROUP: usize = 0xd88;
const LEAGUE: usize = 0x2898;
const STAGES_AT: usize = 0xb7eeea - 0xb77500 - SAVED_FROM;
const STAGES: usize = 0x510;
const CLUB_STAGES: usize = 0xc;

const TEAMS: usize = 2;
const PLAYED: usize = 4;
const FOUR_ROUNDS: usize = 6;
const NUMBERS: usize = 8;
const SCHEDULE: usize = 0x38;
const ROW: usize = 0x18;
const RESULTS: usize = 0x488;
const RESULT_ROW: usize = 0x60;
const UNPLAYED: u8 = 0xff;

/// A youth league of an age group (0 A, 1 B, 2 C).
pub struct YouthLeague<'a> {
    data: &'a [u8],
}

/// A match's goals, home first, and the half-time's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Score {
    pub home: u8,
    pub away: u8,
    pub half_home: u8,
    pub half_away: u8,
}

impl<'a> YouthLeague<'a> {
    pub fn of(records: &'a Records, country: u16, league: usize, age: usize) -> Option<Self> {
        let slot = records.countries.slot(country.into());
        let block = records.block(&format!("{OBJECT:#x}[{slot}]"))?;
        let at = LEAGUES_AT + age * AGE_GROUP + league * LEAGUE;
        Some(Self { data: block.get(at..at + AGE_GROUP)? })
    }

    fn i16_at(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.data[at], self.data[at + 1]])
    }

    pub fn teams(&self) -> usize {
        usize::try_from(self.i16_at(TEAMS)).unwrap_or(0).min(24)
    }

    /// The matchdays played.
    pub fn played(&self) -> usize {
        usize::try_from(self.i16_at(PLAYED)).unwrap_or(0)
    }

    fn four_rounds(&self) -> bool {
        self.i16_at(FOUR_ROUNDS) != 0
    }

    /// The team number at a slot.
    pub fn number(&self, slot: usize) -> i16 {
        self.i16_at(NUMBERS + 2 * slot)
    }

    /// The pairs of a matchday's row of the schedule, by slots.
    pub fn pairs(&self, row: usize) -> Vec<(usize, usize)> {
        let at = SCHEDULE + row * ROW;
        (0..self.teams() / 2)
            .map(|k| (usize::from(self.data[at + 2 * k]), usize::from(self.data[at + 2 * k + 1])))
            .collect()
    }

    /// The result of a slot's match on a matchday from 1, if played
    /// (`FUN_00670978`): a league of four rounds plays its second two on
    /// the first two's rows, its results beside theirs.
    pub fn result(&self, matchday: usize, slot: usize) -> Option<Score> {
        let n = self.teams();
        let (mut row, mut offset) = (matchday, 0);
        if self.four_rounds() && matchday + 2 > 2 * n {
            row -= 2 * n - 2;
            offset = n;
        }
        let (home, away) =
            self.pairs(row.checked_sub(1)?).into_iter().find(|&(h, a)| h == slot || a == slot)?;
        let at = RESULTS + home * RESULT_ROW + (away + offset) * 4;
        let b = self.data.get(at..at + 4)?;
        (b[0] != UNPLAYED).then_some(Score {
            home: b[0],
            away: b[1],
            half_home: b[2],
            half_away: b[3],
        })
    }

    /// A table of the matchdays played (`FUN_00729f8b`): all matches, or
    /// only the home or the away ones, sorted.
    pub fn table(&self, games: Games) -> Vec<Row> {
        let mut rows: Vec<Row> = (0..self.teams())
            .map(|slot| Row { number: self.number(slot), ..Row::default() })
            .collect();
        // The pairs from the matchday's own row; only the result's lookup
        // folds the later rounds onto the first.
        for matchday in 1..=self.played() {
            for (home, away) in self.pairs(matchday - 1) {
                let Some(score) = self.result(matchday, home) else { continue };
                if games != Games::Away {
                    rows[home].count(score.home, score.away);
                }
                if games != Games::Home {
                    rows[away].count(score.away, score.home);
                }
            }
        }
        sort(&mut rows);
        rows
    }
}

/// Which matches a table counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Games {
    All,
    Home,
    Away,
}

/// A team's line of a table (0x14 bytes in the game).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub number: i16,
    pub games: i16,
    pub points: i16,
    pub goals_for: i16,
    pub goals_against: i16,
    pub won: i16,
    pub drawn: i16,
    pub lost: i16,
}

impl Row {
    fn count(&mut self, own: u8, theirs: u8) {
        self.games += 1;
        match own.cmp(&theirs) {
            std::cmp::Ordering::Greater => {
                self.points += 3;
                self.won += 1;
            }
            std::cmp::Ordering::Equal => {
                self.points += 1;
                self.drawn += 1;
            }
            std::cmp::Ordering::Less => self.lost += 1,
        }
        self.goals_for += i16::from(own);
        self.goals_against += i16::from(theirs);
    }

    pub fn difference(&self) -> i16 {
        self.goals_for - self.goals_against
    }

    /// Whether it stands below `other` (`FUN_0072bc49`): fewer points, a
    /// worse difference, fewer goals, fewer games.
    fn below(&self, other: &Row) -> bool {
        (self.points, self.difference(), self.goals_for, self.games)
            < (other.points, other.difference(), other.goals_for, other.games)
    }
}

/// The game's bubble sort: neighbours change places while one stands
/// below the next (`FUN_0072b9b9`).
fn sort(rows: &mut [Row]) {
    let mut swapped = true;
    while swapped {
        swapped = false;
        for i in 1..rows.len() {
            if rows[i - 1].below(&rows[i]) {
                rows.swap(i - 1, i);
                swapped = true;
            }
        }
    }
}

/// How far a club's youth of an age group has come (`FUN_0066fae0`): the
/// stage it is in (3 the final round), its place there, whether it has
/// played in it.
pub struct Stage {
    pub stage: u8,
    pub place: u8,
    pub played: bool,
}

pub fn stage(records: &Records, country: u16, club: usize, age: usize) -> Option<Stage> {
    let slot = records.countries.slot(country.into());
    let block = records.block(&format!("{OBJECT:#x}[{slot}]"))?;
    let stages = block.get(STAGES_AT..STAGES_AT + STAGES)?;
    let at = club * CLUB_STAGES + age;
    Some(Stage {
        stage: *stages.get(at)?,
        place: *stages.get(at + 3)?,
        played: *stages.get(at + 9)? != 0,
    })
}

/// The stage byte of the youth's final round.
pub const FINAL_ROUND: u8 = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_sorts_by_points_difference_goals_and_games() {
        let row = |number, points, goals_for, goals_against, games| Row {
            number,
            points,
            goals_for,
            goals_against,
            games,
            ..Row::default()
        };
        let mut rows =
            vec![row(1, 3, 2, 2, 2), row(2, 4, 1, 1, 2), row(3, 3, 3, 1, 2), row(4, 3, 3, 1, 3)];
        sort(&mut rows);
        assert_eq!(rows.iter().map(|r| r.number).collect::<Vec<_>>(), [2, 4, 3, 1]);
    }
}
