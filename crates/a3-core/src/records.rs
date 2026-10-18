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
numbers!(u8, i8, u16, i16, u32, i32, i64);

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
    /// How hard he trains ([`crate::individual`]): 0 out, 1 building up
    /// (the line-up list's dumbbell) … 4 hard.
    0x34c => intensity, set_intensity: u8;
    /// His favourite ability, for his first individual training: its
    /// number from 1 in the low five bits, how far he got above.
    0x34b => favourite, set_favourite: u8;
    /// The minutes he played in the last match.
    0x25c => minutes, set_minutes: u8;
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
    /// The computer club's staff, by person ([`Person`]): a manager who is
    /// also trainer, a trainer, a manager (the club object's +0x7e …).
    0x9c => staff, set_staff: [u16; 3];
    /// The human managers, by number from 1: the trainer, the manager.
    0xa8 => humans, set_humans: [u8; 2];
    /// League wins, draws and defeats this season.
    0xd4 => record, set_record: [u8; 3];
    /// What the board is like (`FUN_007309a3`), 0 to 5.
    0x123 => board, set_board: u8;
    /// Its place among its league's teams, and in the table.
    0x12f => league_place, set_league_place: u8;
    0x132 => table_place, set_table_place: u8;
    /// Home games this season with a gate, and the gates.
    0x260 => home_games, set_home_games: u8;
    0x264 => gates, set_gates: [i32; 23];
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
    /// Its tactics ([`crate::tactics`]; the club object's +0xa0, +0xa8 …
    /// +0xac): the offside trap's strength, how it plays, its effort and
    /// hardness, the flags; and its passing, which the game keeps here
    /// only.
    0xc6 => offside, set_offside: u8;
    /// What the team can of pressing and of beating the offside trap, 0
    /// to 5, and of set pieces: free kicks in bits 0–2, corners 3–5,
    /// throw-ins 6–8, dirty tricks 12–14 (the club object's +0x9f, +0xa1,
    /// +0xa2; TRAINING-KURZ.md).
    0xc5 => pressing, set_pressing: u8;
    /// The individual training's four buttons, two bits each
    /// ([`crate::individual::state`]).
    0xcf => intensity_states, set_intensity_states: u8;
    0xc7 => anti_offside, set_anti_offside: u8;
    0xc8 => set_pieces, set_set_pieces: u16;
    /// Before the next match: bit 0x800 a training camp, bit 0x1000 a
    /// team meeting (the club object's +0xdf8).
    0x308 => training_flags, set_training_flags: u32;
    /// What watching players costs the club, 10,000 a player on its
    /// observation list (the club object's +0x27c): how much it knows of
    /// its opponents.
    0x4f4 => observation_cost, set_observation_cost: i32;
    0xcc => style, set_style: u8;
    0xcd => effort, set_effort: u8;
    0xce => hardness, set_hardness: u8;
    0xd0 => tactic_flags, set_tactic_flags: u16;
    0xd2 => passing, set_passing: u16;
    /// Its next match (the club object's +0xb46 …): the opponent's country,
    /// 0 none, and number there; where it is played, 0 or less at home; its
    /// kind (2 the league, 3 and 4 the cup, 5 to 10 the European cups); and
    /// its referee, 0 none.
    0x252 => next_country, set_next_country: u8;
    0x254 => next_number, set_next_number: i16;
    0x256 => next_venue, set_next_venue: i16;
    0x258 => next_kind, set_next_kind: u8;
    0x25a => referee, set_referee: i16;
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
    /// Man-marking ([`crate::marking`]; the club object's +0xabe, +0xad2):
    /// the ids of up to ten of the eleven, and of the opponent's players
    /// each marks.
    0x16c => markers, set_markers: [u16; 10];
    0x180 => marked, set_marked: [u16; 10];
    /// Where each of the eleven acts, in pitch pixels from his spot: left,
    /// up, right, down (the club object's +0xb12).
    0x1c0 => areas, set_areas: [[u8; 4]; 11];
    /// The player who goes forward at set pieces, by id, 0 none (the club
    /// object's +0xb3e).
    0x1ec => set_piece, set_set_piece: u16;
    /// The respect the team is held in, 0 to 200 (the club object's +0x92).
    0xb6 => respect, set_respect: i16;
    /// The team's will to win as it stands, before the manager's part,
    /// [`Team::will_to_win`] (`FUN_0069ee6d`).
    0x958 => will_base, set_will_base: i16;
    /// Season tickets: the target the preseason sells towards, and how many
    /// are sold (the club object's +0x22c and +0x498).
    0x4a4 => ticket_target, set_ticket_target: i32;
    0x710 => tickets, set_tickets: i32;
}

