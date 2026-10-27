//! Doping, as the medical department's dialog 394 (`CMediDoping`) does it:
//! who would take something, the list of those to dope, and a dose — what
//! it costs, whether it can be found out, what it does at once.
//!
//! A player's doping byte (R+0x26d): bits 0–1 doped (`FUN_006ff9be`),
//! bits 2–3 the kind taken, from 1 (aggression, condition,
//! concentration), 0x40 on the list (`FUN_006ff976`), 0x80 safe from
//! being found out (`FUN_006ff715`).

use crate::mood;
use crate::records::{Player, Records};

const DOPED: u8 = 0x03;
const KIND_SHIFT: u8 = 2;
const ON_LIST: u8 = 0x40;
const SAFE: u8 = 0x80;
/// What a dose keeps of the old byte (`& 0x70`).
const KEPT: u8 = 0x70;
/// The kind that raises the condition, and by how much (`FUN_00411130`).
pub const CONDITION: u8 = 2;
const CONDITION_GAIN: i16 = 10 * 100;
const CONDITION_MOST: i16 = 10_000;

/// A refusal costs him this much of his contentment with the trainer
/// (`FUN_007007a1(-10)`).
pub const REFUSAL: i32 = -10;

/// The prices of a dose, normal and extra-hard to find (`DAT_00a3b3bc`,
/// `DAT_00a3b3a0`), and the medical budget above which the risk of being
/// found out grows, in steps of (`DAT_00a3b330`, `DAT_00a3b38c`).
pub const NORMAL_PRICE: i32 = 10_000;
pub const ESN_PRICE: i32 = 30_000;
const BUDGET_FREE: i32 = 1_000_000;
const BUDGET_STEP: i32 = 50_000;
const RISK_LEAST: u32 = 2;
const RISK_MOST: u32 = 40;

/// The manager's own player records start above this id.
const MANAGERS_FROM: u16 = 0x4268;
/// R+0x26f bit 0x80: never; traits[1] bit 0x100: only with luck.
const NEVER: u8 = 0x80;
const WARY: u16 = 0x100;
const STRENGTH_BELOW: u8 = 7;
const AGE_BELOW: u32 = 31;
const YEARS_LEAST: i32 = 2;
const CONTRACT_YEAR: u16 = 0xfff;

pub fn doped(r: &Player) -> bool {
    r.doping() & DOPED != 0
}

pub fn on_list(r: &Player) -> bool {
    r.doping() & ON_LIST != 0
}

/// Onto the list or off it (`FUN_006ff8fc`).
pub fn toggle_list(r: &mut Player) {
    r.set_doping(r.doping() ^ ON_LIST);
}

/// How ready he is to take something (`FUN_005168fb`): never, only if
/// his luck lies below the manager's authority, or yes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Readiness {
    Never,
    Luck,
    Yes,
}

/// `age` his age today, `start_year` the season's.
pub fn readiness(r: &Player, id: u16, age: u32, start_year: u16) -> Readiness {
    let content = mood::content_level(mood::parts(r));
    if r.retiring() & NEVER != 0
        || id > MANAGERS_FROM
        || content >= 2
        || r.strength() >= STRENGTH_BELOW
    {
        return Readiness::Never;
    }
    let years = (i32::from(r.contract() & CONTRACT_YEAR) - i32::from(start_year)) as i8;
    if i32::from(years) < YEARS_LEAST || age >= AGE_BELOW || r.traits()[1] & WARY != 0 {
        return Readiness::Luck;
    }
    Readiness::Yes
}

/// Whether he would (`FUN_005169a9`): with only luck to go on, if his
/// luck lies below the manager's authority.
pub fn willing(records: &Records, readiness: Readiness, id: u16, authority: i32) -> bool {
    match readiness {
        Readiness::Never => false,
        Readiness::Luck => i32::from(records.luck.of(id)) < authority,
        Readiness::Yes => true,
    }
}

/// The price of a dose.
pub fn price(esn: bool) -> i32 {
    if esn { ESN_PRICE } else { NORMAL_PRICE }
}

/// The risk of being found out, in percent: 2, or two for every 50,000
/// the planned medical budget lies above 1,000,000, at most 40 — then
/// doubled for the extra-hard kind (`FUN_00516f7a`).
pub fn risk(medical_plan: i32, esn: bool) -> u32 {
    let mut risk = RISK_LEAST;
    if medical_plan > BUDGET_FREE {
        risk = (((medical_plan - BUDGET_FREE) / BUDGET_STEP) as u32 * 2).min(RISK_MOST);
    }
    if esn { risk * 2 } else { risk }
}

/// A dose of `kind` (0 aggression, 1 condition, 2 concentration): doped,
/// the kind kept, safe when the roll of 100 lies at or above the risk
/// (`FUN_006ff715`); the condition rises at once.
pub fn dose(r: &mut Player, kind: u8, roll: u32, risk: u32) {
    let kind = kind.min(2) + 1;
    let mut byte = r.doping() & KEPT | 2 | kind << KIND_SHIFT;
    if roll >= risk {
        byte |= SAFE;
    }
    r.set_doping(byte);
    if kind == CONDITION {
        r.set_condition((r.condition() + CONDITION_GAIN).clamp(0, CONDITION_MOST));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_risk_grows_with_the_medical_budget() {
        assert_eq!(risk(500_000, false), 2);
        assert_eq!(risk(1_000_000, false), 2);
        assert_eq!(risk(1_100_000, false), 4);
        assert_eq!(risk(2_000_000, true), 80);
        assert_eq!(risk(5_000_000, false), 40);
    }
}
