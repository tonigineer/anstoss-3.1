//! A club's tactics as the tactics screen sets them (dialog 271, `CTaktik`;
//! TAKTIK.md in the RE repo): how the team plays, its effort and hardness,
//! time-wasting and dirty tricks, the offside trap, pressing, the keeper
//! coming out, easing off three goals up, a special tactic and the sides it
//! attacks over, its passing and how it keeps its positions; and for a human
//! club the players to spare and to collect a yellow card, the opponent's to
//! foul and to provoke, and a special win bonus.

use std::io;

use a3_files::exe::Exe;

use crate::discipline;
use crate::records::{Extra, Team};

/// "Einsatz": its five states by the value the game keeps (0 to 6), and the
/// value each state is kept as (u16 tables at 0x88ac2c and 0x88ac3c).
const EFFORT_STATES_AT: u32 = 0x88ac2c;
const EFFORT_VALUES_AT: u32 = 0x88ac3c;
const EFFORT_VALUES: usize = 7;
pub const EFFORT_STATES: usize = 5;
/// What the four stepping buttons' states are called: "Spielweise",
/// "Härte", "Einsatz" and "Zeitspiel" (u16 text ids).
const STYLE_TEXTS_AT: u32 = 0x88ac08;
const HARDNESS_TEXTS_AT: u32 = 0x88ac20;
const EFFORT_TEXTS_AT: u32 = 0x88ac48;
const TIME_TEXTS_AT: u32 = 0x88ac18;
pub const STYLES: usize = 7;
pub const HARDNESSES: usize = 5;
pub const TIMES: usize = 4;
/// The special win bonus's scale: up to each limit a step of so much, the
/// last without limit; and the most it may be, a million (`DAT_00a3b330`,
/// a thousand times `DAT_00a3b3e4`).
const BONUS_SCALE_AT: u32 = 0x88abb0;
const SCALE_STEPS: usize = 10;
pub const MOST_BONUS: i32 = 1_000_000;

/// The flags' bits: "Zeitspiel" (0 to 3), "Miese Tricks", the offside
/// trap, pressing, easing off three goals up, the keeper playing up, the
/// special tactic (0 to 7), and the attack over the right, the middle, the
/// left.
pub const TIME_WASTING: u16 = 0x3;
pub const TRICKS: u16 = 0x4;
pub const OFFSIDE_TRAP: u16 = 0x8;
pub const PRESSING: u16 = 0x10;
pub const EASE_OFF: u16 = 0x20;
pub const KEEPER_UP: u16 = 0x40;
const SPECIAL: u16 = 0x380;
const SPECIAL_SHIFT: u16 = 7;
pub const SIDES: [u16; 3] = [0x400, 0x800, 0x1000];
pub const SPECIALS: usize = 8;

/// The passing word's three fields: short or long (0 to 6), safe or risky
/// (0 to 6), keeping position or not (0 to 4) — where each lies and its
/// most.
pub const PASSING: [(u16, i32); 3] = [(0, 6), (3, 6), (6, 4)];
const PASSING_FIELD: u16 = 7;
/// The passing word's bits the comparison with the opponent sets: the
/// strikers and the playmakers are marked man for man ([`crate::marking`]).
pub const MARK_STRIKERS: u16 = 0x4000;
pub const MARK_PLAYMAKERS: u16 = 0x2000;

/// The players the tactics name: one to spare, one to collect a yellow
/// card, the opponent's to foul and to provoke.
pub const SPARE: usize = 0;
pub const CARD: usize = 1;
pub const FOUL: usize = 2;
pub const PROVOKE: usize = 3;
pub const PICKS: usize = 4;

pub struct TacticsTables {
    effort_states: Vec<u16>,
    effort_values: Vec<u16>,
    bonus_scale: Vec<(i32, i32)>,
    pub style_texts: Vec<u16>,
    pub hardness_texts: Vec<u16>,
    pub effort_texts: Vec<u16>,
    pub time_texts: Vec<u16>,
}

impl TacticsTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let scale: Vec<i32> =
            exe.u32s(BONUS_SCALE_AT, 2 * SCALE_STEPS)?.into_iter().map(|v| v as i32).collect();
        Ok(Self {
            effort_states: exe.u16s(EFFORT_STATES_AT, EFFORT_VALUES)?,
            effort_values: exe.u16s(EFFORT_VALUES_AT, EFFORT_STATES)?,
            bonus_scale: scale
                .chunks(2)
                .map(|c| (c[0], c[1]))
                .take_while(|&(_, step)| step != 0)
                .collect(),
            style_texts: exe.u16s(STYLE_TEXTS_AT, STYLES)?,
            hardness_texts: exe.u16s(HARDNESS_TEXTS_AT, HARDNESSES)?,
            effort_texts: exe.u16s(EFFORT_TEXTS_AT, EFFORT_STATES)?,
            time_texts: exe.u16s(TIME_TEXTS_AT, TIMES)?,
        })
    }

    /// The special win bonus at a position of its spinner.
    pub fn bonus_at(&self, position: i32) -> i32 {
        discipline::amount(&self.bonus_scale, position)
    }

    /// The spinner's position for a bonus; its last is the most bonus's.
    pub fn bonus_position(&self, bonus: i32) -> i32 {
        discipline::position(&self.bonus_scale, bonus)
    }
}

