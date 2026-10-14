//! The medical department (`CMedizinischeAbteilung`, dialog 151; the RE
//! repo's MEDIZIN.md): who is injured or plays with a knock, in what order
//! the list shows them, what the doctor says of each, the fitness injection
//! a player may get for some injuries, and the operations due when the
//! department closes.

use std::io;

use a3_files::exe::Exe;

use crate::mood;
use crate::random::Random;
use crate::records::{Player, Records};
use crate::season::ClubState;

/// The kinds of injury and illness (10 bytes each at 0x8ac340): byte 6 lets
/// a player out a week or more play with an injection (`FUN_00767832`).
const KINDS_AT: u32 = 0x8ac340;
const KIND_SIZE: usize = 10;
const KINDS: usize = 95;
const INJECTION_BYTE: usize = 6;

/// Bits of a player's knocks (`Player::knocks`): an operation due, the
/// injection, the three knocks he plays on with.
pub const OPERATION_DUE: u16 = 0x0800;
pub const INJECTION: u16 = 0x1000;
pub const KNOCKS: u16 = 0xe000;
const GLANDULAR_FEVER: u16 = 0x2000;
const ACHILLES: u16 = 0x4000;
const GROIN: u16 = 0x8000;
/// The operations: the kind of injury each turns into, and for how long.
const ACHILLES_OPERATION: (u8, i32) = (65, 56);
const GROIN_OPERATION: (u8, i32) = (66, 42);
/// Days out from which a player is injured rather than knocked.
pub const WEEK: i32 = 7;

pub struct MedicalTables {
    injection: Vec<bool>,
}

impl MedicalTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let kinds = exe.bytes(KINDS_AT, KIND_SIZE * KINDS)?;
        Ok(Self { injection: kinds.chunks(KIND_SIZE).map(|k| k[INJECTION_BYTE] != 0).collect() })
    }

    /// Whether an injury of this kind lets a player play with an injection.
    pub fn injection_allowed(&self, kind: u8) -> bool {
        self.injection.get(usize::from(kind)).copied().unwrap_or(false)
    }
}

/// The sick list's players, by place in the squad (`FUN_00517afe`): the
/// injured, those out longest first — on a tie in squad order — then those
/// who play with a knock, in squad order.
pub fn sick_list(club: &ClubState, knocks: impl Fn(usize) -> u16) -> Vec<usize> {
    let injured = |p: usize| club.status.get(p).map_or(0, |s| s.injured);
    let mut list: Vec<usize> = (0..club.players.len()).filter(|&p| injured(p) != 0).collect();
    list.sort_by_key(|&p| std::cmp::Reverse(injured(p)));
    list.extend((0..club.players.len()).filter(|&p| injured(p) == 0 && knocks(p) & KNOCKS != 0));
    list
}

/// The knock a player plays on with, as the list names it (texts 4449 to
/// 4451).
pub fn knock_text(knocks: u16) -> u16 {
    if knocks & GLANDULAR_FEVER != 0 {
        4449
    } else if knocks & ACHILLES != 0 {
        4450
    } else {
        4451
    }
}

/// What the doctor says of an injured player (`FUN_00518783`), the first
/// that holds: he works like a man possessed, he is over-motivated, he
/// stays out on purpose, he does nothing at all, he has no great wish to
/// get fit; or nothing.
pub fn note(r: &Player, visited: bool) -> Option<u16> {
    const KEEN: u16 = 0x2;
    const HOTHEAD: u16 = 0x2;
    let morale = r.morale();
    let [first, second] = r.traits();
    if first & KEEN != 0 && morale > 90 && visited || morale > 104 {
        Some(4946)
    } else if first & KEEN != 0 && second & HOTHEAD != 0 && morale > 80 && !visited {
        Some(4947)
    } else if morale < 40 {
        Some(4948)
    } else if morale < 60 {
        Some(4949)
    } else if morale < 80 && !visited {
        Some(4950)
    } else {
        None
    }
}

/// Whether a player was visited while injured (the top two bits of his
/// `visits`).
pub fn visited(r: &Player) -> bool {
    r.visits() >> 6 != 0
}

/// How a visited player takes a sick visit: the first two times kindly,
/// with one of five answers; later with one of four (`FUN_00557e1f`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reaction {
    pub kind: u8,
    pub draw: u8,
}

/// Texts of the reactions, by draw: of the first visits, of the later
/// ones (the tables at 0x8885c4 and 0x8885d8).
pub const FIRST_VISITS: [u16; 5] = [1707, 1708, 1709, 1710, 1711];
pub const LATER_VISITS: [u16; 4] = [1712, 1713, 1714, 1715];
/// A visit's reason a player lacks: "verletzt und nie besucht".
const NEVER_VISITED: u32 = 0x100;

impl Reaction {
    pub fn text(self) -> u16 {
        let texts: &[u16] = if self.kind != 0 { &FIRST_VISITS } else { &LATER_VISITS };
        texts.get(usize::from(self.draw)).copied().unwrap_or(0)
    }
}

