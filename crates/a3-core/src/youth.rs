//! The youth: the list of youths without a pre-contract, whom other clubs
//! may poach, each club's youth team and its pool of amateurs, and their
//! way up into the squad (VERLAENGERUNG-ABWERBEN.md §3, JUGEND-AMATEURE.md
//! §1–3 in the RE repo).

use crate::data::Data;
use crate::records::{Player, Records};
use crate::season::ClubState;

/// The youth list (`0xb77338`, its entries from +0x20): 50 slots of 8
/// bytes — the player, his club's country and number, the day he was
/// entered; a free slot's player is 0.
const LIST_AT: u32 = 0xb77338;
const LIST_SIZE: usize = 50;
const ENTRY: usize = 8;

/// A club's youth team (`0xb71b48[i]`, from the object's +0x20): its
/// youths by class (C, B, A), those the club made, their count, those
/// moved up into the squad, their ids.
const TEAM_AT: u32 = 0xb71b48;
const CLASSES: usize = 0;
const COUNT: usize = 7;
const MOVED_UP: usize = 8;
const IDS: usize = 0xa;
/// The most a youth team holds (`FUN_0066ae46`).
const MOST: usize = 20;
/// A club signs no youth once it has this many (`FUN_004d7940`).
pub const FULL: usize = 15;

/// The youth classes by age: C up to 14, B up to 16, A older, counted in
/// the season's year from the first of July (`FUN_0066b0c1`).
const C_YOUTH_AGE: i32 = 15;
const B_YOUTH_AGE: i32 = 17;
const SEASON_MONTH: u8 = 7;

/// A club's amateur pool (`0x8dd4c8[i]`, from +0x20): five who moved up,
/// then five amateurs.
const POOL_AT: u32 = 0x8dd4c8;
const POOL: usize = 5;

/// The youth talks booked (`0x1c53b88`, from +0x20): a count, then
/// entries of 10 bytes from +2, the player first (`FUN_00763b0c`).
const YOUTH_TALKS_AT: u32 = 0x1c53b88;
const YOUTH_TALK: usize = 10;

/// R+0x352: his goals as a youth or an amateur of the pool; R+0x1b4 bit
/// 30: the youth his age group supports ("Förd.").
const YOUTH_GOALS: usize = 0x352;
pub const SUPPORTED: u32 = 0x4000_0000;
/// On trial with the professionals: R+0x271's low four bits, and R+0x26f's
/// bit for a youth or an amateur (`FUN_007069eb`, `FUN_0070699b`).
const TRIAL_MASK: u8 = 0x0f;
const TRIAL: u8 = 8;
const YOUTH_TRIAL: u8 = 0x10;
const AMATEUR_TRIAL: u8 = 0x20;
/// A pre-contract runs ten years (`FUN_004d86a3`).
const PRE_CONTRACT_YEARS: u16 = 10;
const CONTRACT_YEAR: u16 = 0xfff;
/// Shirt numbers run from 1 to 98 (`FUN_00407abb`).
const SHIRTS: std::ops::Range<u8> = 1..99;

/// A career station: 0x14 bytes from the record's +0x20 — begun, ended,
/// fee, goals and appearances, club, the strength with a youth team's bit.
const CAREER_AT: usize = 0x20;
const STATION: usize = 0x14;
const MOST_STATIONS: i16 = 15;
const YOUTH_STATION: u8 = 0x40;

/// The alerts of youth poaching: "Der Spieler kann nicht mit Ihnen
/// verhandeln!", "Der Jugendspieler möchte nicht zu Ihnen wechseln!".
pub const CANNOT_TALK: u32 = 0x882f2c;
pub const WONT_MOVE: u32 = 0x882f88;
pub const MESSAGES: [u32; 2] = [CANNOT_TALK, WONT_MOVE];

/// An entry of the youth list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prospect {
    pub slot: usize,
    pub id: u16,
    pub country: u8,
    pub number: i16,
    /// The day he was entered.
    pub day: i16,
}

fn word(b: &[u8], at: usize) -> i16 {
    b.get(at..at + 2).map_or(0, |w| i16::from_le_bytes([w[0], w[1]]))
}