/// A player the tactics name: his club, and his place in its squad.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pick {
    pub club: (usize, usize),
    pub place: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tactics {
    /// "Spielweise", 0 "Abwehrriegel" to 6 "Brechstange" (team +0xcc).
    pub style: u8,
    /// "Einsatz" as the game keeps it, 0 to 6 (team +0xcd).
    pub effort: u8,
    /// "Härte", 0 "Gewalttätig" to 4 "Nonnenhockey" (team +0xce).
    pub hardness: u8,
    /// The bits above (team +0xd0).
    pub flags: u16,
    /// The three fields of [`PASSING`] (team +0xd2).
    pub passing: u16,
    /// The offside trap's strength; the screen shows a tenth of it (team
    /// +0xc6).
    pub offside: u8,
    /// The special win bonus (the extra data's +0x394).
    pub bonus: i32,
    /// The players of [`SPARE`], [`CARD`], [`FOUL`] and [`PROVOKE`] (the
    /// extra data's +0x38a, as ids).
    pub picks: [Option<Pick>; PICKS],
}

impl Default for Tactics {
    /// A new game's, as the original's human club had them in the oracle's
    /// run.
    fn default() -> Self {
        Self {
            style: 3,
            effort: 4,
            hardness: 2,
            flags: 0,
            passing: 0xdb,
            offside: 40,
            bonus: 0,
            picks: [None; PICKS],
        }
    }
}

impl Tactics {
    /// A team's, from its record and its human club's extra data; the
    /// players are left to the caller, who knows the squads.
    pub fn of(team: &Team, extra: Option<&Extra>) -> Self {
        Self {
            style: team.style(),
            effort: team.effort(),
            hardness: team.hardness(),
            flags: team.tactic_flags(),
            passing: team.passing(),
            offside: team.offside(),
            bonus: extra.map_or(0, Extra::special_bonus),
            picks: [None; PICKS],
        }
    }

    /// Back into the team's record.
    pub fn write(&self, team: &mut Team) {
        team.set_style(self.style);
        team.set_effort(self.effort);
        team.set_hardness(self.hardness);
        team.set_tactic_flags(self.flags);
        team.set_passing(self.passing);
        team.set_offside(self.offside);
    }

    /// And into the human club's extra data, the players by their ids.
    pub fn write_extra(&self, extra: &mut Extra, ids: [u16; PICKS]) {
        extra.set_special_bonus(self.bonus);
        extra.set_picks(ids);
    }

    pub fn has(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }

    pub fn set(&mut self, flag: u16, on: bool) {
        if on {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    /// Whether one of the passing word's bits is set, and setting it.
    pub fn marks(&self, bit: u16) -> bool {
        self.passing & bit != 0
    }

    pub fn set_marks(&mut self, bit: u16, on: bool) {
        if on {
            self.passing |= bit;
        } else {
            self.passing &= !bit;
        }
    }

    pub fn time_wasting(&self) -> usize {
        usize::from(self.flags & TIME_WASTING)
    }

    pub fn special(&self) -> usize {
        usize::from((self.flags & SPECIAL) >> SPECIAL_SHIFT)
    }

    pub fn set_special(&mut self, special: usize) {
        let special = (special as u16) << SPECIAL_SHIFT & SPECIAL;
        self.flags = self.flags & !SPECIAL | special;
    }

    /// "Einsatz" as its button shows it.
    pub fn effort_state(&self, tables: &TacticsTables) -> usize {
        tables.effort_states.get(usize::from(self.effort)).map_or(0, |&s| usize::from(s))
    }

    /// The four stepping buttons' states into the tactics, as the screen
    /// keeps them when it closes (`FUN_0058c6f5`): the effort by its state's
    /// value, so that a value between two states moves to one.
    pub fn keep_buttons(
        &mut self,
        tables: &TacticsTables,
        [style, hardness, effort, time, tricks]: [usize; 5],
    ) {
        self.style = style as u8;
        self.hardness = hardness as u8;
        self.effort = tables.effort_values.get(effort).map_or(self.effort, |&v| v as u8);
        self.flags = self.flags & !(TIME_WASTING | TRICKS)
            | (time as u16 & TIME_WASTING)
            | if tricks != 0 { TRICKS } else { 0 };
    }

    /// One of the passing word's fields.
    pub fn passing(&self, field: usize) -> i32 {
        let (shift, _) = PASSING[field];
        i32::from(self.passing >> shift & PASSING_FIELD)
    }

    pub fn set_passing(&mut self, field: usize, value: i32) {
        let (shift, most) = PASSING[field];
        let value = value.clamp(0, most) as u16;
        self.passing = self.passing & !(PASSING_FIELD << shift) | value << shift;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flags_and_the_passing_are_kept_in_their_bits() {
        // Union Berlin in the user's save: "hohe Flanken", passing 3, 3, 2.
        let mut t = Tactics { flags: 0x100, passing: 0x609b, ..Tactics::default() };
        assert_eq!(t.special(), 2);
        assert_eq!([t.passing(0), t.passing(1), t.passing(2)], [3, 3, 2]);
        t.set_special(5);
        t.set(SIDES[1], true);
        t.set_passing(2, 4);
        assert_eq!(t.flags, 0x280 | 0x800);
        assert_eq!(t.passing, 0x609b & !0x1c0 | 0x100);
    }
}