/// A sick visit on its day (`FUN_00557e1f`): the visits counted, the first
/// two please him with the trainer, one draw for how he takes it. A player
/// healed by then forgets it at once, as the club's store does
/// (`FUN_00637177`).
pub fn visit(r: &mut Player, random: &mut Random, days_out: i32) -> Reaction {
    let visits = r.visits() >> 6;
    let reaction = if visits < 2 {
        mood::change_trainer(r, 2);
        if visits == 0 {
            mood::clear_reasons(r, NEVER_VISITED);
        }
        let mut draw = random.below(5) as u8;
        // He who likes the trainer already is not surprised.
        if mood::parts(r)[0] > 90 && draw == 3 {
            draw = 2;
        }
        Reaction { kind: 1, draw }
    } else {
        let mut draw = random.below(3) as u8;
        if draw == 2 && days_out >= 10 {
            draw = 3;
        }
        Reaction { kind: 0, draw }
    };
    let counted = (visits + 1).min(3);
    r.set_visits(r.visits() & 0x3f | counted << 6);
    if days_out <= 0 {
        r.set_visits(0);
        r.set_injured_on(0);
        r.set_injured_to([0, 0]);
    }
    reaction
}

/// A click on a player's status: the injection off, or on if his injury
/// allows one; only for a player out a week or more (`OnUserC000_00518342`).
/// Whether anything changed.
pub fn toggle_injection(tables: &MedicalTables, r: &mut Player, days_out: i32) -> bool {
    if days_out < WEEK {
        return false;
    }
    let knocks = r.knocks();
    if knocks & INJECTION != 0 {
        r.set_knocks(knocks & !INJECTION);
    } else if tables.injection_allowed(r.injury_kind()) {
        r.set_knocks(knocks | INJECTION);
    } else {
        return false;
    }
    true
}

/// The operations due when the department closes with OK (`FUN_00767917`):
/// an Achilles tendon out 56 days, a groin out 42, from today; the knock
/// goes. `squad` holds each player's id and how long he is out now;
/// returns the players operated on, and for how long they are out.
pub fn operate(records: &mut Records, squad: &[(u16, i32)]) -> Vec<(u16, i32)> {
    let today = records.globals.day();
    let mut operated = Vec::new();
    for &(id, days_out) in squad {
        let Some(r) = (id != 0).then(|| records.players.get_mut(usize::from(id))).flatten() else {
            continue;
        };
        let knocks = r.knocks();
        if knocks & OPERATION_DUE == 0 {
            continue;
        }
        let (bit, (kind, days)) = if knocks & ACHILLES != 0 {
            (ACHILLES, ACHILLES_OPERATION)
        } else if knocks & GROIN != 0 {
            (GROIN, GROIN_OPERATION)
        } else {
            r.set_knocks(knocks & !OPERATION_DUE);
            continue;
        };
        if days_out < days {
            r.set_injury_kind(kind);
            r.set_injured_on(today);
            operated.push((id, days));
            injure_for(records, id, days);
        }
        if let Some(r) = records.players.get_mut(usize::from(id)) {
            r.set_knocks(r.knocks() & !bit & !OPERATION_DUE);
        }
    }
    operated
}

/// Puts a player out for `days` from today: the year and day of the year he
/// is fit again.
fn injure_for(records: &mut Records, id: u16, days: i32) {
    let [_, _, year] = records.globals.date();
    let year_days = i32::from(records.globals.year_days());
    let end = i32::from(records.globals.day_of_year()) + days;
    let to =
        if end < year_days { [year, end as u16] } else { [year + 1, (end - year_days) as u16] };
    if let Some(r) = records.players.get_mut(usize::from(id)) {
        r.set_injured_to(to);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lineup::{BENCH, FIELD, Lineup};
    use crate::season::Status;
    use crate::world;

    fn club(injured: &[i32]) -> ClubState {
        let player = world::Player {
            last_name: String::new(),
            first_name: String::new(),
            strength: 5,
            positions: [3, 0, 0],
            abilities: [0, 0],
            traits: [0, 0],
            birthday: None,
            shirt: 0,
        };
        let lineup = Lineup { spots: [(0, 0); FIELD], field: [None; FIELD], bench: [None; BENCH] };
        let mut club = ClubState::new(vec![player; injured.len()], lineup, Vec::new(), 0);
        club.status =
            injured.iter().map(|&days| Status { injured: days, ..Status::default() }).collect();
        club
    }

    #[test]
    fn the_sick_list_puts_the_longest_out_first_then_the_knocked() {
        let club = club(&[0, 5, 20, 0, 5, 0]);
        let knocks = |p: usize| if p == 5 || p == 0 { ACHILLES } else { 0 };
        assert_eq!(sick_list(&club, knocks), [2, 1, 4, 0, 5]);
    }

    #[test]
    fn a_reaction_names_its_text() {
        assert_eq!(Reaction { kind: 1, draw: 4 }.text(), 1711);
        assert_eq!(Reaction { kind: 0, draw: 3 }.text(), 1715);
        assert_eq!(Reaction { kind: 0, draw: 4 }.text(), 0);
    }
}
