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

impl<T: Field, const N: usize> Field for [T; N] {
    const SIZE: usize = T::SIZE * N;
    fn get(bytes: &[u8]) -> Self {
        std::array::from_fn(|i| T::get(&bytes[i * T::SIZE..]))
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
    /// The club he plays for next season, if he signed with another: its
    /// league and number (0 none).
    0x14c => next_club, set_next_club: [u8; 2];
    /// His contract: salary in thousands a year, the bonuses for playing
    /// and for a goal in hundreds, a fixed transfer fee in ten thousands.
    0x150 => salary, set_salary: u16;
    0x152 => appearance_bonus, set_appearance_bonus: u16;
    0x154 => goal_bonus, set_goal_bonus: u16;
    0x156 => fixed_fee, set_fixed_fee: u16;
    /// The year the contract runs to, in the low twelve bits.
    0x15c => contract, set_contract: u16;
    /// His goals and his appearances in all his career, by league: the
    /// country's four, then the more countries' (`FUN_00704b05`).
    0x190 => career_goals, set_career_goals: [u16; 8];
    0x1a0 => career_appearances, set_career_appearances: [u16; 8];
    /// The contract's terms: the leagues it holds for (low two bits), on
    /// loan (0x100), not to play in the European cups (0x10), extended
    /// itself (0x80), options for club (0x2000) and player (0x1000), a
    /// regular place (0x4000).
    0x15e => terms, set_terms: u16;
    /// The game's day his red card was given, its suspension's date.
    0x1f2 => sent_off_on, set_sent_off_on: i16;
    /// How many children he has.
    0x1f6 => children, set_children: i16;
    /// Team-mates, by id (0 none): his friends and those he does not like,
    /// his best friend and the one he hates.
    0x1f8 => friends, set_friends: [u16; 3];
    0x1fe => dislikes, set_dislikes: [u16; 3];
    0x204 => best_friend, set_best_friend: u16;
    0x206 => hates, set_hates: u16;
    /// On the pitch: those he does not get on with, and those he
    /// understands blindly.
    0x208 => clashes, set_clashes: [u16; 2];
    0x20c => understands, set_understands: [u16; 2];
    /// How many stations his career has (at most 15); the last one's date
    /// tells how long he has been at the club.
    0x212 => career_count, set_career_count: i16;
    /// Goals for his national team.
    0x214 => international_goals, set_international_goals: u16;
    0x168 => birth_day, set_birth_day: u8;
    0x169 => birth_month, set_birth_month: u8;
    0x16a => birth_year, set_birth_year: u16;
    /// Bits: the fans love him (0x10000) or hate him (0x400).
    0x1b0 => flags, set_flags: u32;
    /// Bits: the nickname is shown (31), ostracised (17), an extra trainer
    /// (13), he smokes (11 — his condition counts five less), has a coach's
    /// licence (7), overweight (5).
    0x1b4 => shown, set_shown: u32;
    /// What he has earned so far.
    0x1b8 => earned, set_earned: i32;
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
    /// More bits the market value counts five each for.
    0x1cc => qualities, set_qualities: u16;
    /// Condition and freshness, in hundredths.
    0x1ce => condition, set_condition: i16;
    0x1d0 => freshness, set_freshness: [i16; 3];
    /// Set with an injury: the game's day it happened (`FUN_00766b92`).
    0x1d8 => injured_on, set_injured_on: u16;
    /// The injury lasts to this year and day of the year.
    0x1da => injured_to, set_injured_to: [u16; 2];
    /// Not zero: "angeschlagen", knocked about.
    0x1de => knocked, set_knocked: u8;
    /// Matches of the competitions of the kinds 12 to 15 this season: his
    /// internationals.
    0x266 => kind_12_matches, set_kind_12_matches: u8;
    /// Bit 7: he ends his career; bit 5: his strength is not known yet
    /// (an amateur's, `CAmateure::FUN_0070699b`).
    0x26f => retiring, set_retiring: u8;
    /// His strong foot (bits 2 and 3: 1 right, 2 left, 3 both), how the
    /// fans see him (low bits: 2 loved, 3 hated), counts as home-grown
    /// (0x10), will not be captain (0x40, from the data files).
    0x282 => manner, set_manner: u8;
    /// Matches played this season: in the league, and in the competitions
    /// of the kinds 3–4, 5–10 and 12–15 (the line-up's object +0x54, +0x59,
    /// +0x5f, +0x61).
    0x253 => league_matches, set_league_matches: u8;
    /// Two counts the cohesion takes from the league matches: the first,
    /// those he came on in as a substitute.
    0x254 => league_less, set_league_less: [u8; 2];
    /// Away from the club, with the national team, and for long (a year
    /// and a day of the year each).
    0x1e4 => away_to, set_away_to: [u16; 2];
    0x1e8 => national_to, set_national_to: [u16; 2];
    0x1ec => long_to, set_long_to: [u16; 2];
    /// Bits 13 to 15: he plays with a knock, at 0.9 of his strength
    /// (`FUN_006a144e`).
    0x218 => knocks, set_knocks: u16;
    /// His face: the parts of the portrait.
    0x21a => face, set_face: [u8; 2];
    /// 1 to 12.
    0x21c => strength, set_strength: u8;
    /// The main position, then two side positions, 0 none (1 the keeper).
    0x21d => positions, set_positions: [u8; 3];
    /// His club: its league and number (league 0 none).
    0x220 => club, set_club: [u8; 2];
    /// Talent: 0 huge to 4 none.
    0x222 => talent, set_talent: i8;
    0x223 => shirt, set_shirt: u8;
    /// His nations, the second 0 none.
    0x225 => nation, set_nation: u8;
    0x226 => second_nation, set_second_nation: u8;
    /// The kind of his injury or illness (from 70 an illness).
    0x228 => injury_kind, set_injury_kind: u8;
    /// Matches suspended in the low six bits; bit 7: for how long is not
    /// known yet.
    0x229 => suspension, set_suspension: u8;
    /// Weeks of a ban in the league (`FUN_0070304f`).
    0x22a => banned_weeks, set_banned_weeks: u8;
    /// Cards this season — yellow, red, yellow-red — and suspensions: in
    /// the league (matches in the low five bits of the first byte, not known
    /// yet bit 7; the second's bit 7 red, bits 5 and 6 others), the cup (bit
    /// 7 red, 6 yellow-red, else yellow) and the European cups (weeks, the
    /// low five bits).
    0x22c => league_cards, set_league_cards: [u8; 3];
    0x22f => league_ban, set_league_ban: [u8; 2];
    0x231 => cup_cards, set_cup_cards: [u8; 3];
    0x234 => cup_ban, set_cup_ban: u8;
    0x235 => europe_cards, set_europe_cards: [u8; 3];
    0x238 => europe_ban, set_europe_ban: u8;
    /// Goals this season: in the league, the cup, the European cups; and in
    /// all his career in the cup and the European cups.
    0x23c => goals, set_goals: [u8; 3];
    0x23f => career_cup_goals, set_career_cup_goals: u8;
    0x240 => career_europe_goals, set_career_europe_goals: u8;
    /// This season: own goals, assists, goals coming on as a substitute.
    0x246 => own_goals, set_own_goals: u8;
    0x247 => assists, set_assists: u8;
    0x248 => joker_goals, set_joker_goals: u8;
    /// Penalties and free kicks this season: taken, and scored.
    0x24b => penalties, set_penalties: [u8; 2];
    0x24d => free_kicks, set_free_kicks: [u8; 2];
    /// Appearances in all his career: in the cups, in the European cups.
    0x25f => career_cup_appearances, set_career_cup_appearances: [u8; 5];
    0x265 => career_europe_appearances, set_career_europe_appearances: u8;
    /// Times in the team of the day this season.
    0x268 => of_the_day, set_of_the_day: u8;
    /// Why he is away from the club, in the low four bits; bit 4 marks him
    /// on the line-up list (its status icon 6).
    0x26b => away_kind, set_away_kind: u8;
    /// A doping ban: its kind in the low two bits, how far it has got in
    /// bits 2 and 3.
    0x26d => doping, set_doping: u8;
    /// Bit 7: the long absence's length is not known yet.
    0x26e => long_unknown, set_long_unknown: u8;
    /// Bits a match sets (meaning not traced): a player with none of them
    /// takes heart from being cheered up.
    0x25b => match_bits, set_match_bits: u8;
    /// His grade in the last match, twice it, in the low four bits.
    0x273 => grade, set_grade: u8;
    /// His partner: her kind in the low two bits (0 none, girlfriend,
    /// fiancée, wife), how they stand in bits 2 to 4.
    0x27e => partner, set_partner: u8;
    /// Problems: gambled away at the president's (low two bits), with the
    /// language (bits 2 and 3).
    0x27f => problems, set_problems: u8;
    /// Names, from the first names of the nation at +0x286: his partner's,
    /// an affair's, his children's.
    0x283 => partner_name, set_partner_name: u8;
    0x284 => affair_name, set_affair_name: u8;
    0x286 => names_nation, set_names_nation: u8;
    0x288 => children_names, set_children_names: [u8; 5];
    /// His hobby, 0 none.
    0x28d => hobby, set_hobby: u8;
    /// His contentment with the trainer, the team, the club, the press, his
    /// private life, his distractions and his offers (100 middling).
    0x28f => contentment, set_contentment: [u8; 7];
    /// As a club's player: championships, cups, Champions League, Europa
    /// League, player of the year, Europe's footballer of the year, top
    /// scorer, goals of the month, goal of the year, team of the day and of
    /// the year, yellow-red and red cards, other punishments.
    0x299 => honours, set_honours: [u8; 14];
    /// European championships won (low four bits) and world cups.
    0x2a7 => titles, set_titles: u8;
    /// His form, week by week: how many weeks so far (from 26 counted round
    /// again) and a ring of the last 26.
    0x2a8 => form_weeks, set_form_weeks: u8;
    /// Goals season by season (bit 7: the season counts) and his strength
    /// by half-season, a nibble each (0 none yet).
    0x2a9 => season_goals, set_season_goals: [u8; 20];
    0x2bd => development, set_development: [u8; 20];
    0x2d1 => form_ring, set_form_ring: [u8; 26];
    /// Each league matchday of the season: his goals (high nibble) and
    /// assists, and how he played — bits 6 and 7 played, came on, went off;
    /// bits 2 to 5 his grade; the low two his card (yellow, yellow-red,
    /// red).
    0x2eb => matchday_goals, set_matchday_goals: [u8; 46];
    0x319 => matchdays, set_matchdays: [u8; 46];
    /// Bit 5: the automatic line-ups always pick him; bit 4: never.
    0x271 => picks, set_picks: u8;
    /// Sick visits: how many in the top two bits (up to 3), the days he
    /// has been out unvisited in the low six.
    0x270 => visits, set_visits: u8;
    /// His form this season, week by week: the sum and how many weeks
    /// (`FUN_006ff257`'s "DForm"; the season sets it to his form and 1).
    0x1e0 => season_form, set_season_form: i16;
    0x27b => season_form_weeks, set_season_form_weeks: u8;
    /// 100 is neither happy nor unhappy.
    0x296 => morale, set_morale: u8;
    /// What he did wrong: bit 3 + k an offence of level k (1 to 4) this
    /// week, the low four bits last week's; and how often of each level, two
    /// bits each from the lowest.
    0x297 => offences, set_offences: u8;
    0x298 => offence_counts, set_offence_counts: u8;
    /// 1: the line-up list marks him with the dumbbell (what for is not
    /// traced yet).
    0x34c => dumbbell, set_dumbbell: u8;
    /// The form the line-up showed last, for its trend.
    0x34d => last_form, set_last_form: u8;
    /// Bit k: he was punished for an offence of level k this week.
    0x34f => fined, set_fined: u8;
    0x353 => mood, set_mood: i8;
}

/// A player's career: stations of 0x14 bytes, the date first (u8 day, u8
/// month, u16 year).
const CAREER_AT: usize = 0x20;
const CAREER_STATION: usize = 0x14;

impl Player {
    /// When his last station began: the day he came to his club.
    pub fn joined(&self) -> Option<(u8, u8, u16)> {
        let count = usize::try_from(self.career_count()).ok().filter(|&n| n > 0)?;
        let at = CAREER_AT + (count - 1) * CAREER_STATION;
        Some((self.get(at), self.get(at + 1), self.get(at + 2)))
    }

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
    /// The co-trainer, 0 none (the club object's +0x84).
    0xa2 => co_trainer, set_co_trainer: i16;
    /// A human club's manager: his slot and one, whose extra data the club
    /// has ([`Extra`]); 0 none.
    0xb5 => human, set_human: u8;
    /// What the team counts for in goal, defence, midfield and attack, in
    /// tenths, as the game last reckoned it (the club object's +0x94 …
    /// +0x9a): the line-up's comparison shows the opponent's.
    0xb8 => strengths, set_strengths: [i16; 4];
    0x112 => player_count, set_player_count: u16;
    /// Its number among its country's teams.
    0x129 => number, set_number: u8;
    /// The country: a nation code.
    0x12d => country, set_country: u8;
    /// Its league's level, 0 the first.
    0x12e => level, set_level: u8;
    /// The team's mood ("Stimmung"), 0 to 200 (`FUN_007147ce`).
    0xc4 => mood, set_mood: u8;
    /// Kept between 10 and 50, 30 at first (meaning not traced).
    0x138 => spirit, set_spirit: u8;
    /// This season: bonuses paid the first team, fines taken from players.
    0x500 => bonuses_paid, set_bonuses_paid: i32;
    0x504 => fines, set_fines: i32;
    /// Penalties this season in competitive matches: awarded, scored; and
    /// direct free kicks: taken, scored (the club object's +0xdea … +0xdee).
    0x332 => penalties, set_penalties: [u8; 2];
    0x334 => free_kicks, set_free_kicks: [u16; 2];
    /// The team's bonuses: 0 full, 1 half, 2 none.
    0x830 => bonus_cut, set_bonus_cut: i16;
    /// What the manager has done to the team's will to win, -200 to 200
    /// (`FUN_0069efe8`).
    0x95a => will_to_win, set_will_to_win: i16;
    /// The club it is one with, by number (or, if the first is below 0,
    /// the second): a club and its amateurs.
    0x872 => linked, set_linked: [i8; 2];
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

/// What a human club keeps besides its team record, one for each human
/// slot (0x2940 bytes at `DAT_00a58078` + slot × 0x2940; `FUN_0063b75a`
/// copies parts into the club object).
const EXTRA_AT: u32 = 0xa58078;

record! {
    /// A human club's extra data.
    Extra, 0x2940 - HEADER, base HEADER;
    /// Training sessions of four kinds so far, which cohesion counts (the
    /// club object's +0xcf0, +0xcfe, +0xd18, +0xd1a: the kinds 13, 20, 33
    /// and 34 of `FUN_00749508`).
    0x356 => sessions_13, set_sessions_13: i16;
    0x364 => sessions_20, set_sessions_20: i16;
    0x37e => sessions_33, set_sessions_33: i16;
    0x380 => sessions_34, set_sessions_34: i16;
    /// Who takes the set pieces and who is captain: six lists of player
    /// ids in the manager's order, a 0 ending each — penalties, free kicks
    /// crossed, free kicks shot, corners from the left, from the right, the
    /// captain ([`crate::duties`]).
    0xcc => duty_lists, set_duty_lists: [[u16; 35]; 6];
    /// A count cohesion takes twice (`FUN_0069d5a7`; meaning not traced).
    0x2564 => together, set_together: i16;
    /// The line-up's special settings (`CMannaufSpezial`): bit 1 the
    /// co-trainer lines the team up before a match in a system of his
    /// choosing, bit 2 in the one it stands in.
    0x2824 => line_up_flags, set_line_up_flags: u16;
}

impl Extra {
    /// What the training sessions add to cohesion, before the game halves
    /// what is above 30.
    pub fn training(&self) -> i32 {
        [self.sessions_13(), self.sessions_20(), self.sessions_33(), self.sessions_34()]
            .into_iter()
            .map(i32::from)
            .sum::<i32>()
            + 2 * i32::from(self.together())
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
    /// His abilities at every level, seven a row: the fourth his authority
    /// (`Autorität`).
    0x34 => abilities, set_abilities: [[i16; 7]; 16];
    /// How the public, the fans and the media like him, 0 to 100.
    0x126 => popularity, set_popularity: [u8; 3];
    /// When he signed with his club: day, month, year.
    0x17c => signed_day, set_signed_day: u8;
    0x17d => signed_month, set_signed_month: u8;
    0x17e => signed_year, set_signed_year: u16;
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

    /// Keeps where the eleven stand in one of his formations.
    pub fn set_formation_spots(&mut self, formation: usize, spots: [(i16, i16); 11]) {
        let at = FORMATIONS_AT + formation * FORMATION_SIZE + FORMATION_NAME;
        self.put(at, spots.map(|(x, y)| [x, y]));
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

/// The human managers' week planners (`CDatenTerminplaner`), by slot.
const PLANNER_AT: u32 = 0x1c39620;
/// A day of a planner: its kind, a flag, and what it is about.
const PLANNER_DAYS: usize = 378;
const PLANNER_ENTRY: usize = 0x1c;
const PLANNER_ENTRIES_AT: usize = 0x24;

record! {
    /// A manager's week planner: an appointment for every day of the season
    /// (`0x1c39620` + slot × 0x297c).
    Planner, 10588, base HEADER;
}

/// An appointment of a planner: what kind of appointment (0 none), and the
/// player it is about, for the kinds about a player.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Appointment {
    pub kind: u8,
    pub flag: u8,
    pub player: u16,
}

impl Planner {
    pub fn day(&self, day: usize) -> Appointment {
        if day >= PLANNER_DAYS {
            return Appointment::default();
        }
        let at = PLANNER_ENTRIES_AT + day * PLANNER_ENTRY;
        Appointment { kind: self.get(at), flag: self.get(at + 1), player: self.get(at + 4) }
    }

    /// Books an appointment for a day; what else the day's entry held stays
    /// (`FUN_0072e973`).
    pub fn book(&mut self, day: usize, appointment: Appointment) {
        if day < PLANNER_DAYS {
            let at = PLANNER_ENTRIES_AT + day * PLANNER_ENTRY;
            self.put(at, appointment.kind);
            self.put(at + 1, appointment.flag);
            self.put(at + 4, appointment.player);
        }
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
    /// The start setting "Vorbereitung und Kalkulation" (dialog 256):
    /// normal 5, fast 7, only competitive matches 12.
    0xb5d152 => preparation, set_preparation: u8;
    /// The countries played besides the main one: how many, which.
    0xb5cbae => more_count, set_more_count: i16;
    0xb5cbb0 => main_country, set_main_country: i16;
    0xb5cbb4 => more_countries, set_more_countries: [i16; 4];
    /// The year the game started in.
    0xb5cbca => start_year, set_start_year: u16;
    0xb5cbcc => year, set_year: u16;
    /// How many humans play.
    0xb5cbce => humans, set_humans: u16;
    /// Half weeks since the start.
    0xb5cbd2 => half_weeks, set_half_weeks: u16;
    /// Days since the start of the first season.
    0xb5cbd4 => day, set_day: u16;
    /// The day the week began with.
    0xb5cbd6 => week_start, set_week_start: i16;
    /// The game's day 0 is this many days after the first of July of the
    /// start year (`FUN_0067677e`).
    0xb5cbd8 => first_day, set_first_day: i16;
    0xb5cbda => date, set_date: [u16; 3];
    /// The day of the year, from 0.
    0xb5cbe0 => day_of_year, set_day_of_year: u16;
    /// Days in this year.
    0xb5cca4 => year_days, set_year_days: u16;
    /// A player for each position whose market value counts a fifth more,
    /// by id.
    0xb5dfd0 => stars, set_stars: [u16; 11];
    /// Where the game cycle goes on after loading (`FUN_006da3a9`): 998
    /// inside the week, the half weeks at a week's turn.
    0xb5d172 => resume, set_resume: u16;
}

impl Globals {
    /// Whether the game follows a country's matches: the main country's
    /// and the more ones' (`FUN_00676c03`).
    pub fn played(&self, country: i16) -> bool {
        let more = usize::try_from(self.more_count()).unwrap_or(0);
        self.main_country() == country
            || self.more_countries().iter().take(more).any(|&c| c == country)
    }

    /// The date of a day of the game (`FUN_0067677e`).
    pub fn date_of(&self, day: i16) -> Option<NaiveDate> {
        let start = NaiveDate::from_ymd_opt(self.start_year().into(), 7, 1)?;
        start.checked_add_signed(chrono::Duration::days(
            i64::from(day) + i64::from(self.first_day()),
        ))
    }
}

/// A table of luck the game draws anew each season: thirty numbers below 30,
/// a player's by his id (`DAT_00ae6de8`, `FUN_0065e659`).
const LUCK_AT: u32 = 0xae6de8;
record! {
    Luck, 60, base 0xae6de8;
    0xae6de8 => values, set_values: [i16; 30];
}

impl Luck {
    /// A player's luck.
    pub fn of(&self, id: u16) -> i16 {
        self.values()[usize::from(id) % 30]
    }
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

    /// How a country's clubs are kept: 1 and 2 its teams (1 with forty
    /// amateur clubs after them), others only abroad.
    pub fn kind(&self, country: usize) -> u8 {
        self.get(0xae61ae + country)
    }

    /// By which rule yellow cards add up to a suspension in a country's
    /// league and in its cup (`FUN_0074612e`, `FUN_007464a4`).
    pub fn card_rules(&self, country: usize) -> [i32; 2] {
        let system: u8 = self.get(0xae5c97 + country);
        self.get(0xae62cc + 8 * usize::from(system))
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
    /// The human clubs' extra data, by slot.
    pub extras: Vec<Extra>,
    /// The human managers' week planners, by slot.
    pub planners: Vec<Planner>,
    pub luck: Luck,
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
        let extras = (0..MANAGERS)
            .map(|n| Extra::new(block(format!("{EXTRA_AT:#x}[{n}]"))?))
            .collect::<io::Result<_>>()?;
        let planners = (0..MANAGERS)
            .map(|n| Planner::new(block(format!("{PLANNER_AT:#x}[{n}]"))?))
            .collect::<io::Result<_>>()?;
        let globals = Globals::new(block(format!("{GLOBALS_AT:#x}"))?)?;
        let luck = Luck::new(block(format!("{LUCK_AT:#x}"))?)?;
        let countries = Countries::new(block(format!("{COUNTRIES_AT:#x}"))?)?;
        let names = block("names".to_owned())?.split(|&b| b == 0).map(cp1252).collect();
        Ok(Self {
            players,
            teams,
            managers,
            globals,
            countries,
            extras,
            planners,
            luck,
            names,
            save,
        })
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
        for (n, extra) in self.extras.iter().enumerate() {
            put(format!("{EXTRA_AT:#x}[{n}]"), 0, extra.bytes());
        }
        for (n, planner) in self.planners.iter().enumerate() {
            put(format!("{PLANNER_AT:#x}[{n}]"), 0, planner.bytes());
        }
        put(format!("{GLOBALS_AT:#x}"), 0, self.globals.bytes());
        put(format!("{LUCK_AT:#x}"), 0, self.luck.bytes());
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

    pub fn team_mut(&mut self, country: u16, number: u16) -> Option<&mut Team> {
        let first = usize::try_from(self.countries.first_team(country.into())).ok()?;
        self.teams.get_mut(first + usize::from(number))
    }

    pub fn player(&self, id: u16) -> Option<&Player> {
        (id != 0).then(|| self.players.get(usize::from(id))).flatten()
    }

    /// The slot of the `n`th manager in play.
    pub fn human_slot(&self, n: usize) -> Option<usize> {
        self.globals.slots().iter().enumerate().filter(|(_, s)| **s > 0).nth(n).map(|(i, _)| i)
    }

    /// The name a player goes by: his nickname if he has one shown, else
    /// his first and last name (`FUN_006feb4d`).
    pub fn known_as(&self, id: u16) -> Option<String> {
        let r = self.player(id)?;
        Some(if r.shown() & 0x8000_0000 != 0 {
            self.name(r.nickname()).to_owned()
        } else {
            format!("{} {}", self.name(r.first_name()), self.name(r.last_name()))
        })
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
