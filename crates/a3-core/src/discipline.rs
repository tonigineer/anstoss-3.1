//! What a manager does to his players from the line-up's menu: a special
//! bonus for one player or the whole team (`CSondersiegpraemieAusschuetten`),
//! a fine, a suspension, a cut of the team's bonuses
//! (`CGeldstrafeAussprechen`). The records change as the original's do; the
//! club's money only through what it has paid in bonuses and fines this
//! season, which the finances count.

use std::io;

use a3_files::exe::Exe;

use crate::mood::{self, LAST_REAL_ID, MoodTables};
use crate::random::Random;
use crate::records::{Player, Records};

/// How the sliders' positions turn into amounts: up to each limit a step of
/// so much, the last without limit (`FUN_005c69c9`).
const BONUS_SCALE_AT: u32 = 0x887c68;
const FINE_SCALE_AT: u32 = 0x8817a0;
const SCALE_STEPS: usize = 10;
/// What a fine must come to, in thousands, to settle an offence of each
/// level: once, again, ..., by what he did before.
const THRESHOLDS_AT: u32 = 0x8817f0;
const THRESHOLD_LEVELS: usize = 3;
const THRESHOLD_COLUMNS: usize = 5;
/// The units of the sliders' amounts: a hundred for a player's bonus and a
/// fine, a thousand for the team's bonus (`DAT_00a3b3fc`, `DAT_00a3b3e4`).
pub const PLAYER_UNIT: i64 = 100;
pub const TEAM_UNIT: i64 = 1000;

/// The reasons a bonus gives: "tolle Prämien" to whom it goes, "ungerechte
/// Behandlung" to a team-mate who feels passed over; "scheiß Manager" to a
/// money-grubber fined.
const GREAT_BONUSES: u32 = 0x20000;
const UNFAIR: u32 = 0x200;
const BAD_MANAGER: u32 = 0x4000;
/// Bits of a player's second word of character.
const MONEY_GRUBBER: u16 = 0x40;
const HOTHEAD: u16 = 0x2;
/// A suspended player's mark in his absence.
pub const SUSPENDED: u8 = 0x10;
/// What a suspension leaves of his flags: what pleases him goes.
const GRIEVANCES: u32 = 0xffff;

/// Texts of the game.
pub const BONUS_WELCOMED: u16 = 4280;
pub const BONUS_IN_VAIN: u16 = 4281;
pub const NOT_THAT_EASY: u16 = 4286;
const BONUSES_CUT: u16 = 3641;
const BONUSES_STRUCK: u16 = 3642;
const SOMETHING_AT_LEAST: u16 = 3643;
const BONUSES_BACK: u16 = 3644;
pub const ACCEPTS: u16 = 5324;

pub struct DisciplineTables {
    bonus_scale: Vec<(i32, i32)>,
    fine_scale: Vec<(i32, i32)>,
    thresholds: Vec<i16>,
    pub mood: MoodTables,
}

impl DisciplineTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let scale = |at: u32| -> io::Result<Vec<(i32, i32)>> {
            let values: Vec<i32> =
                exe.u32s(at, 2 * SCALE_STEPS)?.into_iter().map(|v| v as i32).collect();
            Ok(values.chunks(2).map(|c| (c[0], c[1])).take_while(|&(_, step)| step != 0).collect())
        };
        let thresholds = exe.u16s(THRESHOLDS_AT, THRESHOLD_LEVELS * THRESHOLD_COLUMNS)?;
        Ok(Self {
            bonus_scale: scale(BONUS_SCALE_AT)?,
            fine_scale: scale(FINE_SCALE_AT)?,
            thresholds: thresholds.into_iter().map(|t| t as i16).collect(),
            mood: MoodTables::load(exe)?,
        })
    }

    /// A bonus slider's amount at a position, in its units.
    pub fn bonus_amount(&self, position: i32) -> i32 {
        amount(&self.bonus_scale, position)
    }

    pub fn bonus_position(&self, amount: i32) -> i32 {
        position(&self.bonus_scale, amount)
    }

    /// A fine's slider: its amount in hundreds.
    pub fn fine_amount(&self, position: i32) -> i32 {
        amount(&self.fine_scale, position)
    }
}

/// The amount at a slider's position on a scale of (limit, step) pairs
/// (`FUN_005c69c9`).
pub(crate) fn amount(scale: &[(i32, i32)], mut position: i32) -> i32 {
    let mut below = 0;
    for &(limit, step) in scale {
        if limit == -1 {
            return below + position * step;
        }
        let steps = (limit - below) / step;
        if position <= steps {
            return below + position * step;
        }
        below = limit;
        position -= steps;
    }
    below + position * scale.last().map_or(1, |&(_, step)| step)
}

