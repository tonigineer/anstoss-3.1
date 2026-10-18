//! A human club's year calendar (JAHRESKALENDER.md in the RE repo): an
//! entry for every day of the season — training camps, tournaments,
//! friendlies, matches, parties and events — which the calendar screen
//! shows a month at a time, books by dragging, and changes by a click.

use std::io;

use a3_files::exe::Exe;

use crate::records::{PLANNER_DAYS, PLANNER_ENTRY, Planner};

/// The kinds of days.
pub const CAMP: u8 = 1;
pub const TOURNAMENT: u8 = 2;
pub const INDOOR_TOURNAMENT: u8 = 3;
pub const BEACH_TOURNAMENT: u8 = 4;
pub const FRIENDLY: u8 = 5;
pub const MATCH: u8 = 6;
pub const PARTY: u8 = 7;
pub const EVENT: u8 = 8;
/// The last day anything may be booked on.
pub const LAST_BOOKING: usize = 336;

/// A match entry's fields: its competition code, both teams (country and
/// number), the goals, whether the result is known.
const MATCH_CODE: usize = 4;
const MATCH_HOME: usize = 8;
const MATCH_AWAY: usize = 0xc;
const MATCH_GOALS: usize = 0x10;
const MATCH_FLAGS: usize = 0x14;
const RESULT_KNOWN: u8 = 0x20;
/// A camp marker's +4: 1 its first day, 0 its last; +6 its place.
const CAMP_START: usize = 4;
const CAMP_PLACE: usize = 6;

const KINDS_AT: u32 = 0x8a8140;
const KIND_SIZE: usize = 5;
const KINDS: usize = 12;
/// A kind's columns in the table: shown, days needed, allowed in a camp,
/// deleted by a click.
const SHOWN: usize = 0;
const DAYS_NEEDED: usize = 2;
const IN_CAMP: usize = 3;
const DELETABLE: usize = 4;
const EVENTS_AT: u32 = 0x882b4c;
const EVENTS: usize = 7;
const PARTIES_AT: u32 = 0x882b5c;
const PARTIES: usize = 6;
/// The camps' places: 26 records, the name's text at +0x36.
const PLACES_AT: u32 = 0x8ab318;
const PLACES: u32 = 26;
const PLACE_SIZE: u32 = 0x38;
const PLACE_NAME: u32 = 0x36;

/// A day of the calendar, as stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Day([u8; PLANNER_ENTRY]);

impl Day {
    pub fn kind(&self) -> u8 {
        self.0[0]
    }

    fn word(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.0[at], self.0[at + 1]])
    }

    /// A party's or an event's value, a camp marker's start flag.
    pub fn value(&self) -> i16 {
        self.word(4)
    }

    /// A camp's place, from 1 (0 not known).
    pub fn place(&self) -> i16 {
        self.word(CAMP_PLACE)
    }

    pub fn camp_start(&self) -> bool {
        self.kind() == CAMP && self.word(CAMP_START) != 0
    }

    /// A tournament's or friendly's club: country and number.
    pub fn club(&self) -> (u8, i16) {
        (self.0[4], self.word(6))
    }

    /// A match's competition code, teams, and result if known.
    pub fn match_code(&self) -> u32 {
        u32::from_le_bytes(self.0[MATCH_CODE..MATCH_CODE + 4].try_into().unwrap_or_default())
    }

    pub fn teams(&self) -> [(u8, i16); 2] {
        [
            (self.0[MATCH_HOME], self.word(MATCH_HOME + 2)),
            (self.0[MATCH_AWAY], self.word(MATCH_AWAY + 2)),
        ]
    }

    pub fn result(&self) -> Option<(u8, u8)> {
        (self.0[MATCH_FLAGS] & RESULT_KNOWN != 0)
            .then_some((self.0[MATCH_GOALS], self.0[MATCH_GOALS + 1]))
    }
}

/// The game's tables of the calendar.
pub struct YearTables {
    kinds: Vec<u8>,
    /// The events' and parties' texts, by their values.
    pub event_texts: Vec<u16>,
    pub party_texts: Vec<u16>,
    /// The camps' places' names, from place 1.
    pub place_texts: Vec<u16>,
}

impl YearTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        Ok(Self {
            kinds: exe.bytes(KINDS_AT, KINDS * KIND_SIZE)?.to_vec(),
            event_texts: exe.u16s(EVENTS_AT, EVENTS)?,
            party_texts: exe.u16s(PARTIES_AT, PARTIES)?,
            place_texts: (0..PLACES)
                .map(|p| Ok(exe.u16s(PLACES_AT + p * PLACE_SIZE + PLACE_NAME, 1)?[0]))
                .collect::<io::Result<_>>()?,
        })
    }

    fn of(&self, kind: u8, column: usize) -> u8 {
        self.kinds.get(usize::from(kind) * KIND_SIZE + column).copied().unwrap_or(0)
    }

    /// Whether a day's kind shows in the calendar (`FUN_0072ef9e`).
    pub fn shown(&self, kind: u8) -> bool {
        self.of(kind, SHOWN) != 0
    }

    /// Whether a click deletes it: parties and events (`FUN_0072daf8`).
    pub fn deletable(&self, kind: u8) -> bool {
        self.of(kind, DELETABLE) != 0
    }

    /// Whether the days a kind needs from a day on are all free
    /// (`FUN_0072e917`).
    pub fn free(&self, calendar: &Planner, day: usize, kind: u8) -> bool {
        let days = usize::from(self.of(kind, DAYS_NEEDED));
        (day..day + days).all(|d| d < PLANNER_DAYS && calendar.day(d).kind == 0)
    }

    /// Whether a kind may be booked during a camp: friendlies only
    /// (`FUN_0072f552`).
    pub fn allowed_in_camp(&self, kind: u8) -> bool {
        self.of(kind, IN_CAMP) != 0
    }
}

