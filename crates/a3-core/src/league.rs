//! What a club's place in its league is worth (VEREINSFUEHRUNG.md §4.4):
//! the class of season goal the place reaches, from the program's table
//! by the league's size (`FUN_007760ce`, `FUN_0077600e`, `FUN_00424050`,
//! `FUN_00635a35`). The board asks this when it weighs letting its
//! manager go.

use std::io;

use a3_files::exe::Exe;

use crate::records::{Records, Team};

/// The table (`0x8ac91f`): sixteen rows for the leagues' sizes, a byte
/// for each of twenty-four places — the class the place reaches.
const CLASS_AT: u32 = 0x8ac91f;
const CLASS_ROWS: usize = 16;
const CLASS_PLACES: usize = 24;
/// The leagues' objects (`0x1a09d48[li]`, their data from +0x20): the
/// teams playing stand at the object's +0x50.
const LEAGUE_AT: u32 = 0x1a09d48;
const LEAGUE_TEAMS: usize = 0x30;
/// What the game takes for a league it has none of.
const TEAMS: i32 = 18;
/// The top six leagues have a size of their own.
const OWN_SIZE: u8 = 6;
/// The countries whose league is worth more, in the globals' ranking
/// (`0xb5d1a8`; the Globals record starts at `0xb5cb88`): fifteen rows of
/// 0x24, the country first. `FUN_00424050` reads it and answers 4 for the
/// top three, 3 for the nine after, 2 for the rest.
const RANK_AT: usize = 0xb5d1a8 - 0xb5cb88;
const RANK_ROWS: usize = 15;
/// The team record: its league's level, its place, the class the table
/// gives a place of none, and the season's own goal.
const LEVEL_AT: usize = 0x12e;
const PLACE_AT: usize = 0x132;
/// The class stored for a club standing outside the table (team +0x135).
pub const NO_PLACE_AT: usize = 0x135;
const GOAL_AT: usize = 0x136;

pub struct LeagueTables {
    class: Vec<u8>,
}

impl LeagueTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        Ok(Self { class: exe.bytes(CLASS_AT, CLASS_ROWS * CLASS_PLACES)?.to_vec() })
    }
}

/// How many teams play in this league (`FUN_00635a35`); the six with a
/// size of their own and any league not played take eighteen.
fn teams(records: &Records, country: u8, level: u8) -> i32 {
    if level >= OWN_SIZE {
        return TEAMS;
    }
    let first = i32::from(records.countries.first_league(usize::from(country)));
    let index = first + i32::from(level);
    records
        .block(&format!("{LEAGUE_AT:#x}[{index}]"))
        .and_then(|b| b.get(LEAGUE_TEAMS..LEAGUE_TEAMS + 2))
        .map_or(TEAMS, |two| i32::from(i16::from_le_bytes([two[0], two[1]])))
}

/// How much the country's league counts (`FUN_00424050`).
fn ranking(records: &Records, country: u8) -> i32 {
    let key = i16::from(country);
    let row = (0..RANK_ROWS)
        .find(|&i| records.globals.get::<i16>(RANK_AT + i * 0x24) == key)
        .unwrap_or(RANK_ROWS);
    if row < 3 {
        4
    } else if row < 9 {
        3
    } else {
        2
    }
}

/// The class of goal the club's place reaches (`FUN_007760ce`): a place
/// of none answers the class stored for it, in a top league the first
/// place none and the next three one, places up to the country's ranking
/// three, and else the table by the league's size; below the top league
/// the first three places two.
pub fn class(records: &Records, tables: &LeagueTables, team: &Team) -> i32 {
    let place = i32::from(team.get::<u8>(PLACE_AT));
    if place == 0 {
        return i32::from(team.get::<i8>(NO_PLACE_AT));
    }
    let (country, level) = (team.country(), team.get::<u8>(LEVEL_AT));
    if level == 0 {
        if place == 1 {
            return 0;
        }
        if place < 4 {
            return 1;
        }
        if ranking(records, country) + 2 >= place {
            return 3;
        }
    } else if place < 4 {
        return 2;
    }
    let row = teams(records, country, level) / 2 - 5;
    let at = usize::try_from(row * CLASS_PLACES as i32 + place).unwrap_or(usize::MAX);
    tables.class.get(at).copied().map_or(0, i32::from)
}

/// The season's own goal (team +0x136): the class the board set itself.
pub fn goal(team: &Team) -> i32 {
    i32::from(team.get::<i8>(GOAL_AT))
}
