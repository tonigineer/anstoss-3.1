//! Each player's own training (dialog 347; TRAINING-INDIV.md in the RE
//! repo): the ability he works on and how far he has got with each, how
//! hard he trains, an extra shift ("Sonderschicht") and the youth round
//! ("Nachwuchsrunde"); and the base intensity for four groups of players,
//! by the minutes they played in the last match.

use std::io;

use a3_files::exe::Exe;

/// A human club keeps this many players' choices.
pub const ENTRIES: usize = 40;
/// The bytes of one in the save.
pub const ENTRY: usize = 0x30;
/// Working on no ability: he only keeps those he has.
pub const NOTHING: i16 = 0x10;
/// How far he got with an ability he has.
pub const LEARNT: i8 = 100;
/// The abilities of a field player and of a keeper.
pub const FIELD_ABILITIES: usize = 16;
pub const KEEPER_ABILITIES: usize = 9;
/// How hard he trains: out (injured or away), building up, light, normal,
/// hard.
pub const OUT: i16 = 0;
pub const BUILD_UP: i16 = 1;
pub const LIGHT: i16 = 2;
pub const NORMAL: i16 = 3;
pub const HARD: i16 = 4;
/// The groups by minutes played: more than 70, 31 to 70, fewer, none.
pub const GROUPS: usize = 4;

/// Out this long, a player rests.
const LONG_OUT: i32 = 7;
/// Minutes that make the first and the second group.
const MOST_MINUTES: u8 = 71;
const SOME_MINUTES: u8 = 31;
/// The random favourite ability a player starts with: its number in the
/// low five bits, how far he got in the rest, this much a step.
const FAVOURITE: u8 = 0x1f;
const FAVOURITE_STEP: i32 = 14;
const MOST_FAVOURITE: i32 = 99;

const DIFFICULTIES_AT: [u32; 2] = [0x8aa4e4, 0x8aa4f4];
const INTENSITY_TEXTS_AT: u32 = 0x88b1e0;
const STEPS_AT: u32 = 0x88b1f4;
const INTENSITIES: usize = 5;

/// One player's choices, as the store holds them (`DAT_00a50028`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entry {
    /// His id, 0 for a free entry.
    pub id: u16,
    /// How far he got with each ability, 0 to 100 (he has it).
    pub progress: [i8; FIELD_ABILITIES],
    /// The ability he works on, or [`NOTHING`].
    pub ability: i16,
    pub youth: bool,
    pub extra: bool,
    pub intensity: i16,
    /// The rest of the entry, which the game does not use.
    rest: [u8; ENTRY - 0x1a],
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            id: 0,
            progress: [0; FIELD_ABILITIES],
            ability: NOTHING,
            youth: false,
            extra: false,
            intensity: OUT,
            rest: [0; ENTRY - 0x1a],
        }
    }
}

impl Entry {
    pub fn read(bytes: &[u8]) -> Self {
        let word = |at: usize| i16::from_le_bytes([bytes[at], bytes[at + 1]]);
        Self {
            id: word(0) as u16,
            progress: std::array::from_fn(|i| bytes[2 + i] as i8),
            ability: word(0x12),
            youth: word(0x14) != 0,
            extra: word(0x16) != 0,
            intensity: word(0x18),
            rest: bytes[0x1a..ENTRY].try_into().unwrap_or([0; ENTRY - 0x1a]),
        }
    }

    pub fn write(&self, bytes: &mut [u8]) {
        let mut put =
            |at: usize, value: i16| bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
        put(0, self.id as i16);
        put(0x12, self.ability);
        put(0x14, i16::from(self.youth));
        put(0x16, i16::from(self.extra));
        put(0x18, self.intensity);
        for (i, &p) in self.progress.iter().enumerate() {
            bytes[2 + i] = p as u8;
        }
        bytes[0x1a..ENTRY].copy_from_slice(&self.rest);
    }

    /// The choices of a player who has none yet (`FUN_007467de`): his
    /// favourite ability if he lacks it, else his first weakness, else
    /// none; the abilities he has learnt.
    pub fn new(
        id: u16,
        keeper: bool,
        [good, bad]: [u16; 2],
        favourite: u8,
        intensity: i16,
    ) -> Self {
        let mut entry = Self { id, intensity, ..Self::default() };
        let pick = usize::from(favourite & FAVOURITE);
        if pick > 0 && good & (1 << (pick - 1)) == 0 {
            entry.ability = (pick - 1) as i16;
            let got = i32::from(favourite >> 5) * FAVOURITE_STEP;
            entry.progress[pick - 1] = got.min(MOST_FAVOURITE) as i8;
        }
        for b in 0..abilities(keeper) {
            if bad & (1 << b) != 0 {
                if entry.ability == NOTHING {
                    entry.ability = b as i16;
                }
            } else if good & (1 << b) != 0 {
                entry.progress[b] = LEARNT;
            } else if b as i16 != entry.ability {
                entry.progress[b] = 0;
            }
        }
        entry
    }
}

/// How many abilities a player can have.
pub fn abilities(keeper: bool) -> usize {
    if keeper { KEEPER_ABILITIES } else { FIELD_ABILITIES }
}

/// How hard a player trains before anyone chose (`FUN_0074be15`): not at
/// all while out a week or suspended, light while out a little.
pub fn first_intensity(days_out: i32, suspended: bool) -> i16 {
    if days_out >= LONG_OUT || suspended {
        OUT
    } else if days_out >= 1 {
        LIGHT
    } else {
        NORMAL
    }
}

