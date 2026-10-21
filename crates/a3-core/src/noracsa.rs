//! NORACSA's player search, "Search, Find, and Buy" (VEREINE-NORACSA.md
//! §2 in the RE repo): the criteria a manager ticks, the clubs searched in
//! the leagues chosen, the test every player of theirs goes through.

use std::io;

use a3_files::exe::Exe;
use chrono::NaiveDate;

use crate::records::{Player, Records};

/// The league choices' special codes by the main country, 0xff ending
/// each list: Germany's, England's, any other's (`0x885c7c`).
const CODES_AT: [(i16, u32); 2] = [(GERMANY, 0x885c7c), (ENGLAND, 0x885c84)];
const OTHER_CODES_AT: u32 = 0x885c8c;
const CODES_END: u8 = 0xff;
const CODE_TEXTS_AT: u32 = 0x885c5c;
const CODES: usize = 8;
/// The traits' and abilities' choices, (text, data) pairs ending in text 0
/// (`0x885fb0`, `0x885e10`).
const TRAITS_AT: u32 = 0x885fb0;
const ABILITIES_AT: u32 = 0x885e10;
/// The same for a keeper and for a field player, which "Bitte melden!"
/// offers by the position chosen (`FUN_005270c1`).
const KEEPER_TRAITS_AT: u32 = 0x885ee8;
const KEEPER_ABILITIES_AT: u32 = 0x885d28;
const FIELD_TRAITS_AT: u32 = 0x885f20;
const FIELD_ABILITIES_AT: u32 = 0x885d80;
const MOST_CHOICES: usize = 32;

const GERMANY: i16 = 14;
const ENGLAND: i16 = 16;
/// The special codes: everything, the main country, its first and second
/// league, England's third and fourth, Germany's regional leagues, the
/// further countries played, the rest abroad.
pub const ALL: u8 = 0;
const MAIN: u8 = 1;
const MAIN_FIRST: u8 = 2;
const MAIN_SECOND: u8 = 3;
const ENGLAND_LOW: u8 = 4;
const REGIONAL: u8 = 5;
pub const BONUS: u8 = 6;
pub const ABROAD: u8 = 7;
/// A code from this on is the main country's league of that place.
pub const MAIN_LEAGUE: u8 = 100;
const OWN_LEAGUES: u8 = 200;
const LAST_COUNTRY: u16 = 0x74;
const NO_LEAGUE: u8 = 10;

/// The most hits a search keeps.
pub const MOST_HITS: usize = 100;
/// The age bands and the goal bands the buttons tick (`0x8a68b4`,
/// `0x8a68cc`); ages and goals past them are not looked at.
const AGES: [(usize, usize); 6] = [(17, 19), (20, 22), (23, 25), (26, 28), (29, 31), (32, 49)];
const AGE_LIMIT: usize = 50;
const GOALS: [(usize, usize); 7] =
    [(0, 99), (0, 99), (6, 99), (11, 99), (16, 99), (21, 99), (26, 99)];
const GOAL_LIMIT: usize = 100;
const LAST_REAL_ID: u16 = 0x4268;
const GOALKEEPER: u8 = 1;
/// R+0x282 bit 0x10: counts as home-grown ("Fußballinländer").
const HOME_GROWN: u8 = 0x10;
const CONTRACT_YEAR: u16 = 0xfff;
/// The most unhappy contentment level.
const UNHAPPY: usize = 3;

/// The flags of C+0x39: only with a fixed release fee, only national
/// players, only the really unhappy.
pub const FIXED_FEE: u8 = 0x01;
pub const NATIONAL: u8 = 0x40;
pub const UNHAPPY_ONLY: u8 = 0x80;

/// The choices of the screen's combos, as the exe lists them.
pub struct NoracsaTables {
    /// The league combo's special codes and their texts.
    pub codes: Vec<(i16, Vec<u8>)>,
    pub other_codes: Vec<u8>,
    pub code_texts: Vec<u16>,
    /// The traits and the abilities: text and data, "egal" first (data
    /// 100); a keeper's ability has bit 0x8000.
    pub traits: Vec<(u16, u32)>,
    pub abilities: Vec<(u16, u32)>,
    pub keeper_traits: Vec<(u16, u32)>,
    pub keeper_abilities: Vec<(u16, u32)>,
    pub field_traits: Vec<(u16, u32)>,
    pub field_abilities: Vec<(u16, u32)>,
}

