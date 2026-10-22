//! What 466's sub-dialogs tell of the grounds (STADION-ANSICHT.md §8): the
//! info's figures, the staff, what is being built, and the fans'
//! complaints.

use super::actions::{JOBS, kind_of, listed};
use super::{ENTRIES, Grounds, long};
use crate::stadium::{STANDS, Sub};

/// The investments in the football grounds and the others (i64
/// `0x1cb5f80`, `0x1cb5f88`).
const INVESTED_FOOTBALL: usize = 0x1d5d0;
const INVESTED: usize = 0x1d5d8;
/// The counters of the Fanshop, the Kassenhaus and the Infostand
/// (`0x1cb5f2c`, `0x1cb5f70`, `0x1cb5f78`).
const FAN_SHOPS: usize = 0x1d57c;
const TICKET_OFFICES: usize = 0x1d5c0;
const INFO_STANDS: usize = 0x1d5c8;

/// The info's figures (467, `0x7c7456`): the land the club's things cover
/// (sprite width × height, in quarter hectares), its buildings finished
/// and being built, their upkeep a season.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Info {
    pub area: i32,
    pub finished: i32,
    pub building: i32,
    pub upkeep: i32,
}

/// A complaint of the fans (468), by its text.
pub type Complaint = u16;

/// What the fans look at besides the grounds: the stadium played in (its
/// look and a copy of its sub-record), the club's country and league
/// level, the month.
pub struct Fans<'a> {
    pub look: [u8; 12],
    pub stadium: &'a Sub,
    pub country: u8,
    pub level: u8,
    pub month: i32,
}

/// The kinds the fans count: food, things to do, the representative, the
/// flowers, ice cream, parking.
const FOOD: [i16; 5] = [17, 41, 35, 25, 26];
const ICE_CREAM: i16 = 41;
const FUN: [i16; 6] = [38, 42, 16, 46, 63, 43];
const SHOWPIECES: [i16; 6] = [45, 34, 43, 15, 65, 64];
const FLOWERS: i16 = 32;
const CAR_PARK: (i16, i32) = (93, 5000);
const MULTI_STOREY: (i16, i32) = (51, 20_000);
/// The countries where nobody stands (16, 57).
const ALL_SEATERS: [u8; 2] = [16, 57];

impl Grounds {
    /// The entries the club has, in use, from the last back
    /// (`FUN_007be046` and its kind).
    fn present(&self) -> impl Iterator<Item = i16> + '_ {
        (0..ENTRIES).rev().map(|k| self.entry(k)).filter(|e| e.owned && e.used).map(|e| e.kind)
    }

    /// The info's figures: the entries walked down from the count by index
    /// — one beyond it after a hole is missed —, the club's only.
    pub fn info(&self) -> Info {
        let mut info = Info::default();
        let count = usize::try_from(self.count()).unwrap_or(0).min(ENTRIES);
        for e in (0..count).rev().map(|k| self.entry(k)).filter(|e| e.owned) {
            if let Some(t) = kind_of(e.kind).filter(|t| t.building) {
                if e.weeks > 0 {
                    info.building += 1;
                } else {
                    info.finished += 1;
                }
                info.upkeep += t.upkeep;
            }
            let (w, h, _) = self.size(e.kind);
            info.area += w * h;
        }
        info
    }

    fn i64_at(&self, at: usize) -> i64 {
        i64::from_le_bytes(self.data[at..at + 8].try_into().unwrap())
    }

    /// What was invested in the football grounds (never anything) and in
    /// the rest.
    pub fn invested(&self) -> (i64, i64) {
        (self.i64_at(INVESTED_FOOTBALL), self.i64_at(INVESTED))
    }

    /// The staff's wages a year (`FUN_007bdc02`).
    pub fn wages(&self) -> i64 {
        (0..JOBS.len()).map(|job| i64::from(self.staff(job)) * i64::from(JOBS[job].1)).sum()
    }

    /// What is being built, the club's or not: entries by weeks left, the
    /// later entry first among equals (`0x7c6d9a`); with the price each
    /// is listed at.
    pub fn works_in_progress(&self) -> Vec<(u8, i16, i64)> {
        let mut works: Vec<_> = (0..ENTRIES)
            .rev()
            .map(|k| self.entry(k))
            .filter(|e| e.used && e.weeks > 0)
            .map(|e| (e.weeks, e.kind, listed(e.kind)))
            .collect();
        works.sort_by_key(|w| w.0);
        works
    }

    /// The fans' complaints, in the order the game asks (`0x7c32ac`).
    pub fn complaints(&self, fans: &Fans) -> Vec<Complaint> {
        let s = fans.stadium;
        let places = s.places()[5];
        let standing: i32 = (0..STANDS).map(|i| i32::from(s.standing(i))).sum();
        let look = fans.look;
        let count = |kind: i16| self.present().filter(|&k| k == kind).count() as i32;
        let kinds = |kinds: &[i16]| kinds.iter().filter(|&&k| count(k) > 0).count();
        let food = self.present().filter(|k| FOOD.contains(k)).count() as i32;
        let parking: i32 = self
            .present()
            .map(|k| match k {
                k if k == CAR_PARK.0 => CAR_PARK.1,
                k if k == MULTI_STOREY.0 => MULTI_STOREY.1,
                _ => 0,
            })
            .sum();
        let age = s.average_age();
        let checks: [(bool, Complaint); 16] = [
            (look[0] != 0 && look[4..8].iter().any(|&t| t != 0), 5035),
            (kinds(&FOOD) < 2, 5036),
            (kinds(&FUN) < 2, 5054),
            (long(&self.data, FAN_SHOPS) == 0, 5055),
            ((fans.level == 0 && places < 20_000) || (fans.level == 1 && places < 10_000), 5056),
            (age > 25, 5057),
            (age <= 25 && age > 10, 5058),
            (!ALL_SEATERS.contains(&fans.country) && standing == 0, 5059),
            (!(0..STANDS).all(|i| s.roof(i)), 5060),
            (food < places / 10_000, 5061),
            ((5..=10).contains(&fans.month) && count(ICE_CREAM) == 0, 5062),
            (count(FLOWERS) > places / 1000, 5063),
            (long(&self.data, TICKET_OFFICES) < places / 20_000, 5064),
            (long(&self.data, INFO_STANDS) < places / 40_000, 5065),
            (fans.level == 0 && kinds(&SHOWPIECES) == 0, 5066),
            (parking < places / 2, 5067),
        ];
        checks.iter().filter(|c| c.0).map(|c| c.1).collect()
    }
}