/// The youth list's entries, free slots left out.
pub fn prospects(records: &Records) -> Vec<Prospect> {
    let Some(block) = records.block(&format!("{LIST_AT:#x}")) else { return Vec::new() };
    (0..LIST_SIZE)
        .filter_map(|slot| {
            let at = slot * ENTRY;
            let id = word(block, at) as u16;
            (id != 0).then(|| Prospect {
                slot,
                id,
                country: block.get(at + 2).copied().unwrap_or(0),
                number: word(block, at + 4),
                day: word(block, at + 6),
            })
        })
        .collect()
}

/// A slot's player, if it holds one.
pub fn prospect_in(records: &Records, slot: usize) -> Option<Prospect> {
    prospects(records).into_iter().find(|p| p.slot == slot)
}

/// Takes him off the list, from every slot (`FUN_0076408d`).
pub fn drop_prospect(records: &mut Records, id: u16) {
    let Some(block) = records.block_mut(&format!("{LIST_AT:#x}")) else { return };
    for slot in 0..LIST_SIZE {
        if word(block, slot * ENTRY) as u16 == id {
            block[slot * ENTRY..slot * ENTRY + 2].fill(0);
        }
    }
}

fn team_block(country: u8, number: u16, records: &Records) -> Option<String> {
    let index = records.team_index(country.into(), number)?;
    Some(format!("{TEAM_AT:#x}[{index}]"))
}

/// A club's youths, and those moved up, who count against its limit.
pub fn youths(records: &Records, country: u8, number: u16) -> (Vec<u16>, usize) {
    let Some(block) = team_block(country, number, records).and_then(|b| records.block(&b)) else {
        return (Vec::new(), 0);
    };
    let count = usize::from(block.get(COUNT).copied().unwrap_or(0));
    let ids = (0..count).map(|k| word(block, IDS + 2 * k) as u16).collect();
    (ids, usize::try_from(word(block, MOVED_UP)).unwrap_or(0))
}

/// His youth class by his birthday: 0 C, 1 B, 2 A (`FUN_0066b0c1`).
pub fn class(records: &Records, id: u16) -> usize {
    let Some(r) = records.player(id) else { return 0 };
    let mut age = i32::from(records.globals.start_year()) - i32::from(r.birth_year());
    if r.birth_month() < SEASON_MONTH {
        age += 1;
    }
    match age {
        ..C_YOUTH_AGE => 0,
        C_YOUTH_AGE..B_YOUTH_AGE => 1,
        _ => 2,
    }
}

/// Stores a club's youths: the classes counted anew (`FUN_0063937e`).
fn store(records: &mut Records, country: u8, number: u16, ids: &[u16]) {
    let classes = ids.iter().fold([0i16; 3], |mut c, &id| {
        c[class(records, id)] += 1;
        c
    });
    let Some(name) = team_block(country, number, records) else { return };
    let Some(block) = records.block_mut(&name) else { return };
    for (k, n) in classes.into_iter().enumerate() {
        block[CLASSES + 2 * k..CLASSES + 2 * k + 2].copy_from_slice(&n.to_le_bytes());
    }
    block[COUNT] = ids.len() as u8;
    for (k, id) in ids.iter().enumerate() {
        if let Some(w) = block.get_mut(IDS + 2 * k..IDS + 2 * k + 2) {
            w.copy_from_slice(&id.to_le_bytes());
        }
    }
}

/// What a youth's parents agree to a year for a club of a league level
/// (`FUN_0066b648`): the less the higher the league.
pub fn salary(id: u16, age: i32, level: u8) -> i32 {
    let less = 3 - i32::from(level.min(2));
    less * (i32::from(id % 3) * 1000 + 2000 * less + (age - 12) * 1000)
}

