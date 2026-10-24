//! The stock market and what the managers hold of their own club
//! (VEREINSFUEHRUNG.md §2.6): the shares a club has out, the portfolios'
//! places, and the majority which carries a manager's confidence to 100
//! (`FUN_00405e95`, `FUN_0040689a`, `FUN_00406b4f`, `FUN_007701b2`).

use crate::records::Records;

/// The stock market object (`0x8be408`, its data from +0x20): an i32
/// count and entries of 0x108 from +0x14.
const MARKET_AT: u32 = 0x8be408;
const MARKET_COUNT: usize = 0;
const MARKET_FIRST: usize = 0x14;
const MARKET_ENTRY: usize = 0x108;
const CLUB_COUNTRY: usize = 0x1c;
const CLUB_NUMBER: usize = 0x1e;
const CLUB_SHARES: usize = 0x20;
/// The managers' portfolios (`0x8dc6e0[slot − 1]`): fifty places each,
/// a used flag, the market entry held and how many shares.
const PORT_AT: u32 = 0x8dc6e0;
const PORT_SLOTS: usize = 50;
const PORT_USED: usize = 0;
const PORT_ENTRY: usize = 0x34;
const PORT_SHARES: usize = 0x3c;
const PORT_STEP: usize = 0x10;

fn word(b: &[u8], at: usize) -> i16 {
    match (b.get(at), b.get(at + 1)) {
        (Some(lo), Some(hi)) => i16::from_le_bytes([*lo, *hi]),
        _ => 0,
    }
}

fn long(b: &[u8], at: usize) -> i32 {
    match b.get(at..at + 4) {
        Some(four) => i32::from_le_bytes(four.try_into().unwrap()),
        _ => 0,
    }
}

/// The club's place on the market and its shares in all
/// (`FUN_00405e95`).
fn listing(records: &Records, country: u8, number: i16) -> Option<(usize, i32)> {
    let b = records.block(&format!("{MARKET_AT:#x}"))?;
    let count = word(b, MARKET_COUNT).max(0) as usize;
    (0..count).find_map(|k| {
        let e = MARKET_FIRST + k * MARKET_ENTRY;
        if word(b, e + CLUB_COUNTRY) != i16::from(country) || word(b, e + CLUB_NUMBER) != number {
            return None;
        }
        Some((k, long(b, e + CLUB_SHARES)))
    })
}

/// What the manager in this slot holds of that market place
/// (`FUN_0040689a`).
fn holding(records: &Records, slot: i16, place: usize) -> i32 {
    let Some(b) = records.block(&format!("{PORT_AT:#x}[{}]", slot - 1)) else {
        return 0;
    };
    (0..PORT_SLOTS).fold(0, |sum, k| {
        let used = b.get(PORT_USED + k).is_some_and(|&u| u != 0);
        let at = PORT_ENTRY + k * PORT_STEP;
        if !used || usize::try_from(long(b, at)).unwrap_or(usize::MAX) != place {
            return sum;
        }
        sum + long(b, at + (PORT_SHARES - PORT_ENTRY))
    })
}

/// The shares as the board screen shows them (§2.6): the shares in all,
/// the manager's own, and the other humans' by their place among the
/// managers. Nothing when the club is not on the market.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Split {
    pub total: i32,
    pub own: i32,
    pub others: Vec<(usize, i32)>,
}

/// The manager's place among the humans, 1 to 4, from the slots the
/// globals keep; 0 for a manager not in play. His portfolio of shares
/// stands at `0x8dc6e0[place − 1]`.
pub fn place(records: &Records, index: usize) -> i16 {
    records.globals.slots().get(index).copied().unwrap_or(0)
}

/// The club's shares among the humans (`FUN_005b548f`'s reading): the
/// manager's own holding and every other human's, in the order the
/// board screen's rows take them.
pub fn split(records: &Records, slot: i16, country: u8, number: i16) -> Option<Split> {
    let (place, total) = listing(records, country, number)?;
    if total <= 0 {
        return None;
    }
    let others = records
        .globals
        .slots()
        .iter()
        .enumerate()
        .filter(|(_, s)| **s > 0 && **s != slot)
        .map(|(i, &s)| (i, holding(records, s, place)))
        .collect();
    Some(Split { total, own: holding(records, slot, place), others })
}

/// Whether the manager at this place among the managers holds the
/// majority of his own club's shares (`FUN_00406b4f`): the club is on
/// the market and he holds more than half of it.
pub fn majority(records: &Records, index: usize) -> bool {
    let Some(manager) = records.managers.get(index) else { return false };
    let slot = place(records, index);
    if slot == 0 || manager.country() == 0 {
        return false;
    }
    listing(records, manager.country() as u8, manager.team() as i16)
        .is_some_and(|(place, total)| total != 0 && holding(records, slot, place) > total / 2)
}

/// The board's confidence as the screens show it: 100 for a manager
/// holding the majority of his own club, else as he stands
/// (`FUN_007701b2`).
pub fn confidence(records: &Records, index: usize) -> i32 {
    match records.managers.get(index) {
        Some(_) if majority(records, index) => 100,
        Some(manager) => manager.shown_confidence(),
        None => 0,
    }
}