/// Where a team lists its players.
const SQUAD: usize = 0x1f2;
/// Where its name is, and how long it may be.
const TEAM_NAME: (usize, usize) = (0x20, 0x7c);
/// The most a team's will to win may be.
const WILL_MOST: i16 = 200;

impl Team {
    pub fn name(&self) -> String {
        text(&self.0[TEAM_NAME.0 - HEADER..], TEAM_NAME.1)
    }

    /// The team's will to win, 0 to 200 (`FUN_0069ee6d`).
    pub fn will(&self) -> i16 {
        self.will_base().wrapping_add(self.will_to_win()).clamp(0, WILL_MOST)
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
    /// The training plan: three sessions a day for six weeks, a unit each
    /// ([`crate::training`]; the club object's +0xc0a).
    0x270 => plan, set_plan: [u8; 126];
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
    /// The tactics' players, by id (0 none): one to spare, one to collect a
    /// yellow card, the opponent's to foul and to provoke; and the special
    /// win bonus ([`crate::tactics`]).
    0x38a => picks, set_picks: [u16; 4];
    0x394 => special_bonus, set_special_bonus: i32;
    /// The line-up's special settings (`CMannaufSpezial`): bit 1 the
    /// co-trainer lines the team up before a match in a system of his
    /// choosing, bit 2 in the one it stands in.
    0x2824 => line_up_flags, set_line_up_flags: u16;
    /// What the co-trainer trains when he plans a week: days, units and
    /// each focus' units ([`crate::training::CoTraining`]); bit 0 of the
    /// flags above lets him plan every week.
    0x2826 => co_training_days, set_co_training_days: i16;
    0x2828 => co_training_units, set_co_training_units: i16;
    0x282a => co_training_focus, set_co_training_focus: [u8; 9];
    /// The player the detective watches (0 nobody).
    0x2834 => detective, set_detective: u16;
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

/// The referees, by id (the save's blocks of their records, 0x20 before
/// their fields).
const REFEREE_AT: u32 = 0x19f4ed8;
const REFEREES: usize = 240;

/// The people who are not players: computer clubs' trainers and managers
/// and more (0x54 bytes at `DAT_00be3920` + id × 0x54, ids 1 to 1499).
const PERSON_AT: u32 = 0xbe3920;
const PERSONS: usize = 1500;

record! {
    /// A person: his names in the pool, and how safe his job is (0 safe,
    /// 1–2 wobbling, 3 and more about to go; `FUN_00732813`).
    Person, 0x54 - HEADER, base HEADER;
    0x20 => names, set_names: [u16; 2];
    0x2d => job, set_job: u8;
    /// The kind of training a co-trainer plans, the row of his templates'
    /// odds (`FUN_006317c6`).
    0x26 => training_kind, set_training_kind: u8;
}

/// The stadiums of the teams played (0x290 bytes at `DAT_01a48138` + team
/// index × 0x290), and of the foreign ones (0xf0 at `DAT_01a3af38` + index
/// from the first foreign team): the name, how many it holds.
const STADIUM_AT: u32 = 0x1a48138;
const FOREIGN_STADIUM_AT: u32 = 0x1a3af38;
const STADIUM_NAME: (usize, usize) = (0x22, 0x3a);
const STADIUM_CAPACITY: usize = 0x5c;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Stadium {
    pub name: String,
    pub capacity: i32,
}

impl Stadium {
    fn new(bytes: &[u8]) -> Option<Self> {
        let at = |offset: usize| offset - HEADER;
        let capacity = bytes.get(at(STADIUM_CAPACITY)..at(STADIUM_CAPACITY) + 4)?;
        Some(Self {
            name: text(bytes.get(at(STADIUM_NAME.0)..)?, STADIUM_NAME.1),
            capacity: i32::from_le_bytes(capacity.try_into().ok()?),
        })
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
    /// His options: the line-up's view in bits 0 to 3 (0 the pitch, else
    /// the list), the screen his turn starts on in bits 6 and 7, the
    /// short-term training as pictures in bit 8.
    0x212c => options, set_options: u32;
    /// How the week's player in individual training is chosen when nobody
    /// is: 0 not, else the least in form of all, of the eleven, of the
    /// keepers and defenders, the midfielders, the strikers.
    0x14a8 => individual_rule, set_individual_rule: u8;
    /// A training camp's conditions: food, going out, control, 0 to 4.
    0x14aa => camp_rules, set_camp_rules: [u8; 3];
    /// 1: he plays in his own team too, a player-manager (the manager
    /// object's +0x26).
    0x2e8 => playing, set_playing: i16;
}

/// The bits of a manager's options that choose the line-up's view.
const LINE_UP_VIEW: u32 = 0xf;
/// The bit that shows his short-term training as pictures (dialog 513).
const TRAINING_PICTURES: u32 = 0x100;

/// A manager's week templates: a name of 30 bytes and a week's sessions
/// each (the manager object's +0x38a).
const TEMPLATES_AT: usize = 0x4d6;
const TEMPLATE_SIZE: usize = 0x33;
const TEMPLATE_NAME: usize = 30;
pub const TEMPLATES: usize = 18;
/// What he chose for each week of the game, counted from the first
/// season's start (the manager object's +0x720): the template, 1 more
/// than its number (0 none), and the player in individual training (0
/// none).
const WEEKS_AT: usize = 0x86c;
const WEEK_SIZE: usize = 0x3a;
const WEEK_TEMPLATE: usize = 4;
const WEEK_PLAYER: usize = 6;
const WEEK_TEMPLATE_NAME: (usize, usize) = (8, 30);
const WEEK_PLAYER_NAME: (usize, usize) = (0x26, 20);
/// The season's weeks a manager plans.
pub const WEEKS: usize = 50;

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

    /// His week templates: their names and sessions.
    pub fn templates(&self) -> Vec<(String, [u8; 21])> {
        (0..TEMPLATES)
            .map(|t| {
                let at = TEMPLATES_AT + t * TEMPLATE_SIZE;
                (text(&self.0[at..], TEMPLATE_NAME), self.get(at + TEMPLATE_NAME))
            })
            .collect()
    }

    /// What he planned for a week of the season.
    pub fn week(&self, week: usize) -> crate::training::Week {
        let at = WEEKS_AT + week.min(WEEKS - 1) * WEEK_SIZE;
        let name = |(offset, len): (usize, usize)| text(&self.0[at + offset..], len);
        crate::training::Week {
            done: self.get::<u8>(at) != 0,
            kind: self.get(at + 1),
            condition: self.get(at + 2),
            freshness: self.get(at + 3),
            template: self.get(at + WEEK_TEMPLATE),
            player: self.get(at + WEEK_PLAYER),
            template_name: name(WEEK_TEMPLATE_NAME),
            player_name: name(WEEK_PLAYER_NAME),
        }
    }

    pub fn set_week(&mut self, week: usize, entry: &crate::training::Week) {
        let at = WEEKS_AT + week.min(WEEKS - 1) * WEEK_SIZE;
        self.put(at, u8::from(entry.done));
        self.put(at + 1, entry.kind);
        self.put(at + 2, entry.condition);
        self.put(at + 3, entry.freshness);
        self.put(at + WEEK_TEMPLATE, entry.template);
        self.put(at + WEEK_PLAYER, entry.player);
        for ((offset, len), name) in
            [(WEEK_TEMPLATE_NAME, &entry.template_name), (WEEK_PLAYER_NAME, &entry.player_name)]
        {
            let field = &mut self.0[at + offset..at + offset + len];
            let bytes: Vec<u8> =
                name.chars().map(|c| a3_files::bitmap_font::to_cp1252(c).unwrap_or(b'?')).collect();
            let n = bytes.len().min(len - 1);
            field[..n].copy_from_slice(&bytes[..n]);
            field[n..].fill(0);
        }
    }

    /// Whether his short-term training shows as pictures (`FUN_006a5cd5`).
    pub fn training_pictures(&self) -> bool {
        self.options() & TRAINING_PICTURES != 0
    }

    pub fn set_training_pictures(&mut self, pictures: bool) {
        let bit = if pictures { TRAINING_PICTURES } else { 0 };
        self.set_options(self.options() & !TRAINING_PICTURES | bit);
    }

    /// Whether his line-up shows as a list (`FUN_006a5ba7`).
    pub fn list_view(&self) -> bool {
        self.options() & LINE_UP_VIEW != 0
    }

    /// Switches his line-up's view, as the turn does after the view's
    /// switch: the list is 1 (`FUN_006a5bcb`).
    pub fn set_list_view(&mut self, list: bool) {
        self.set_options(self.options() & !LINE_UP_VIEW | u32::from(list));
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
    /// (`0x1c39620` + slot × 0x297c); his year calendar is one too.
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

    /// The teams of a day's match in a year calendar, home first: their
    /// countries and numbers.
    pub fn teams(&self, day: usize) -> Option<[(u16, i16); 2]> {
        if day >= PLANNER_DAYS {
            return None;
        }
        let at = PLANNER_ENTRIES_AT + day * PLANNER_ENTRY;
        Some([(self.get(at + 8), self.get(at + 0xa)), (self.get(at + 0xc), self.get(at + 0xe))])
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

/// The human clubs' individual training, by slot (`0xa50028` + slot ×
/// 0x7a0): forty players' choices ([`crate::individual::Entry`]).
const INDIVIDUAL_AT: u32 = 0xa50028;

record! {
    /// A human club's individual training.
    IndividualStore, crate::individual::ENTRIES * crate::individual::ENTRY, base HEADER;
}

impl IndividualStore {
    pub fn entries(&self) -> Vec<crate::individual::Entry> {
        self.0.chunks(crate::individual::ENTRY).map(crate::individual::Entry::read).collect()
    }

    pub fn set_entries(&mut self, entries: &[crate::individual::Entry]) {
        for (bytes, entry) in self.0.chunks_mut(crate::individual::ENTRY).zip(entries) {
            entry.write(bytes);
        }
    }
}

/// The human managers' year calendars, by slot (`0x1c2f030` + slot ×
/// 0x297c, the same entries as the planners'): the kind of every day of
/// the season — 6 a match, 7 a party, 8 an event.
const CALENDAR_AT: u32 = 0x1c2f030;

/// The block of globals around the date (`0xb5cb68`, its data from
/// `0xb5cb88`).
const GLOBALS_AT: u32 = 0xb5cb68;

record! {
    /// The globals of the game's date and its humans.
    Globals, 0x1590, base 0xb5cb88;
    /// Whether each human slot is in play: above 0.
    0xb5cb9c => slots, set_slots: [i16; MANAGERS];
    /// Seasons so far, 1 in the first.
    0xb5cbc8 => seasons, set_seasons: u16;
    /// The funds' prices, which deposits are worth by (`DAT_00b5cc8c`).
    0xb5cc8c => fund_prices, set_fund_prices: [i32; 5];
    /// How far the European cups are: the Champions League's stage (0 the
    /// qualifier, 1 and 2 the group stages, then the knock-out rounds) and
    /// the UEFA cup's round (EUROPAPOKAL-STAND.md).
    0xb5d164 => europe_stage, set_europe_stage: i16;
    0xb5d166 => uefa_round, set_uefa_round: i16;
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
    /// A country's slot among those with tables of their own: titles,
    /// archives, cups (`DAT_00ae5c97`).
    pub fn slot(&self, country: usize) -> u8 {
        self.get(0xae5c97 + country)
    }

    /// A country's leagues: the first's index among all leagues, how many
    /// (`DAT_00ae5fda`, `DAT_00ae60c4`).
    pub fn first_league(&self, country: usize) -> i16 {
        self.get(0xae5fda + 2 * country)
    }

    pub fn leagues(&self, country: usize) -> i16 {
        self.get(0xae60c4 + 2 * country)
    }

    /// Where the foreign teams start among all, for their stadiums
    /// (`DAT_00ae6304`).
    pub fn foreign_start(&self) -> i16 {
        self.get(0xae6304)
    }

    /// The European cups' names, the Champions League's and the UEFA
    /// cup's, as the new game set them (`DAT_00ae668e`, `DAT_00ae66ce`).
    pub fn cup_names(&self) -> [String; 2] {
        [0xae668e, 0xae66ce].map(|at| text(&self.0[at - Self::BASE..], 0x40))
    }

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
    pub calendars: Vec<Planner>,
    pub individual: Vec<IndividualStore>,
    pub luck: Luck,
    /// The pool of names the players' names point into.
    pub names: Vec<String>,
    /// By id.
    pub persons: Vec<Person>,
    /// By team index, and the foreign teams' from the first.
    pub stadiums: Vec<Stadium>,
    pub foreign_stadiums: Vec<Stadium>,
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
        let calendars = (0..MANAGERS)
            .map(|n| Planner::new(block(format!("{CALENDAR_AT:#x}[{n}]"))?))
            .collect::<io::Result<_>>()?;
        let individual = (0..MANAGERS)
            .map(|n| IndividualStore::new(block(format!("{INDIVIDUAL_AT:#x}[{n}]"))?))
            .collect::<io::Result<_>>()?;
        let globals = Globals::new(block(format!("{GLOBALS_AT:#x}"))?)?;
        let luck = Luck::new(block(format!("{LUCK_AT:#x}"))?)?;
        let countries = Countries::new(block(format!("{COUNTRIES_AT:#x}"))?)?;
        let names = block("names".to_owned())?.split(|&b| b == 0).map(cp1252).collect();
        let persons = (0..PERSONS)
            .map(|n| Person::new(block(format!("{PERSON_AT:#x}[{n}]"))?))
            .collect::<io::Result<_>>()?;
        let stadiums = |at: u32| -> Vec<Stadium> {
            (0..)
                .map_while(|n| blocks.get(format!("{at:#x}[{n}]").as_str()).copied())
                .map(|bytes| {
                    Stadium::new(bytes).unwrap_or(Stadium { name: String::new(), capacity: 0 })
                })
                .collect()
        };
        let (stadiums, foreign_stadiums) = (stadiums(STADIUM_AT), stadiums(FOREIGN_STADIUM_AT));
        Ok(Self {
            players,
            teams,
            managers,
            globals,
            countries,
            extras,
            planners,
            calendars,
            individual,
            luck,
            names,
            persons,
            stadiums,
            foreign_stadiums,
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
        for (n, calendar) in self.calendars.iter().enumerate() {
            put(format!("{CALENDAR_AT:#x}[{n}]"), 0, calendar.bytes());
        }
        for (n, store) in self.individual.iter().enumerate() {
            put(format!("{INDIVIDUAL_AT:#x}[{n}]"), 0, store.bytes());
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

    /// A block of the save the records do not read, by its name: an
    /// object's address, `[n]` for one of many.
    pub fn block(&self, name: &str) -> Option<&[u8]> {
        self.save.blocks.iter().find(|b| b.name == name).map(|b| b.data.as_slice())
    }

    /// A person's name: both names, or the one he has (`FUN_00637cbb`).
    pub fn person_name(&self, id: u16) -> String {
        let Some(person) = self.persons.get(usize::from(id)).filter(|_| id != 0) else {
            return String::new();
        };
        match person.names() {
            [0, 0] => String::new(),
            [0, one] | [one, 0] => self.name(one).to_owned(),
            [first, last] => format!("{} {}", self.name(first), self.name(last)),
        }
    }

    /// A referee by id, 1 to 240: his name, competence and hardness
    /// (`FUN_0063161e`; 0x34 bytes at `DAT_019f4ef8` + id × 0x34).
    pub fn referee(&self, id: i16) -> Option<(&str, u8, u8)> {
        let id = usize::try_from(id).ok().filter(|id| (1..=REFEREES).contains(id))?;
        let bytes = self.block(&format!("{REFEREE_AT:#x}[{id}]"))?;
        let name = u16::from_le_bytes([*bytes.get(2)?, *bytes.get(3)?]);
        Some((self.name(name), *bytes.get(5)?, *bytes.get(6)?))
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