/// Signs a youth of another club for a club, on a salary, no fee paid
/// (`CJugendspielerAbwerben` 0x4d76d9): out of his club's youth team, into
/// the club's, off the list, his career's station closed and a new one
/// begun today.
pub fn sign(records: &mut Records, id: u16, (country, number): (u8, u16), salary: i32) {
    let Some([from_country, from_number]) = records.player(id).map(|r| r.club()) else { return };
    leave_youth(records, (from_country, from_number.into()), id);
    let (mut to, _) = youths(records, country, number);
    if to.len() < MOST {
        to.push(id);
        if let Some(r) = records.players.get_mut(usize::from(id)) {
            r.set_club([country, number as u8]);
        }
    }
    drop_prospect(records, id);
    let date = records.globals.date();
    if let Some(r) = records.players.get_mut(usize::from(id)) {
        r.set_salary((salary / 1000) as u16);
        let [day, month, year] = date;
        let today = [day as u8, month as u8];
        let mut count = r.career_count();
        if count > 0 {
            let at = CAREER_AT + (count as usize - 1) * STATION;
            r.put(at + 4, today);
            r.put(at + 6, year);
        }
        // A full career drops its first station (`FUN_00704109`).
        if count == MOST_STATIONS {
            for k in 1..count as usize {
                let station: [u8; STATION] = r.get(CAREER_AT + k * STATION);
                r.put(CAREER_AT + (k - 1) * STATION, station);
            }
            count -= 1;
        }
        let at = CAREER_AT + count as usize * STATION;
        r.put(at, today);
        r.put(at + 2, year);
        r.put(at + 4, [0u8; 4]);
        r.put(at + 8, 0u32);
        r.put(at + 0xc, [0u16; 2]);
        r.put(at + 0x10, [country, number as u8, 0, r.strength() + YOUTH_STATION]);
        r.set_career_count(count + 1);
    }
    store(records, country, number, &to);
}

/// Out of a club's youth team, his youth goals gone (`FUN_0066aea3`).
fn leave_youth(records: &mut Records, (country, number): (u8, u16), id: u16) {
    let (mut ids, _) = youths(records, country, number);
    ids.retain(|&y| y != id);
    store(records, country, number, &ids);
    if let Some(r) = records.players.get_mut(usize::from(id)) {
        r.put(YOUTH_GOALS, 0u8);
    }
}

/// His goals as a youth or an amateur of the pool.
pub fn goals(r: &Player) -> u8 {
    r.get(YOUTH_GOALS)
}

/// "Förd." for a youth: off if he has it, else his age group's moves to
/// him (`OnUserC000_004d905b`).
pub fn support(records: &mut Records, club: (u8, u16), id: u16, group: usize) {
    if records.player(id).is_some_and(|r| r.shown() & SUPPORTED != 0) {
        if let Some(r) = records.players.get_mut(usize::from(id)) {
            r.set_shown(r.shown() & !SUPPORTED);
        }
        return;
    }
    let (ids, _) = youths(records, club.0, club.1);
    let mates: Vec<u16> = ids.iter().copied().filter(|&y| class(records, y) == group).collect();
    for y in mates {
        if let Some(r) = records.players.get_mut(usize::from(y)) {
            r.set_shown(r.shown() & !SUPPORTED);
        }
    }
    if let Some(r) = records.players.get_mut(usize::from(id)) {
        r.set_shown(r.shown() | SUPPORTED);
    }
    store(records, club.0, club.1, &ids);
}

/// A youth's pre-contract, on a salary for ten years: off the youth list
/// (`FUN_004d86a3`).
pub fn pre_contract(records: &mut Records, id: u16, salary: i32) {
    drop_prospect(records, id);
    let year = records.globals.start_year();
    if let Some(r) = records.players.get_mut(usize::from(id)) {
        r.set_salary((salary / 1000) as u16);
        r.set_contract(r.contract() & !CONTRACT_YEAR | (year + PRE_CONTRACT_YEARS) & CONTRACT_YEAR);
    }
}

/// Whether a youth talk with him is booked (`FUN_00763b0c`).
pub fn youth_talk_booked(records: &Records, id: u16) -> bool {
    records.block(&format!("{YOUTH_TALKS_AT:#x}")).is_some_and(|b| {
        let count = usize::try_from(word(b, 0)).unwrap_or(0);
        (0..count).any(|k| word(b, 2 + k * YOUTH_TALK) as u16 == id)
    })
}

/// The professional-contract talks with the youth (`0x1c53ba8`) and the
/// amateurs (`0x1c53a68`, block from 0x20 before): a count, then up to 28
/// entries of 10 bytes — the player, the club's country and number, the
/// day, 0 (`FUN_00763937`); and the planner's kinds they are booked as.
const AMATEUR_TALKS_AT: u32 = 0x1c53a48;
const TALKS_MOST: usize = 0x1c;
pub const YOUTH_CONTRACT_TALK: u8 = 0x0f;
pub const AMATEUR_CONTRACT_TALK: u8 = 0x10;

