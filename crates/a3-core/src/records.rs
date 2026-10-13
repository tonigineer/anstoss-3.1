//! The original's records, as a savegame holds them: players, teams,
//! managers, the globals of the date and the name pool, each a newtype over
//! its bytes. Fields are declared once, at their offsets in the object as the
//! decompile has them; bytes nobody has named yet stay as they are, so a
//! save reads and writes back unchanged.
//!
//! Every object of the original starts with a header of 0x20 bytes, which
//! the save leaves out: a field at +0x1bc of a player sits at 0x19c of its
//! record in the save.

use std::collections::HashMap;
use std::fmt;
use std::io;

use a3_files::an3::Save;
use a3_files::sav::cp1252;
use chrono::NaiveDate;

/// The header of every object, which the save leaves out.
const HEADER: usize = 0x20;

/// A value in a record: little-endian, as the original keeps it.
pub trait Field: Sized {
    const SIZE: usize;
    fn get(bytes: &[u8]) -> Self;
    fn put(self, bytes: &mut [u8]);
}

macro_rules! numbers {
    ($($ty:ty),*) => {$(
        impl Field for $ty {
            const SIZE: usize = size_of::<$ty>();
            fn get(bytes: &[u8]) -> Self {
                <$ty>::from_le_bytes(bytes[..Self::SIZE].try_into().expect("a field inside its record"))
            }
            fn put(self, bytes: &mut [u8]) {
                bytes[..Self::SIZE].copy_from_slice(&self.to_le_bytes());
            }
        }
    )*};
}
numbers!(u8, i8, u16, i16, u32, i32);

impl<T: Field + Copy + Default, const N: usize> Field for [T; N] {
    const SIZE: usize = T::SIZE * N;
    fn get(bytes: &[u8]) -> Self {
        let mut values = [T::default(); N];
        for (i, value) in values.iter_mut().enumerate() {
            *value = T::get(&bytes[i * T::SIZE..]);
        }
        values
    }
    fn put(self, bytes: &mut [u8]) {
        for (i, value) in self.into_iter().enumerate() {
            value.put(&mut bytes[i * T::SIZE..]);
        }
    }
}

/// Declares a record over its bytes in the save, `SIZE` of them, with a
/// getter and a setter for each field, at its offset from `BASE` — the
/// object's start, for most records 0x20 before the bytes the save holds.
macro_rules! record {
    (
        $(#[$meta:meta])*
        $record:ident, $size:expr, base $base:expr;
        $( $(#[$doc:meta])* $offset:literal => $get:ident, $set:ident: $ty:ty; )*
    ) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq)]
        pub struct $record(Box<[u8]>);

        impl $record {
            /// Its bytes in the save.
            pub const SIZE: usize = $size;
            /// The address its fields are counted from.
            const BASE: usize = $base;

            pub fn new(bytes: &[u8]) -> io::Result<Self> {
                if bytes.len() < Self::SIZE {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{} bytes for a {} of {}", bytes.len(), stringify!($record), Self::SIZE),
                    ));
                }
                Ok(Self(bytes[..Self::SIZE].into()))
            }

            pub fn bytes(&self) -> &[u8] {
                &self.0
            }

            /// The field of type `T` at `offset`, for fields not declared yet.
            pub fn get<T: Field>(&self, offset: usize) -> T {
                T::get(&self.0[offset - Self::BASE..])
            }

            pub fn put<T: Field>(&mut self, offset: usize, value: T) {
                value.put(&mut self.0[offset - Self::BASE..]);
            }

            $(
                $(#[$doc])*
                pub fn $get(&self) -> $ty {
                    self.get($offset)
                }

                pub fn $set(&mut self, value: $ty) {
                    self.put($offset, value);
                }
            )*
        }

        impl fmt::Debug for $record {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($record))
                    $(.field(stringify!($get), &self.$get()))*
                    .finish()
            }
        }
    };
}

/// Text in a record: Windows-1252 up to the first NUL, within `len` bytes.
fn text(bytes: &[u8], len: usize) -> String {
    let bytes = &bytes[..len.min(bytes.len())];
    cp1252(&bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())])
}

