//! What the managers are worth, as the comparison of the players and
//! their clubs asks it (dialog 211, ERFOLGE.md §A.6): nine marks apiece,
//! each weighed by the game's difficulty (`FUN_006dda52`).

use chrono::NaiveDate;

use crate::{
    autolineup, finances, league,
    records::{self, Records, Team},
    stock,
};

/// The unit the marks are scaled by (`DAT_00a3b3bc`).
const UNIT: i32 = 10_000;
/// The age a player counts old at in the squad's marks (`FUN_006dd041`).
const OLD: i32 = 33;
/// More than three of them and every further old one costs the squad.
const OLD_MOST: i32 = 3;
/// A player young for his strength adds this.
const YOUNG_BONUS: i32 = 20;
/// How far his age may stay above his strength and still count young.
const YOUNG_WITHIN: u32 = 16;

/// How much the game counts the abilities it asks of a player (0x8a6718)
/// and the traits it asks of his character (0x8a6738), a bit each, and
/// how much a manager stands for by where he ranks (0x8a6758).
const ABILITY_POINTS: [i16; 16] = [2, 2, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0];
const TRAIT_POINTS: [i16; 16] = [-2, 0, 2, -2, -2, -2, 2, -2, 1, -2, -2, 0, 0, 0, 0, 0];
const STANDING_POINTS: [i16; 16] = [
    -200, -100, 0, 100, 200, 400, 600, 800, 1000, 1250, 1500, 1750, 2000, 2500, 3000, 4000,
];

/// The nine marks of one manager (`FUN_006dda52`), in the order dialog
/// 211 asks them: his money, the contract he is on, his shares, how his
/// team has gone since he came, what the youth brought, what he did for
/// the club, how the season runs, what he has won, and what he stands
/// for. The last of them is his standing; the screen shows it beside the
/// mark in brackets.
/// `human` is his number among the humans, 1 to 4: his career record and
/// his portfolio stand one block after the last human's (`FUN_006a426b`,
/// which takes the manager with that number).
pub fn marks(records: &Records, manager: &records::Manager, human: usize) -> [i32; 9] {
    let mut out = [0; 9];
    out[0] = adjust(manager, manager.cash() / UNIT);
    let shares = stock::portfolio_value(records, i16::try_from(human).unwrap_or(0));
    out[2] = adjust(manager, (shares / i64::from(UNIT)) as i32);
    if manager.country() != 0 {
        let team = records.team(manager.country(), manager.team());
        if let Some(team) = team {
            let (base, per_team) = finances::TARGET_POINTS[target_at(team)];
            let size = league::teams(records, team.country(), team.level());
            let pay = manager.pay_point() * (base + per_team * (size - 10)) + manager.pay_fixed();
            out[1] = adjust(manager, pay / 2 / UNIT);
            out[3] = adjust(manager, team_marks(records, team) - manager.team_start());
        }
        out[4] = adjust(manager, manager.youth_marks());
        out[5] = adjust(manager, manager.service_marks());
        out[6] = adjust(manager, manager.season_marks());
    }
    let trainer = records
        .career(human.wrapping_sub(1))
        .and_then(|c| c.get(0x42..0x44))
        .map_or(0, |two| i16::from_le_bytes([two[0], two[1]]));
    out[7] = adjust(manager, manager.honour_marks()) + i32::from(trainer) * 5;
    let standing = usize::try_from(manager.standing()).unwrap_or(0).min(STANDING_POINTS.len() - 1);
    out[8] = adjust(manager, i32::from(STANDING_POINTS[standing]))
        + adjust(manager, manager.standing_marks());
    out
}

/// How the game weighs a mark by its difficulty (`FUN_006ddc45`): four
/// times the mark, less a quarter of `difficulty − 2` times it — so the
/// plainest difficulty leaves four times the mark, and an easier one
/// more.
fn adjust(manager: &records::Manager, x: i32) -> i32 {
    let difficulty = i32::from(manager.difficulty() & 0x7f);
    x.wrapping_mul(4)
        .wrapping_sub((difficulty - 2).wrapping_mul(x).wrapping_div(4))
}