/// Whether a contract talk with him is booked among the amateurs', which
/// "Profivertrag geben" asks for youths too (`FUN_00763cca`).
pub fn amateur_talk_booked(records: &Records, id: u16) -> bool {
    records.block(&format!("{AMATEUR_TALKS_AT:#x}")).is_some_and(|b| {
        let count = usize::try_from(word(b, 0)).unwrap_or(0);
        (0..count).any(|k| word(b, 2 + k * YOUTH_TALK) as u16 == id)
    })
}

/// A contract talk on `day`, into the youth's list or the amateurs'
/// (`FUN_00765a59`, `FUN_007660ca`); false when the list is full.
pub fn book_contract_talk(
    records: &mut Records,
    youth: bool,
    id: u16,
    (country, number): (u8, u16),
    day: i16,
) -> bool {
    let at = if youth { YOUTH_TALKS_AT } else { AMATEUR_TALKS_AT };
    let Some(block) = records.block_mut(&format!("{at:#x}")) else { return false };
    let count = usize::try_from(word(block, 0)).unwrap_or(0);
    let entry = 2 + count * YOUTH_TALK;
    if count >= TALKS_MOST || block.len() < entry + YOUTH_TALK {
        return false;
    }
    let mut put = |at: usize, value: u16| block[at..at + 2].copy_from_slice(&value.to_le_bytes());
    put(entry, id);
    put(entry + 4, number);
    put(entry + 6, day as u16);
    put(entry + 8, 0);
    put(0, count as u16 + 1);
    // The country a byte of its own; the one after it stays as it was.
    block[entry + 2] = country;
    true
}

/// A club's amateur pool: those moved up, the amateurs (`FUN_00407ecd`).
pub fn pool(records: &Records, country: u8, number: u16) -> [Vec<u16>; 2] {
    let Some(block) = records
        .team_index(country.into(), number)
        .and_then(|i| records.block(&format!("{POOL_AT:#x}[{i}]")))
    else {
        return Default::default();
    };
    let list = |from: usize| {
        (0..POOL)
            .map(|k| word(block, from + 2 * k))
            .filter(|&id| id > 0)
            .map(|id| id as u16)
            .collect()
    };
    [list(0), list(2 * POOL)]
}

/// Stores a club's pool (`FUN_00408036`).
fn store_pool(records: &mut Records, country: u8, number: u16, lists: &[Vec<u16>; 2]) {
    let Some(i) = records.team_index(country.into(), number) else { return };
    let Some(block) = records.block_mut(&format!("{POOL_AT:#x}[{i}]")) else { return };
    for (n, list) in lists.iter().enumerate() {
        for k in 0..POOL {
            let id = list.get(k).copied().unwrap_or(0);
            let at = (n * POOL + k) * 2;
            block[at..at + 2].copy_from_slice(&id.to_le_bytes());
        }
    }
}

/// A shirt number for him unless he has one: the first no one of the
/// squad wears (`FUN_00407abb`).
pub fn give_shirt(records: &mut Records, squad: &[u16], id: u16) {
    if records.player(id).is_none_or(|r| r.shirt() != 0) {
        return;
    }
    let worn: Vec<u8> =
        squad.iter().filter_map(|&p| records.player(p)).map(|r| r.shirt()).collect();
    if let Some(free) = SHIRTS.clone().find(|n| !worn.contains(n))
        && let Some(r) = records.players.get_mut(usize::from(id))
    {
        r.set_shirt(free);
    }
}

/// Where the trial came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trial {
    Youth,
    Amateur,
}

