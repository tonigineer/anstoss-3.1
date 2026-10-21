//! The youth: the list of youths without a pre-contract, whom other clubs
//! may poach, and each club's youth team (VERLAENGERUNG-ABWERBEN.md §3 in
//! the RE repo).

use crate::records::Records;

/// The youth list (`0xb77338`, its entries from +0x20): 50 slots of 8
/// bytes — the player, his club's country and number, the day he was
/// entered; a free slot's player is 0.
const LIST_AT: u32 = 0xb77338;
const LIST_SIZE: usize = 50;
const ENTRY: usize = 8;

/// A club's youth team (`0xb71b48[i]`, from the object's +0x20): its
/// youths by class (C, B, A), those the club made, their count, a second
/// count, their ids.
const TEAM_AT: u32 = 0xb71b48;
const CLASSES: usize = 0;
const COUNT: usize = 7;
const SECOND_COUNT: usize = 8;
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

/// A club's youths, and the second count that adds to them.
pub fn youths(records: &Records, country: u8, number: u16) -> (Vec<u16>, usize) {
    let Some(block) = team_block(country, number, records).and_then(|b| records.block(&b)) else {
        return (Vec::new(), 0);
    };
    let count = usize::from(block.get(COUNT).copied().unwrap_or(0));
    let ids = (0..count).map(|k| word(block, IDS + 2 * k) as u16).collect();
    (ids, usize::try_from(word(block, SECOND_COUNT)).unwrap_or(0))
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
    let (mut from, _) = youths(records, from_country, from_number.into());
    from.retain(|&y| y != id);
    store(records, from_country, from_number.into(), &from);
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