impl NoracsaTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let codes = |at: u32| -> io::Result<Vec<u8>> {
            Ok(exe.bytes(at, CODES)?.iter().copied().take_while(|&c| c != CODES_END).collect())
        };
        let pairs = |at: u32| -> io::Result<Vec<(u16, u32)>> {
            let words = exe.u32s(at, 2 * MOST_CHOICES)?;
            Ok(words.chunks(2).take_while(|p| p[0] != 0).map(|p| (p[0] as u16, p[1])).collect())
        };
        Ok(Self {
            codes: CODES_AT
                .iter()
                .map(|&(country, at)| Ok((country, codes(at)?)))
                .collect::<io::Result<_>>()?,
            other_codes: codes(OTHER_CODES_AT)?,
            code_texts: exe.u32s(CODE_TEXTS_AT, CODES)?.into_iter().map(|t| t as u16).collect(),
            traits: pairs(TRAITS_AT)?,
            abilities: pairs(ABILITIES_AT)?,
            keeper_traits: pairs(KEEPER_TRAITS_AT)?,
            keeper_abilities: pairs(KEEPER_ABILITIES_AT)?,
            field_traits: pairs(FIELD_TRAITS_AT)?,
            field_abilities: pairs(FIELD_ABILITIES_AT)?,
        })
    }

    /// The special codes the league combo starts with, by the main
    /// country.
    pub fn codes_of(&self, main: i16) -> &[u8] {
        self.codes.iter().find(|(c, _)| *c == main).map_or(&self.other_codes, |(_, c)| c)
    }
}

/// What a search looks for (C, `FUN_006e287e`).
#[derive(Clone, Default, Debug)]
pub struct Criteria {
    /// A pattern of the name, `?` one letter, `*` any run.
    pub name: String,
    /// Bit k for strength k, 1 to 13.
    pub strengths: u16,
    /// The age bands ticked, bits 0 to 5.
    pub ages: u16,
    /// Bit k for main position k.
    pub positions: u16,
    /// The leagues: a country, and a code (a special code with no
    /// country, a league place from 100 with the main country).
    pub league: (u8, u8),
    /// 0 any; 1 only home players; … 6 only foreigners.
    pub nationality: u8,
    pub own_country: u8,
    pub abilities: u32,
    pub keeper_abilities: u32,
    pub traits: u32,
    pub contract_ending: bool,
    pub flags: u8,
    /// The goal bands ticked, bits 0 to 6.
    pub goals: u16,
    /// The least talent, 0 huge to 4 none.
    pub talent: i8,
}

/// A player found: his id, his club's country, league level and number.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hit {
    pub id: u16,
    pub country: u8,
    pub level: u8,
    pub number: u16,
}