/// The position of an amount on such a scale (`FUN_005c690b`).
pub(crate) fn position(scale: &[(i32, i32)], amount: i32) -> i32 {
    let (mut below, mut steps) = (0, 0);
    for &(limit, step) in scale {
        if limit == -1 || amount <= limit {
            return (amount - below) / step + steps;
        }
        steps += (limit - below) / step;
        below = limit;
    }
    steps
}

/// A club's team record by its country and number.
#[derive(Clone, Copy, Debug)]
pub struct Club {
    pub country: u16,
    pub number: u16,
}

/// How much bonuses count for, by the club's league: a first league's
/// players expect most.
fn weight(records: &Records, club: Club) -> i32 {
    match records.team(club.country, club.number).map_or(0, |t| t.level()) {
        0 => 1,
        1 => 2,
        _ => 4,
    }
}

fn squad(records: &Records, club: Club) -> Vec<u16> {
    records.team(club.country, club.number).map_or_else(Vec::new, |t| t.players())
}

fn player_mut(records: &mut Records, id: u16) -> Option<&mut Player> {
    (id != 0).then(|| records.players.get_mut(usize::from(id))).flatten()
}

fn team_mut(records: &mut Records, club: Club) -> Option<&mut crate::records::Team> {
    let first = usize::try_from(records.countries.first_team(club.country.into())).ok()?;
    records.teams.get_mut(first + usize::from(club.number))
}

/// A special bonus of `units` hundreds to one player; the text it gets
/// (`FUN_00540af2` with a player chosen). The manager himself cannot pay
/// himself one; nothing costs anything then.
pub fn bonus_for_player(
    tables: &DisciplineTables,
    records: &mut Records,
    random: &mut Random,
    club: Club,
    manager_slot: usize,
    player: u16,
    units: i32,
) -> Option<u16> {
    if units <= 0 || player == 0 {
        return None;
    }
    if usize::from(player) == usize::from(LAST_REAL_ID) + manager_slot + 1 {
        return Some(NOT_THAT_EASY);
    }
    let tries = units * weight(records, club) / 300;
    let welcomed = (0..tries).any(|_| random.below(100) < 30);
    let mut passed_over = 3;
    if welcomed {
        if let Some(r) = player_mut(records, player) {
            tables.mood.set_reasons(r, GREAT_BONUSES);
        }
        passed_over = 0;
        for mate in squad(records, club) {
            if random.below(100) < 3 && mate != player {
                if let Some(r) = player_mut(records, mate) {
                    tables.mood.set_reasons(r, UNFAIR);
                }
                passed_over += 1;
            }
        }
    }
    if let Some(team) = team_mut(records, club) {
        team.set_bonuses_paid(team.bonuses_paid() + (i64::from(units) * PLAYER_UNIT) as i32);
    }
    Some(if passed_over > 2 { BONUS_IN_VAIN } else { BONUS_WELCOMED })
}

/// A special bonus of `units` thousands to the whole team; the text it
/// gets. It pleases some, and takes the edge off the team's will to win.
pub fn bonus_for_team(
    tables: &DisciplineTables,
    records: &mut Records,
    random: &mut Random,
    club: Club,
    units: i32,
) -> Option<u16> {
    if units <= 0 {
        return None;
    }
    let tries = units * weight(records, club) / 50;
    let squad = squad(records, club);
    for _ in 0..tries {
        if random.below(100) < 30 {
            let lucky = squad.get(random.below(squad.len() as u32) as usize).copied();
            if let Some(r) = lucky.and_then(|id| player_mut(records, id)) {
                tables.mood.set_reasons(r, GREAT_BONUSES);
            }
        }
    }
    if let Some(team) = team_mut(records, club) {
        let will = (i32::from(team.will_to_win()) - 2 * tries).clamp(-200, 200);
        team.set_will_to_win(will as i16);
        team.set_bonuses_paid(team.bonuses_paid() + (i64::from(units) * TEAM_UNIT) as i32);
    }
    Some(if tries > 2 { BONUS_WELCOMED } else { BONUS_IN_VAIN })
}

/// The team's bonuses: full, half, none.
pub fn bonus_cut(records: &Records, club: Club) -> i16 {
    records.team(club.country, club.number).map_or(0, |t| t.bonus_cut())
}

/// Cuts the team's bonuses to `cut`, or gives them back; what the team
/// says to it, if it changed (`FUN_004b491d`).
pub fn cut_bonuses(records: &mut Records, club: Club, cut: i16) -> Option<u16> {
    let team = team_mut(records, club)?;
    let old = team.bonus_cut();
    team.set_bonus_cut(cut);
    match cut.cmp(&old) {
        std::cmp::Ordering::Greater => Some(if cut == 2 { BONUSES_STRUCK } else { BONUSES_CUT }),
        std::cmp::Ordering::Less => Some(if cut == 1 { SOMETHING_AT_LEAST } else { BONUSES_BACK }),
        std::cmp::Ordering::Equal => None,
    }
}