/// A group's button state, 0 to 2, from the team's four (team +0xcf).
pub fn state(states: u8, group: usize) -> u8 {
    (states >> (2 * group)) & 3
}

pub fn with_state(states: u8, group: usize, state: u8) -> u8 {
    states & !(3 << (2 * group)) | (state & 3) << (2 * group)
}

/// How hard a player trains by the four buttons (`FUN_0074bbeb`): his
/// group's state, then less when he is tired, more when he is fresh but
/// out of condition.
pub fn intensity(
    states: u8,
    days_out: i32,
    minutes: u8,
    today: usize,
    condition: i32,
    freshness: i32,
) -> i16 {
    if days_out >= LONG_OUT {
        return OUT;
    }
    let group = if minutes >= MOST_MINUTES {
        0
    } else if minutes >= SOME_MINUTES {
        1
    } else if minutes == 0 && today != 0 {
        3
    } else {
        2
    };
    let mut c = (i16::from(state(states, group)) + 2).min(HARD);
    if condition < 30 && freshness < 70 {
        c = BUILD_UP;
    }
    if condition < 30 && freshness >= 70 {
        c = HARD;
    }
    if condition < 50 && freshness >= 70 && c < NORMAL {
        c = NORMAL;
    }
    if freshness < 30 {
        c = LIGHT;
    }
    if freshness < 50 && c > NORMAL {
        c = NORMAL;
    }
    c
}

/// Keeps a player's choices in the store: in his entry, else the first
/// free one, else nowhere (`FUN_00746997`).
pub fn keep(store: &mut [Entry], entry: Entry) {
    let at = store
        .iter()
        .position(|e| e.id == entry.id)
        .or_else(|| store.iter().position(|e| e.id == 0));
    if let Some(at) = at {
        store[at] = entry;
    }
}

/// Frees the entries of players no longer in the squad (`FUN_00746b23`).
pub fn purge(store: &mut [Entry], squad: &[u16]) {
    for entry in store.iter_mut().filter(|e| e.id != 0 && !squad.contains(&e.id)) {
        entry.id = 0;
    }
}

/// The game's tables of individual training.
pub struct IndividualTables {
    /// How hard each ability is to learn, field players' and keepers'.
    difficulties: [Vec<u8>; 2],
    /// The intensities' names.
    pub intensity_texts: Vec<u16>,
    /// The next intensity a click gives, fit and out a week.
    steps: Vec<i16>,
}

impl IndividualTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let [field, keeper] = DIFFICULTIES_AT;
        Ok(Self {
            difficulties: [
                exe.bytes(field, FIELD_ABILITIES)?.to_vec(),
                exe.bytes(keeper, FIELD_ABILITIES)?.to_vec(),
            ],
            intensity_texts: exe
                .u32s(INTENSITY_TEXTS_AT, INTENSITIES)?
                .into_iter()
                .map(|t| t as u16)
                .collect(),
            steps: exe.u16s(STEPS_AT, 2 * INTENSITIES)?.into_iter().map(|s| s as i16).collect(),
        })
    }

    /// How hard an ability is to learn (`FUN_0074721e`).
    pub fn difficulty(&self, keeper: bool, ability: usize) -> u8 {
        self.difficulties[usize::from(keeper)].get(ability).copied().unwrap_or(0)
    }

    /// The text of an intensity.
    pub fn intensity_text(&self, intensity: i16) -> u16 {
        usize::try_from(intensity)
            .ok()
            .and_then(|i| self.intensity_texts.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// The intensity a click on it gives: round from building up to hard,
    /// a player out a week from light; out stays out (`0x596949`).
    pub fn step(&self, intensity: i16, long_out: bool) -> i16 {
        if intensity < BUILD_UP {
            return intensity;
        }
        let at = usize::from(long_out) * INTENSITIES + intensity.min(HARD) as usize;
        self.steps.get(at).copied().unwrap_or(intensity)
    }
}

/// How long a progress bar is, by how hard its ability is: 80, 100 or 120.
pub fn bar_width(difficulty: u8) -> i32 {
    match difficulty {
        1 => 80,
        2 => 100,
        _ => 120,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_entry_works_on_the_first_weakness() {
        // Good at Kopfball, bad at Flanken (bits 0 and 6).
        let entry = Entry::new(7, false, [1, 1 << 6], 0, NORMAL);
        assert_eq!(entry.ability, 6);
        assert_eq!((entry.progress[0], entry.progress[6]), (LEARNT, 0));
        let favourite = Entry::new(7, false, [1, 0], 3 | 2 << 5, NORMAL);
        assert_eq!((favourite.ability, favourite.progress[2]), (2, 28));
    }

    #[test]
    fn the_buttons_give_the_intensity_by_minutes() {
        // Union's states 0 and fit players: light for everyone.
        assert_eq!(intensity(0, 0, 90, 63, 87, 84), LIGHT);
        // 0x94: 0, 1, 1, 2 — no minutes after the first day is hard.
        assert_eq!(intensity(0x94, 0, 0, 63, 80, 80), HARD);
        assert_eq!(intensity(0x94, 0, 45, 63, 80, 80), NORMAL);
        assert_eq!(intensity(0x94, 0, 0, 63, 36, 75), HARD);
        assert_eq!(intensity(0x94, 0, 0, 63, 80, 20), LIGHT);
        assert_eq!(intensity(0x94, 9, 90, 63, 80, 80), OUT);
        assert_eq!(with_state(0x94, 3, 0), 0x14);
    }
}