/// The clubs a search goes through, in its order (`FUN_006e1813`).
fn clubs(records: &Records, (country, code): (u8, u8)) -> Vec<(u8, u16)> {
    let countries = &records.countries;
    let main = records.globals.main_country();
    let mut clubs = Vec::new();
    let whole = |k: u16, clubs: &mut Vec<(u8, u16)>| {
        let count = countries.team_count(usize::from(k)).max(0) as u16;
        for n in 0..count {
            if records.team(k, n).is_some_and(|t| t.level() < NO_LEAGUE) {
                clubs.push((k as u8, n));
            }
        }
    };
    let league = |c: i16, i: i16, clubs: &mut Vec<(u8, u16)>| {
        if !records.globals.played(c) || !(0..countries.leagues(c as usize)).contains(&i) {
            return;
        }
        if let Some(league) = crate::opponent::League::of(records, c as u16, i as u8) {
            for place in 0..league.teams() {
                clubs.push((c as u8, league.team(place) as u16));
            }
        }
    };
    let further = || {
        let more = usize::try_from(records.globals.more_count()).unwrap_or(0);
        records.globals.more_countries().into_iter().take(more).collect::<Vec<i16>>()
    };
    match (country, code) {
        (0, ALL) => (1..=LAST_COUNTRY)
            .filter(|&k| countries.kind(usize::from(k)) != 0)
            .for_each(|k| whole(k, &mut clubs)),
        (0, MAIN) => {
            (0..countries.leagues(main as usize)).for_each(|i| league(main, i, &mut clubs))
        }
        (0, MAIN_FIRST) => league(main, 0, &mut clubs),
        (0, MAIN_SECOND) => league(main, 1, &mut clubs),
        (0, ENGLAND_LOW) => (2..4).for_each(|i| league(ENGLAND, i, &mut clubs)),
        (0, REGIONAL) => {
            let last = if records.globals.start_year() < 2000 { 5 } else { 3 };
            (2..=last).for_each(|i| league(GERMANY, i, &mut clubs));
        }
        (0, BONUS) => further().into_iter().for_each(|c| league(c, 0, &mut clubs)),
        (0, ABROAD) => (1..=LAST_COUNTRY)
            .filter(|&k| countries.kind(usize::from(k)) == 2)
            .for_each(|k| whole(k, &mut clubs)),
        (0, _) => {}
        (c, code) if i16::from(c) == main && code < OWN_LEAGUES => {
            league(main, i16::from(code) - i16::from(MAIN_LEAGUE), &mut clubs)
        }
        (c, _) => league(c.into(), 0, &mut clubs),
    }
    clubs
}

/// The search (`FUN_006e1813`): every player of the clubs in squad order
/// that passes the test, at most a hundred, unsorted.
pub fn search(records: &Records, c: &Criteria, today: NaiveDate) -> Vec<Hit> {
    let mut ages = [c.ages == 0; AGE_LIMIT];
    for (bit, &(from, to)) in AGES.iter().enumerate() {
        if c.ages & (1 << bit) != 0 {
            ages[from..=to].fill(true);
        }
    }
    let mut goals = [c.goals == 0; GOAL_LIMIT];
    for (bit, &(from, to)) in GOALS.iter().enumerate() {
        if c.goals & (1 << bit) != 0 {
            goals[from..=to].fill(true);
        }
    }
    let pattern = c.name.to_ascii_uppercase();
    let mut hits = Vec::new();
    for (country, number) in clubs(records, c.league) {
        let Some(team) = records.team(country.into(), number) else { continue };
        for id in team.players() {
            if hits.len() == MOST_HITS {
                return hits;
            }
            let Some(r) = records.player(id) else { continue };
            if passes(records, r, id, c, &pattern, today, (&ages, &goals)) {
                hits.push(Hit { id, country, level: team.level(), number });
            }
        }
    }
    hits
}

/// The test, rejecting at the first criterion that fails
/// (`FUN_006e287e`).
fn passes(
    records: &Records,
    r: &Player,
    id: u16,
    c: &Criteria,
    pattern: &str,
    today: NaiveDate,
    (ages, goals): (&[bool; AGE_LIMIT], &[bool; GOAL_LIMIT]),
) -> bool {
    let main = r.positions()[0];
    if r.talent() > c.talent
        || (c.strengths != 0 && c.strengths & (1 << r.strength().min(15)) == 0)
        || (c.positions != 0 && c.positions & (1 << main.min(15)) == 0)
        || id > LAST_REAL_ID
    {
        return false;
    }
    let age = r.birthday().and_then(|b| today.years_since(b)).map_or(0, |y| y as usize);
    if age >= AGE_LIMIT || !ages[age] {
        return false;
    }
    let own = r.nation() == c.own_country;
    let home_grown = r.manner() & HOME_GROWN != 0;
    let rejected = match c.nationality {
        1 => !own,
        2 => !own && home_grown,
        3 => home_grown,
        4 => !home_grown && own,
        5 => !home_grown,
        6 => own,
        _ => false,
    };
    if rejected {
        return false;
    }
    let good = r.good() as i16 as i32 as u32;
    let abilities = if main == GOALKEEPER {
        (c.keeper_abilities != 0 && good & c.keeper_abilities == 0) || c.abilities != 0
    } else {
        (c.abilities != 0 && good & c.abilities == 0) || c.keeper_abilities != 0
    };
    let traits = r.traits()[0] as i16 as i32 as u32;
    if abilities || (c.traits != 0 && traits & c.traits == 0) {
        return false;
    }
    let year = i32::from(records.globals.start_year());
    let years = (i32::from(r.contract() & CONTRACT_YEAR) - year).max(0);
    if c.contract_ending && years >= 2 {
        return false;
    }
    if c.flags & FIXED_FEE != 0 && r.fixed_fee() == 0 {
        return false;
    }
    if c.flags & UNHAPPY_ONLY != 0 && crate::mood::content_level(crate::mood::parts(r)) != UNHAPPY {
        return false;
    }
    if c.flags & NATIONAL != 0 && r.kind_12_matches() == 0 {
        return false;
    }
    let scored = usize::from(r.goals()[0]);
    if records.globals.played(r.club()[0].into()) && scored < GOAL_LIMIT && !goals[scored] {
        return false;
    }
    if pattern.is_empty() {
        return true;
    }
    let upper = |s: &str| s.to_ascii_uppercase().chars().collect::<Vec<char>>();
    let pattern: Vec<char> = pattern.chars().collect();
    glob(&upper(records.name(r.last_name())), &pattern)
        || glob(&upper(&records.known_as(id).unwrap_or_default()), &pattern)
}