/// The worst he did this week, by level 1 to 4 (`FUN_00702daf`).
pub fn offence(r: &Player) -> u8 {
    (1..=4).rev().find(|&k| r.offences() & (0x10 << (k - 1)) != 0).unwrap_or(0)
}

/// How often he did something of a level, at most three times.
fn times(r: &Player, level: u8) -> u8 {
    (r.offence_counts() >> (2 * (level - 1))) & 3
}

/// Forgets his offences of a level and below (`FUN_00702e5b`).
fn settle(r: &mut Player, level: u8) {
    let mask = [0x11, 0x33, 0x77][usize::from(level.clamp(1, 3)) - 1];
    r.set_offences(r.offences() & !mask);
}

/// What a fine comes to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fined {
    /// He did nothing to be fined for.
    Innocent,
    /// He has been punished this week already.
    Already,
    /// Nothing, for a fine of nothing.
    Nothing,
    /// He takes it to heart and promises to do better: a hothead no more.
    Reformed,
    /// He accepts it; his contentment with the trainer before and after.
    Accepts { before: i32, after: i32 },
}

/// A fine of `amount` euros (`FUN_004b4d0d`).
pub fn fine(
    tables: &DisciplineTables,
    records: &mut Records,
    random: &mut Random,
    club: Club,
    player: u16,
    amount: i64,
) -> Fined {
    let weight = weight(records, club);
    let Some(r) = records.player(player) else { return Fined::Nothing };
    let level = offence(r);
    if level == 0 {
        return Fined::Innocent;
    }
    if r.fined() != 0 {
        return Fined::Already;
    }
    if amount <= 0 {
        return Fined::Nothing;
    }
    if let Some(team) = team_mut(records, club) {
        team.set_fines(team.fines() + amount as i32);
    }
    let r = player_mut(records, player).expect("the player");
    r.set_fined(r.fined() | 1 << level);
    let again = times(r, level);
    let worse: u8 = (level + 1..=3).map(|k| times(r, k)).sum();
    let before = mood::parts(r)[0];
    // Levels above three are never set; the original would read past its
    // table for them.
    let row = usize::from(level.min(3)) - 1;
    let threshold = |column: usize| {
        i64::from(tables.thresholds[row * THRESHOLD_COLUMNS + column]) * TEAM_UNIT
            / i64::from(weight)
    };
    let column = match again {
        1 => {
            settle(r, level);
            0
        }
        n if n > 1 => {
            if amount >= threshold(0) {
                settle(r, level);
            }
            1
        }
        _ if worse != 0 => {
            if amount >= threshold(3) {
                settle(r, level);
            }
            3
        }
        _ => {
            if amount >= threshold(2) {
                settle(r, level);
            }
            2
        }
    };
    if amount > threshold(column) {
        mood::change_trainer(r, -20);
    }
    let character = r.traits()[1];
    if character & MONEY_GRUBBER != 0 && amount > threshold(2) {
        tables.mood.set_reasons(r, BAD_MANAGER);
    }
    if character & HOTHEAD != 0 {
        // The second chance can never come: its threshold is the higher.
        let reformed = (amount >= threshold(4) && random.below(10) == 0)
            || (amount < threshold(4) && amount >= threshold(2) && random.below(50) == 0);
        if reformed {
            let mut traits = r.traits();
            traits[1] &= !HOTHEAD;
            r.set_traits(traits);
            return Fined::Reformed;
        }
    }
    Fined::Accepts { before, after: mood::parts(r)[0] }
}

/// Whether he is suspended already.
pub fn suspended(r: &Player) -> bool {
    r.away_kind() & SUSPENDED != 0
}

/// Notes he was dealt with this week, as asking to suspend him does even
/// when it is called off.
pub fn mark_dealt_with(records: &mut Records, player: u16) {
    if let Some(r) = player_mut(records, player) {
        let level = offence(r);
        r.set_fined(r.fined() | 1 << level);
    }
}

/// Suspends him from matches and training (`FUN_004b4abf`): what pleases
/// him goes, and a bad offence that was not his first is settled.
pub fn suspend(records: &mut Records, player: u16) {
    let Some(r) = player_mut(records, player) else { return };
    let level = offence(r);
    r.set_away_kind(r.away_kind() | SUSPENDED);
    if level >= 3 && times(r, 3) + times(r, 4) > 1 {
        settle(r, level);
    }
    r.set_flags(r.flags() & GRIEVANCES);
    r.set_morale(mood::morale(r));
}