/// Players: ids 0 to 17005, 0 none; a thousand to a block of the save.
pub const PLAYERS: usize = 17006;
const PLAYERS_PER_BLOCK: usize = 1000;

record! {
    /// A player (0x354 bytes at `DAT_00c02550` + id × 0x354), filled from the
    /// data files by `FUN_0068c903`.
    Player, 0x354 - HEADER, base HEADER;
    /// The year the contract runs to.
    0x15c => contract, set_contract: u16;
    0x168 => birth_day, set_birth_day: u8;
    0x169 => birth_month, set_birth_month: u8;
    0x16a => birth_year, set_birth_year: u16;
    /// Bit 31: the nickname is shown.
    0x1b4 => shown, set_shown: u32;
    /// The player's index in the array.
    0x1bc => id, set_id: u16;
    /// Names, as indices into the name pool.
    0x1be => first_name, set_first_name: u16;
    0x1c0 => last_name, set_last_name: u16;
    0x1c2 => nickname, set_nickname: u16;
    /// Abilities he is good and bad at, a bit each.
    0x1c4 => good, set_good: u16;
    0x1c6 => bad, set_bad: u16;
    /// Bits of character and skill.
    0x1c8 => traits, set_traits: [u16; 2];
    /// Condition and freshness, in hundredths.
    0x1ce => condition, set_condition: i16;
    0x1d0 => freshness, set_freshness: [i16; 3];
    /// Set with an injury: the game's day it happened (`FUN_00766b92`).
    0x1d8 => injured_on, set_injured_on: u16;
    /// The injury lasts to this year and day of the year.
    0x1da => injured_to, set_injured_to: [u16; 2];
    /// Not zero: "angeschlagen", knocked about.
    0x1de => knocked, set_knocked: u8;
    /// Away from the club, with the national team, and for long (a year
    /// and a day of the year each).
    0x1e4 => away_to, set_away_to: [u16; 2];
    0x1e8 => national_to, set_national_to: [u16; 2];
    0x1ec => long_to, set_long_to: [u16; 2];
    /// Bits 13 to 15: he plays with a knock, at 0.9 of his strength
    /// (`FUN_006a144e`).
    0x218 => knocks, set_knocks: u16;
    /// 1 to 12.
    0x21c => strength, set_strength: u8;
    /// The main position, then two side positions, 0 none (1 the keeper).
    0x21d => positions, set_positions: [u8; 3];
    0x223 => shirt, set_shirt: u8;
    0x225 => nation, set_nation: u8;
    /// Matches suspended in the low six bits; bit 7: for how long is not
    /// known yet.
    0x229 => suspension, set_suspension: u8;
    /// Why he is away from the club, in the low four bits; bit 4 marks him
    /// on the line-up list (its status icon 6).
    0x26b => away_kind, set_away_kind: u8;
    /// Bit 7: the long absence's length is not known yet.
    0x26e => long_unknown, set_long_unknown: u8;
    /// 100 is neither happy nor unhappy.
    0x296 => morale, set_morale: u8;
    /// 1: the line-up list marks him with the dumbbell (what for is not
    /// traced yet).
    0x34c => dumbbell, set_dumbbell: u8;
    /// The form the line-up showed last, for its trend.
    0x34d => last_form, set_last_form: u8;
    0x353 => mood, set_mood: i8;
}

impl Player {
    pub fn birthday(&self) -> Option<NaiveDate> {
        let (day, month, year) = (self.birth_day(), self.birth_month(), self.birth_year());
        NaiveDate::from_ymd_opt(year.into(), month.into(), day.into())
    }
}

/// Teams: the clubs of every country, a country's from `first_team`.
pub const TEAMS: usize = 0x1ec;
const TEAM_AT: u32 = 0x1a72ff8;
/// The most players a team lists.
const SQUAD_MOST: usize = 50;