/// The name's match (`FUN_006e274c`): the whole name, `?` any one letter,
/// `*` any run — but a run must end in the letter after it.
fn glob(s: &[char], p: &[char]) -> bool {
    match p {
        [] => s.is_empty(),
        ['*'] => true,
        ['*', next, rest @ ..] => {
            (0..s.len()).any(|i| (s[i] == *next || *next == '?') && glob(&s[i + 1..], rest))
        }
        [first, rest @ ..] => {
            !s.is_empty() && (*first == '?' || *first == s[0]) && glob(&s[1..], rest)
        }
    }
}

/// A manager's request to NORACSA, "Bitte melden!" (the manager record's
/// +0x19cc, 0x2a8 bytes): a state, then the criteria laid out as the
/// search keeps them (C).
pub struct Wanted(pub Vec<u8>);

const REQUEST_AT: usize = 0x19cc;
const REQUEST: usize = 0x2a8;
const C: usize = 4;
/// The fields of C: the name, the bits of strength, age and position, the
/// league, the nationality, the own country, the abilities' and traits'
/// masks, the contract box, the talent.
pub mod field {
    pub const NAME: usize = 0;
    pub(super) const NAME_LEN: usize = 20;
    pub const STRENGTHS: usize = 0x1c;
    pub const AGES: usize = 0x1e;
    pub const POSITIONS: usize = 0x20;
    pub const LEAGUE: usize = 0x22;
    pub const NATIONALITY: usize = 0x24;
    pub const OWN_COUNTRY: usize = 0x26;
    pub const ABILITIES: usize = 0x28;
    pub const KEEPER_ABILITIES: usize = 0x2c;
    pub const TRAITS: usize = 0x30;
    pub const CONTRACT: usize = 0x38;
    pub const TALENT: usize = 0x42;
}
const HITS: usize = 0x2a0;
/// A request's states: none, stored, searched.
pub const STORED: i16 = 1;
const SEARCHED: i16 = 2;

impl Wanted {
    pub fn of(manager: &crate::records::Manager) -> Self {
        Self(
            manager
                .bytes()
                .get(REQUEST_AT..REQUEST_AT + REQUEST)
                .map_or_else(|| vec![0; REQUEST], <[u8]>::to_vec),
        )
    }

    pub fn store(&self, manager: &mut crate::records::Manager) {
        for (k, &b) in self.0.iter().enumerate() {
            manager.put(REQUEST_AT + k, b);
        }
    }

    pub fn state(&self) -> i16 {
        i16::from_le_bytes([self.0[0], self.0[1]])
    }

    pub fn set_state(&mut self, state: i16) {
        self.0[..2].copy_from_slice(&state.to_le_bytes());
    }

    /// A field of C, two or four bytes, or one.
    pub fn put_u16(&mut self, at: usize, v: u16) {
        self.0[C + at..C + at + 2].copy_from_slice(&v.to_le_bytes());
    }