/// The target the club plays for this season, as an index into the
/// target tables (the team record's +0x10c).
fn target_at(team: &Team) -> usize {
    usize::try_from(team.get::<i8>(finances::TARGET))
        .unwrap_or(0)
        .min(finances::TARGET_POINTS.len() - 1)
}

/// What a team is worth to its manager (`FUN_006dd041`): what the squad
/// can do, what its respect, will and mood come to, and how fit and how
/// fresh its best players are.
fn team_marks(records: &Records, team: &Team) -> i32 {
    let [day, month, year] = records.globals.date();
    let today = NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day));
    let mut sum = 0;
    let mut old = 0;
    for id in team.players() {
        let Some(player) = records.player(id) else { continue };
        sum += player_marks(player);
        let Some(today) = today else { continue };
        let years = autolineup::age(player.birthday(), today);
        if years >= OLD {
            old += 1;
            if old > OLD_MOST {
                sum -= 10;
            }
        } else {
            let strength = i32::from(player.strength());
            if ((years - strength) as u32) < YOUNG_WITHIN {
                sum += YOUNG_BONUS;
            }
        }
    }
    i32::from(team.respect()) / 10
        + sum
        - 60
        + i32::from(team.will()) / 10
        + i32::from(team.mood()) / 10
        + average(records, team, freshness_points) / 5
        + average(records, team, condition_points) / 5
}

/// What one player can do (`FUN_006dd197`): his strength ten times over,
/// plus a point for every ability he is good at and what his traits are
/// each worth.
fn player_marks(player: &records::Player) -> i32 {
    let mut sum = i32::from(player.strength()) * 10;
    let traits = player.traits();
    for bit in 0..16 {
        if player.good() & (1 << bit) != 0 {
            sum += 1;
        }
        if traits[0] & (1 << bit) != 0 {
            sum += i32::from(ABILITY_POINTS[bit]);
        }
        if traits[1] & (1 << bit) != 0 {
            sum += i32::from(TRAIT_POINTS[bit]);
        }
    }
    sum
}

/// How fit a player looks to the game (`FUN_00457e00`): his condition
/// in hundreds; an injured one with five or more points loses five, and
/// a badly injured one counts as nothing.
fn condition_points(player: &records::Player) -> i32 {
    let points = i32::from(player.condition()) / 100;
    if player.shown() & 0x800 == 0 {
        points
    } else if points < 5 {
        0
    } else {
        points - 5
    }
}

/// How fresh a player looks (`FUN_006ff649`): his three freshness
/// figures in hundreds, so at most 100; no figure at all counts as
/// none, which the game turns into a wild negative.
fn freshness_points(player: &records::Player) -> i32 {
    let [one, two, three] = player.freshness();
    let sum = i32::from(one) + i32::from(two) + i32::from(three);
    if sum < 0 {
        -0x1_0000
    } else if sum >= 10_001 {
        100
    } else {
        sum / 100
    }
}

/// The team's average of a player's figure, from the records: over the
/// eleven and the bench, or the squad if neither has anyone, or 50
/// (`FUN_006771b8`, `FUN_00676f16` — averages over the club's players).
fn average(records: &Records, team: &Team, of: fn(&records::Player) -> i32) -> i32 {
    let value = |id: u16| records.player(id).map_or(0, of);
    let picked: Vec<i32> =
        team.lineup().into_iter().chain(team.bench()).filter(|&id| id != 0).map(value).collect();
    if !picked.is_empty() {
        return picked.iter().sum::<i32>() / picked.len() as i32;
    }
    let squad = team.players();
    if squad.is_empty() {
        return 50;
    }
    squad.iter().map(|&id| value(id)).sum::<i32>() / squad.len() as i32
}