/// A youth or an amateur of the pool joins his club's squad on trial: after
/// its last player of his main position or one before
/// (`FUN_0070df0b`), out of the youth team or the pool (`FUN_004d88ec`,
/// `CAmateure` 0x4410fe). `key` is the club's place in the season.
pub fn move_up(
    records: &mut Records,
    data: &Data,
    club: &mut ClubState,
    (key, (country, number)): ((usize, usize), (u8, u16)),
    id: u16,
    trial: Trial,
) {
    let Some(main) = records.player(id).map(|r| r.positions()[0]) else { return };
    let Some(mut squad) = records.team(country.into(), number).map(|t| t.players()) else {
        return;
    };
    let place = squad
        .iter()
        .rposition(|&p| records.player(p).is_some_and(|r| r.positions()[0] <= main))
        .map_or(0, |k| k + 1);
    squad.insert(place, id);
    if let Some(team) = records.team_mut(country.into(), number) {
        team.set_players(&squad);
    }
    if let Some(r) = records.players.get_mut(usize::from(id)) {
        r.set_club([country, number as u8]);
        r.set_picks(r.picks() & !TRIAL_MASK | TRIAL);
        let bit = if trial == Trial::Youth { YOUTH_TRIAL } else { AMATEUR_TRIAL };
        r.set_retiring(r.retiring() | bit);
        if trial == Trial::Youth {
            r.set_shown(r.shown() & !SUPPORTED);
        }
    }
    match trial {
        Trial::Youth => {
            leave_youth(records, (country, number), id);
            if let Some(block) =
                team_block(country, number, records).and_then(|name| records.block_mut(&name))
            {
                let moved = word(block, MOVED_UP).wrapping_add(1);
                block[MOVED_UP..MOVED_UP + 2].copy_from_slice(&moved.to_le_bytes());
            }
        }
        // Out of the amateurs, among those moved up (`FUN_00407c2c`).
        Trial::Amateur => {
            let [mut up, mut amateurs] = pool(records, country, number);
            amateurs.retain(|&a| a != id);
            if !up.contains(&id) && up.len() < POOL {
                up.push(id);
            }
            store_pool(records, country, number, &[up, amateurs]);
        }
    }
    if let Some(joining) = crate::load::any_player(records, data, id) {
        club.insert(key, place, joining);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use a3_files::an3::Save;

    use super::*;

    #[test]
    fn parents_ask_less_the_higher_the_league() {
        // Draghici (323) and Moumouni (349), both 17, for a first-league
        // club; Moumouni for a club in the third league or lower.
        assert_eq!(salary(323, 17, 0), 39_000);
        assert_eq!(salary(349, 17, 0), 36_000);
        assert_eq!(salary(349, 17, 5), 8_000);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_signed_youth_changes_teams_and_begins_a_station() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let path = dir.join("saves/Probe1.an3");
        if !path.exists() {
            eprintln!("no Probe1 among the saves; the oracle's copy has it");
            return;
        }
        let mut records = Records::from_save(Save::read(&path).unwrap()).unwrap();
        let manager = records.humans().next().unwrap();
        let (country, number) = (manager.country() as u8, manager.team());
        let from = (0..20u16)
            .filter(|&n| n != number)
            .find(|&n| !youths(&records, country, n).0.is_empty())
            .unwrap();
        let id = youths(&records, country, from).0[0];
        // On the list first.
        let list = records.block_mut(&format!("{LIST_AT:#x}")).unwrap();
        list[..8].copy_from_slice(&[id as u8, (id >> 8) as u8, country, 0, from as u8, 0, 0, 0]);
        assert_eq!(prospect_in(&records, 0).map(|p| p.id), Some(id));
        let stations = records.player(id).unwrap().career_count();
        let (before, _) = youths(&records, country, number);
        sign(&mut records, id, (country, number), 33_000);
        assert!(!youths(&records, country, from).0.contains(&id));
        let (after, _) = youths(&records, country, number);
        assert_eq!(after.len(), before.len() + 1);
        assert_eq!(after.last(), Some(&id));
        assert!(prospects(&records).iter().all(|p| p.id != id));
        let r = records.player(id).unwrap();
        assert_eq!(r.club(), [country, number as u8]);
        assert_eq!(r.salary(), 33);
        assert_eq!(r.career_count(), (stations + 1).min(MOST_STATIONS));
        let last = CAREER_AT + (r.career_count() as usize - 1) * STATION;
        let station: [u8; STATION] = r.get(last);
        assert_eq!(station[0x13], r.strength() + YOUTH_STATION);
        assert_eq!(station[0x10..0x12], [country, number as u8]);
        // The classes counted anew add up to the count.
        let name = team_block(country, number, &records).unwrap();
        let block = records.block(&name).unwrap();
        let classes: i16 = (0..3).map(|k| word(block, CLASSES + 2 * k)).sum();
        assert_eq!(classes as usize, after.len());
    }
}