record! {
    /// A club's team (0x990 bytes at `DAT_01a72ff8` + index × 0x990), from
    /// which the game fills its club objects (`FUN_0063b75a`).
    Team, 0x990 - HEADER, base HEADER;
    /// What the team counts for in goal, defence, midfield and attack, in
    /// tenths, as the game last reckoned it (the club object's +0x94 …
    /// +0x9a): the line-up's comparison shows the opponent's.
    0xb8 => strengths, set_strengths: [i16; 4];
    0x112 => player_count, set_player_count: u16;
    /// Its number among its country's teams.
    0x129 => number, set_number: u8;
    /// The country: a nation code.
    0x12d => country, set_country: u8;
    /// Player ids of the eleven and the bench, 0 an empty place.
    0x148 => lineup, set_lineup: [u16; 11];
    0x15e => bench, set_bench: [u16; 7];
    0x1ee => cohesion, set_cohesion: u8;
    /// The formation, 0 the 5-3-2.
    0x1f0 => formation, set_formation: u8;
    /// Where the eleven stand, in pitch pixels: the formation's spots, or
    /// where the manager moved a box (the club object's +0xae6).
    0x194 => spots, set_spots: [[i16; 2]; 11];
    /// Season tickets: the target the preseason sells towards, and how many
    /// are sold (the club object's +0x22c and +0x498).
    0x4a4 => ticket_target, set_ticket_target: i32;
    0x710 => tickets, set_tickets: i32;
}

/// Where a team lists its players.
const SQUAD: usize = 0x1f2;
/// Where its name is, and how long it may be.
const TEAM_NAME: (usize, usize) = (0x20, 0x7c);

impl Team {
    pub fn name(&self) -> String {
        text(&self.0[TEAM_NAME.0 - HEADER..], TEAM_NAME.1)
    }

    /// The ids of its players.
    pub fn players(&self) -> Vec<u16> {
        let count = usize::from(self.player_count()).min(SQUAD_MOST);
        (0..count).map(|i| self.get(SQUAD + 2 * i)).collect()
    }
}

/// Human managers, in their slots.
pub const MANAGERS: usize = 4;
const MANAGER_AT: u32 = 0xa62578;

record! {
    /// A human manager (0x2158 bytes at `DAT_00a62598` + slot × 0x2158;
    /// the save holds the record from there).
    Manager, 0x2158 - HEADER, base 0;
    /// The club: its country and its number there.
    0x1e => country, set_country: u16;
    0x20 => team, set_team: u16;
    /// 3: under contract.
    0x22 => status, set_status: u8;
    0x23 => level, set_level: u8;
    0x2e => birth_day, set_birth_day: u8;
    0x2f => birth_month, set_birth_month: u8;
    0x30 => birth_year, set_birth_year: u16;
    /// The board's confidence in percent, and what it has on top
    /// (`FUN_007701b2` adds them up).
    0x184 => confidence, set_confidence: i16;
    0x186 => confidence_ups, set_confidence_ups: [u8; 2];
    0x188 => confidence_swings, set_confidence_swings: [i8; 2];
}

/// A manager's formations: a name of ten bytes and the eleven's spots, x
/// and y in pitch pixels, each (the manager object's +0x118).
const FORMATIONS_AT: usize = 0x2f0;
const FORMATION_SIZE: usize = 0x36;
const FORMATION_NAME: usize = 10;

impl Manager {
    /// His formations, as the formations dialog offers them.
    pub fn formations(&self) -> Vec<(String, [(i16, i16); 11])> {
        (0..crate::lineup::FORMATIONS_OF_A_MANAGER)
            .map(|f| {
                let at = FORMATIONS_AT + f * FORMATION_SIZE;
                let name = text(&self.0[at..], FORMATION_NAME);
                let spots: [[i16; 2]; 11] = self.get(at + FORMATION_NAME);
                (name, spots.map(|[x, y]| (x, y)))
            })
            .collect()
    }

    /// The board's confidence as the screens show it, from 0 to 100.
    pub fn shown_confidence(&self) -> i32 {
        let [a, b] = self.confidence_ups();
        let [c, d] = self.confidence_swings();
        let sum = i32::from(self.confidence())
            + i32::from(a)
            + i32::from(b)
            + i32::from(c)
            + i32::from(d);
        sum.clamp(0, 100)
    }
}

