//! How content a player is, and his morale from it, as his record holds
//! them: seven parts — trainer, team, club, press, private life,
//! distractions, offers — around a hundred each, the first three moved by
//! the reasons his flags hold (`FUN_007008b9`, `FUN_00700c42`,
//! `FUN_00700e3f`); his morale their sum less six hundred (`FUN_0070147e`).
//! A reason set clears those it excludes (`FUN_00770800`).

use std::io;

use a3_files::exe::Exe;

use crate::records::Player;

/// For each reason, a bit of the flags, the reasons it clears.
const EXCLUSIONS_AT: u32 = 0x8ac7c0;
const REASONS: usize = 32;
/// The most each part and the morale come to.
pub const MOST: i32 = 120;
const MIDDLING: i32 = 100;
/// The ids above are the managers who play.
pub const LAST_REAL_ID: u16 = 17_000;

/// Grievances of the trainer, the team and the club, ten off each; what
/// pleases him with each, ten more while that keeps it to a hundred.
const TRAINER: ([u32; 9], [u32; 1]) =
    ([0x1, 0x2, 0x10, 0x8, 0x100, 0x1000, 0x4000, 0x2000, 0x8000], [0x80000]);
const TEAM: ([u32; 3], [u32; 1]) = ([0x40, 0x80, 0x800], [0x40000]);
const CLUB: ([u32; 3], [u32; 3]) = ([0x4, 0x20, 0x400], [0x10000, 0x20000, 0x100000]);

pub struct MoodTables {
    exclusions: Vec<u32>,
}

impl MoodTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        Ok(Self { exclusions: exe.u32s(EXCLUSIONS_AT, REASONS)? })
    }

    /// Gives him the reasons of `mask`, each clearing those it excludes,
    /// and reckons his morale anew (`FUN_00770800`).
    pub fn set_reasons(&self, r: &mut Player, mask: u32) {
        let mut flags = r.flags();
        for (bit, &excluded) in self.exclusions.iter().enumerate() {
            if mask & (1 << bit) != 0 {
                flags = (flags & !excluded) | (1 << bit);
            }
        }
        r.set_flags(flags);
        r.set_morale(morale(r));
    }
}

/// His contentment, part by part, as the bars of the player info show it:
/// the record's, the first three moved by the reasons his flags hold, a
/// best friend who manages pleasing him too.
pub fn parts(r: &Player) -> [i32; 7] {
    let flags = r.flags();
    let base = r.contentment().map(i32::from);
    let manager_friend = r.best_friend() > LAST_REAL_ID;
    let less = |value: i32, bits: &[u32]| {
        value - 10 * bits.iter().filter(|&&b| flags & b != 0).count() as i32
    };
    let up = |value: i32| if value < MIDDLING { (value + 10).min(MIDDLING) } else { value };
    let more = |mut value: i32, bits: &[u32]| {
        for &b in bits {
            if flags & b != 0 {
                value = up(value);
            }
        }
        value
    };
    let mut trainer = more(less(base[0], &TRAINER.0), &TRAINER.1);
    if manager_friend {
        trainer = up(trainer);
    }
    let team = more(less(base[1], &TEAM.0), &TEAM.1);
    let club = more(less(base[2] + if manager_friend { 5 } else { 0 }, &CLUB.0), &CLUB.1);
    let clamp = |v: i32| v.clamp(0, MOST);
    [clamp(trainer), clamp(team), clamp(club), base[3], base[4], base[5], base[6]]
}

/// His morale from his contentment: the parts' sum less six hundred, a
/// little less taken off while a doping ban of a known kind runs.
pub fn morale(r: &Player) -> u8 {
    let sum: i32 = parts(r).iter().sum();
    let doping = r.doping();
    let less = if doping & 3 != 0 && (doping >> 2) & 3 == 1 {
        if doping & 2 != 0 { 590 } else { 595 }
    } else {
        600
    };
    (sum - less).clamp(0, MOST) as u8
}

/// Moves his contentment with the trainer by `by`, and his morale with it
/// (`FUN_007007a1`).
pub fn change_trainer(r: &mut Player, by: i32) {
    let mut parts = r.contentment();
    parts[0] = (i32::from(parts[0]) + by).clamp(0, MOST) as u8;
    r.set_contentment(parts);
    r.set_morale(morale(r));
}

/// How content he is, as one of four: happy, content, unhappy, wanting to
/// leave (`FUN_00770877`): the last when the trainer, the team and the club
/// together please him too little, else by the sum of all seven parts.
pub fn content_level(parts: [i32; 7]) -> usize {
    let [trainer, team, club, press, private, distraction, offers] = parts;
    let work = trainer + team + club - 200;
    let all = work + press + private + distraction + offers - 300;
    match () {
        _ if work < 70 => 3,
        _ if all < 170 => 2,
        _ if all < 201 => 1,
        _ => 0,
    }
}

/// Moves his contentment with his private life by `by`, and his morale with
/// it (`FUN_007010d9`).
pub fn change_private(r: &mut Player, by: i32) {
    let mut parts = r.contentment();
    parts[4] = (i32::from(parts[4]) + by).clamp(0, MOST) as u8;
    r.set_contentment(parts);
    r.set_morale(morale(r));
}

/// Takes the reasons of `mask` from him, and reckons his morale anew
/// (`FUN_00770942`).
pub fn clear_reasons(r: &mut Player, mask: u32) {
    r.set_flags(r.flags() & !mask);
    r.set_morale(morale(r));
}