    pub fn put_u32(&mut self, at: usize, v: u32) {
        self.0[C + at..C + at + 4].copy_from_slice(&v.to_le_bytes());
    }

    pub fn put_u8(&mut self, at: usize, v: u8) {
        self.0[C + at] = v;
    }

    pub fn u16_at(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.0[C + at], self.0[C + at + 1]])
    }

    pub fn u32_at(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.0[C + at..C + at + 4].try_into().unwrap())
    }

    /// The name, its 19 letters and an end (`strncpy(C, …, 0x13)`).
    pub fn set_name(&mut self, name: &str) {
        let name_field = &mut self.0[C + field::NAME..C + field::NAME + field::NAME_LEN];
        name_field.fill(0);
        let bytes: Vec<u8> = name.chars().filter_map(a3_files::bitmap_font::to_cp1252).collect();
        let n = bytes.len().min(field::NAME_LEN - 1);
        name_field[..n].copy_from_slice(&bytes[..n]);
    }
}

/// The requests at a week start (`FUN_006e1d1d`): a stored one is
/// searched — and as the game writes it, its abilities' masks reject
/// everybody, so it finds none — its name wiped; any other is done with.
pub fn age_requests(records: &mut Records) {
    let slots: Vec<usize> = (0..records.managers.len())
        .filter(|&slot| records.globals.slots().get(slot).is_some_and(|&s| s > 0))
        .collect();
    for slot in slots {
        let Some(manager) = records.managers.get_mut(slot) else { continue };
        let mut request = Wanted::of(manager);
        if request.state() == STORED {
            request.put_u8(field::NAME, 0);
            request.put_u16(HITS, 0);
            request.set_state(SEARCHED);
        } else {
            request.set_state(0);
        }
        request.store(manager);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use a3_files::an3::Save;

    use super::*;

    fn probe() -> Option<Records> {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let path = dir.join("saves/Probe1.an3");
        if !path.exists() {
            eprintln!("no Probe1 among the saves; the oracle's copy has it");
            return None;
        }
        Some(Records::from_save(Save::read(&path).unwrap()).unwrap())
    }

    #[test]
    fn a_name_matches_as_a_whole() {
        let chars = |s: &str| s.chars().collect::<Vec<_>>();
        assert!(glob(&chars("MUELLER"), &chars("M*R")));
        assert!(glob(&chars("MUELLER"), &chars("M?ELLER")));
        assert!(!glob(&chars("MUELLER"), &chars("MUEL")));
        assert!(glob(&chars("MUELLER"), &chars("*")));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_search_keeps_to_its_criteria() {
        let Some(records) = probe() else { return };
        let today = NaiveDate::from_ymd_opt(2025, 8, 11).unwrap();
        let all = Criteria { talent: 2, ..Criteria::default() };
        let hits = search(&records, &all, today);
        assert_eq!(hits.len(), MOST_HITS);
        assert!(hits.iter().all(|h| h.id <= LAST_REAL_ID));
        // Keepers of strength 5 only, in the main country's first league
        // in its record's order.
        let keepers = Criteria {
            strengths: 1 << 5,
            positions: 1 << GOALKEEPER,
            league: (0, MAIN_FIRST),
            ..all.clone()
        };
        let hits = search(&records, &keepers, today);
        assert!(!hits.is_empty());
        for h in &hits {
            let r = records.player(h.id).unwrap();
            assert_eq!((r.strength(), r.positions()[0], h.level), (5, GOALKEEPER, 0));
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_request_lives_a_week() {
        let Some(mut records) = probe() else { return };
        let slot = records.human_slot(0).unwrap();
        let mut request = Wanted::of(&records.managers[slot]);
        request.set_name("Kane");
        request.set_state(STORED);
        request.store(&mut records.managers[slot]);
        age_requests(&mut records);
        let request = Wanted::of(&records.managers[slot]);
        assert_eq!((request.state(), request.0[C + field::NAME]), (SEARCHED, 0));
        age_requests(&mut records);
        assert_eq!(Wanted::of(&records.managers[slot]).state(), 0);
    }
}