const MANAGER_FIRST_NAME: (usize, usize) = (0, 0xa);
const MANAGER_LAST_NAME: (usize, usize) = (0xa, 0x12);

impl Manager {
    pub fn first_name(&self) -> String {
        text(&self.0[MANAGER_FIRST_NAME.0..], MANAGER_FIRST_NAME.1)
    }

    pub fn last_name(&self) -> String {
        text(&self.0[MANAGER_LAST_NAME.0..], MANAGER_LAST_NAME.1)
    }
}

/// The block of globals around the date (`0xb5cb68`, its data from
/// `0xb5cb88`).
const GLOBALS_AT: u32 = 0xb5cb68;

record! {
    /// The globals of the game's date and its humans.
    Globals, 0x1590, base 0xb5cb88;
    /// Whether each human slot is in play: above 0.
    0xb5cb9c => slots, set_slots: [i16; MANAGERS];
    /// The year the game started in.
    0xb5cbca => start_year, set_start_year: u16;
    0xb5cbcc => year, set_year: u16;
    /// How many humans play.
    0xb5cbce => humans, set_humans: u16;
    /// Half weeks since the start.
    0xb5cbd2 => half_weeks, set_half_weeks: u16;
    /// Days since the start of the first season.
    0xb5cbd4 => day, set_day: u16;
    0xb5cbda => date, set_date: [u16; 3];
    /// The day of the year, from 0.
    0xb5cbe0 => day_of_year, set_day_of_year: u16;
    /// Days in this year.
    0xb5cca4 => year_days, set_year_days: u16;
    /// Where the game cycle goes on after loading (`FUN_006da3a9`): 998
    /// inside the week, the half weeks at a week's turn.
    0xb5d172 => resume, set_resume: u16;
}

/// The block of tables by country (`0xae5b18`, its data from `0xae5b38`).
const COUNTRIES_AT: u32 = 0xae5b18;
record! {
    /// Tables by country.
    Countries, 0x129c, base 0xae5b38;
}

impl Countries {
    /// How many teams a country has.
    pub fn team_count(&self, country: usize) -> i16 {
        self.get(0xae5e06 + 2 * country)
    }

    /// The index of a country's first team.
    pub fn first_team(&self, country: usize) -> i16 {
        self.get(0xae5ef0 + 2 * country)
    }
}

/// A savegame's records, and the save itself for everything not read into
/// them yet.
pub struct Records {
    /// By id.
    pub players: Vec<Player>,
    pub teams: Vec<Team>,
    /// By slot.
    pub managers: Vec<Manager>,
    pub globals: Globals,
    pub countries: Countries,
    /// The pool of names the players' names point into.
    pub names: Vec<String>,
    save: Save,
}