pub fn day(calendar: &Planner, day: usize) -> Day {
    Day(calendar.entry(day))
}

/// Whether a day lies in a training camp: a camp marker, or after the
/// last one marked as starting one (`FUN_0072f096`).
pub fn in_camp(calendar: &Planner, d: usize) -> bool {
    if day(calendar, d).kind() == CAMP {
        return true;
    }
    (0..d)
        .rev()
        .find(|&e| day(calendar, e).kind() == CAMP)
        .is_some_and(|e| day(calendar, e).camp_start())
}

/// A party or an event booked on a day, what else its entry held cleared
/// (`FUN_0072ede4`).
pub fn book(calendar: &mut Planner, d: usize, kind: u8, value: i16) {
    let mut entry = [0; PLANNER_ENTRY];
    entry[0] = kind;
    entry[1] = 1;
    entry[4..6].copy_from_slice(&value.to_le_bytes());
    calendar.set_entry(d, entry);
}

/// A day's entry gone (`FUN_0072dc32`).
pub fn clear(calendar: &mut Planner, d: usize) {
    calendar.set_entry(d, [0; PLANNER_ENTRY]);
}

/// The last camp's end moved to a day after today, if that may be: there
/// is a camp starting before it, not over before today, and nothing but
/// it from its start to the day (`FUN_0072f235`, `FUN_0072f191`,
/// `FUN_0072f39a`). Whether anything moved.
pub fn move_camp_end(calendar: &mut Planner, d: usize, today: usize) -> bool {
    if d <= today || d >= PLANNER_DAYS {
        return false;
    }
    let Some(start) = (0..d).rev().find(|&e| day(calendar, e).camp_start()) else {
        return false;
    };
    let over = start < today
        && (start + 1..today).any(|e| {
            let entry = day(calendar, e);
            entry.kind() == CAMP && !entry.camp_start()
        });
    if over {
        return false;
    }
    if (start..=d).any(|e| !matches!(day(calendar, e).kind(), 0 | CAMP)) {
        return false;
    }
    if day(calendar, d).kind() == CAMP {
        return false;
    }
    let end = |e: usize, calendar: &Planner| {
        let entry = day(calendar, e);
        entry.kind() == CAMP && !entry.camp_start()
    };
    if in_camp(calendar, d) {
        // Inside the camp: the end comes forward to the day.
        let Some(old) = (d + 1..PLANNER_DAYS).find(|&e| day(calendar, e).kind() == CAMP) else {
            return false;
        };
        if !end(old, calendar) {
            return false;
        }
        let mut entry = calendar.entry(d);
        clear(calendar, old);
        entry[0] = CAMP;
        calendar.set_entry(d, entry);
    } else {
        // After it: the end goes on to the day.
        let Some(old) = (1..d).rev().find(|&e| day(calendar, e).kind() == CAMP) else {
            return false;
        };
        if !end(old, calendar) {
            return false;
        }
        clear(calendar, old);
        let mut entry = [0; PLANNER_ENTRY];
        entry[0] = CAMP;
        calendar.set_entry(d, entry);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calendar() -> Planner {
        Planner::new(&vec![0; Planner::SIZE]).unwrap()
    }

    fn camp(calendar: &mut Planner, start: usize, end: usize) {
        let mut first = [0; PLANNER_ENTRY];
        first[0] = CAMP;
        first[1] = 1;
        first[4] = 1;
        first[6] = 7;
        calendar.set_entry(start, first);
        let mut last = [0; PLANNER_ENTRY];
        last[0] = CAMP;
        last[1] = 1;
        calendar.set_entry(end, last);
    }

    #[test]
    fn a_camp_ends_later_or_sooner() {
        let mut c = calendar();
        camp(&mut c, 70, 71);
        assert!(in_camp(&c, 70) && in_camp(&c, 71) && !in_camp(&c, 72));
        assert!(move_camp_end(&mut c, 75, 65));
        assert_eq!(day(&c, 71).kind(), 0);
        assert!(in_camp(&c, 73) && day(&c, 75).kind() == CAMP && !in_camp(&c, 76));
        assert!(move_camp_end(&mut c, 72, 65));
        assert!(!in_camp(&c, 73) && day(&c, 72).kind() == CAMP);
        // Not on today or before.
        assert!(!move_camp_end(&mut c, 65, 65));
    }

    #[test]
    fn a_party_is_booked_and_cleared() {
        let mut c = calendar();
        book(&mut c, 80, PARTY, 2);
        assert_eq!((day(&c, 80).kind(), day(&c, 80).value()), (PARTY, 2));
        clear(&mut c, 80);
        assert_eq!(day(&c, 80).kind(), 0);
    }
}