impl Records {
    pub fn from_save(save: Save) -> io::Result<Self> {
        let blocks: HashMap<&str, &[u8]> =
            save.blocks.iter().map(|block| (block.name.as_str(), block.data.as_slice())).collect();
        let block = |name: String| {
            blocks.get(name.as_str()).copied().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, format!("no block {name} in the save"))
            })
        };
        let mut players = Vec::with_capacity(PLAYERS);
        for id in 0..PLAYERS {
            let data = block(format!("players[{}]", id / PLAYERS_PER_BLOCK))?;
            players.push(Player::new(&data[(id % PLAYERS_PER_BLOCK) * Player::SIZE..])?);
        }
        let teams = (0..TEAMS)
            .map(|n| Team::new(block(format!("{TEAM_AT:#x}[{n}]"))?))
            .collect::<io::Result<_>>()?;
        let managers = (0..MANAGERS)
            .map(|n| Manager::new(block(format!("{MANAGER_AT:#x}[{n}]"))?))
            .collect::<io::Result<_>>()?;
        let globals = Globals::new(block(format!("{GLOBALS_AT:#x}"))?)?;
        let countries = Countries::new(block(format!("{COUNTRIES_AT:#x}"))?)?;
        let names = block("names".to_owned())?.split(|&b| b == 0).map(cp1252).collect();
        Ok(Self { players, teams, managers, globals, countries, names, save })
    }

    /// The save with the records written back into their blocks.
    pub fn to_save(&self) -> Save {
        let mut save = self.save.clone();
        let mut records: HashMap<String, Vec<(usize, &[u8])>> = HashMap::new();
        let mut put =
            |name: String, at: usize, bytes| records.entry(name).or_default().push((at, bytes));
        for (id, player) in self.players.iter().enumerate() {
            let at = (id % PLAYERS_PER_BLOCK) * Player::SIZE;
            put(format!("players[{}]", id / PLAYERS_PER_BLOCK), at, player.bytes());
        }
        for (n, team) in self.teams.iter().enumerate() {
            put(format!("{TEAM_AT:#x}[{n}]"), 0, team.bytes());
        }
        for (n, manager) in self.managers.iter().enumerate() {
            put(format!("{MANAGER_AT:#x}[{n}]"), 0, manager.bytes());
        }
        put(format!("{GLOBALS_AT:#x}"), 0, self.globals.bytes());
        put(format!("{COUNTRIES_AT:#x}"), 0, self.countries.bytes());
        for block in &mut save.blocks {
            for (at, bytes) in records.get(&block.name).into_iter().flatten() {
                block.data[*at..*at + bytes.len()].copy_from_slice(bytes);
            }
        }
        save
    }

    /// A name from the pool.
    pub fn name(&self, index: u16) -> &str {
        self.names.get(usize::from(index)).map_or("", String::as_str)
    }

    /// The team of a country's club, by its number there.
    pub fn team(&self, country: u16, number: u16) -> Option<&Team> {
        let first = usize::try_from(self.countries.first_team(country.into())).ok()?;
        self.teams.get(first + usize::from(number))
    }

    pub fn player(&self, id: u16) -> Option<&Player> {
        (id != 0).then(|| self.players.get(usize::from(id))).flatten()
    }

    /// The managers in play, in their slots.
    pub fn humans(&self) -> impl Iterator<Item = &Manager> {
        let slots = self.globals.slots();
        self.managers.iter().zip(slots).filter(|(_, slot)| *slot > 0).map(|(manager, _)| manager)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn fields_read_and_write_little_endian() {
        let mut bytes = vec![0; Team::SIZE];
        bytes[0x148 - HEADER..0x14c - HEADER].copy_from_slice(&[0xb9, 0x0c, 0xbc, 0x0c]);
        let mut team = Team::new(&bytes).unwrap();
        assert_eq!(team.lineup()[..2], [3257, 3260]);
        team.set_formation(3);
        assert_eq!(team.bytes()[0x1f0 - HEADER], 3);
        assert!(Team::new(&bytes[1..]).is_err());
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_saves_read_into_records_and_back() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut read = 0;
        for entry in std::fs::read_dir(dir.join("saves")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|extension| !extension.eq_ignore_ascii_case("an3")) {
                continue;
            }
            let save = Save::read(&path).unwrap();
            let records = Records::from_save(save.clone()).unwrap();
            for (id, player) in records.players.iter().enumerate() {
                assert_eq!(usize::from(player.id()), id, "{}", path.display());
            }
            let humans: Vec<_> = records.humans().collect();
            assert_eq!(humans.len(), usize::from(records.globals.humans()), "{}", path.display());
            for manager in humans.iter().filter(|manager| manager.status() == 3) {
                let team = records.team(manager.country(), manager.team()).unwrap();
                assert!(!team.name().is_empty());
                let squad = team.players();
                assert!(!squad.is_empty());
                for id in team.lineup().into_iter().chain(team.bench()).filter(|&id| id != 0) {
                    assert!(
                        squad.contains(&id),
                        "{}: {id} plays, but is not in the squad",
                        path.display()
                    );
                }
                for id in squad {
                    assert!(!records.name(records.player(id).unwrap().last_name()).is_empty());
                }
            }
            assert!(a3_files::an3::compare(&save, &records.to_save()).blocks.is_empty());
            read += 1;
        }
        assert!(read > 0, "no saves in {}", dir.display());
    }
}
