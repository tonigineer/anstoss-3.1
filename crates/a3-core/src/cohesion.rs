//! How well a team plays together — "Eingespieltheit", from 0 to 200 — as
//! the original reckons it for a human club (`FUN_0069774b`): how long the
//! best sixteen of the squad have been at the club, each of the eleven on
//! his own position or a foreign one, friends and enemies side by side,
//! the matches the eleven have played, and what training and the manager's
//! time at the club add; less one for each of the sixteen injured for a
//! week or more.
//!
//! Not yet: the twenty it costs when the formation is not the last match's
//! (the club object's +0xa97, +0xa98).

use chrono::{Datelike, NaiveDate};

use crate::lineup::{self, FIELD, Tables};
use crate::season::ClubState;

/// How many players the base takes, and the most months one counts with.
const BEST: usize = 16;
const MOST_MONTHS: i32 = 60;
/// A player on his main position adds, one on a foreign one takes away;
/// friends side by side add, enemies take away.
const OWN_POSITION: i32 = 2;
const FRIENDS: i32 = 3;
/// Injured for more than this many days.
const INJURED_DAYS: i32 = 6;
/// The positions, keeper to striker.
const POSITIONS: usize = 11;
const MOST: i32 = 200;
/// Who has no record of joining counts the months since this day of the
/// year: 1 July of 1997, a common year (the game's day count leaves the
/// year aside).
const NO_HISTORY: i32 = 181;

pub fn of(tables: &Tables, club: &ClubState, today: NaiveDate) -> i32 {
    let status = |p: usize| club.status.get(p).copied().unwrap_or_default();
    let form = |p: usize| lineup::form(lineup::fitness_of(&club.fitness, p));
    let strength = |p: usize| i32::from(club.players[p].strength);
    // Each main position's players, the strongest first (`FUN_006975c3`):
    // one of equal strength goes before another only if their forms are
    // equal too — the game compares them so.
    let mut lists: Vec<Vec<usize>> = vec![Vec::new(); POSITIONS];
    for p in 0..club.players.len() {
        let position = usize::from(club.players[p].positions[0]).min(POSITIONS - 1);
        let list = &mut lists[position];
        let mut at = list.len();
        while at > 0
            && (strength(list[at - 1]) < strength(p)
                || strength(list[at - 1]) == strength(p) && form(list[at - 1]) == form(p))
        {
            at -= 1;
        }
        list.insert(at, p);
    }
    // The best of each position, then the best of the rest, up to sixteen.
    let mut chosen: Vec<usize> =
        lists[1..].iter().filter_map(|list| list.first().copied()).collect();
    let mut next = [1; POSITIONS];
    while chosen.len() < BEST {
        let mut best: Option<(usize, i32, i32)> = None;
        for position in 1..POSITIONS {
            let Some(&p) = lists[position].get(next[position]) else { continue };
            let key = (strength(p), form(p));
            if best.is_none_or(|(_, s, f)| key.0 > s || key.0 == s && key.1 > f) {
                best = Some((position, key.0, key.1));
            }
        }
        let Some((position, ..)) = best else { break };
        chosen.push(lists[position][next[position]]);
        next[position] += 1;
    }
    if chosen.is_empty() {
        return 0;
    }
    let months: i32 =
        chosen.iter().map(|&p| months_at_club(status(p).joined, today).min(MOST_MONTHS)).sum();
    let injured = chosen.iter().filter(|&&p| status(p).injured > INJURED_DAYS).count() as i32;
    let mut cohesion = months * 10 / chosen.len() as i32 / 6;
    // The eleven.
    let lineup = &club.lineup;
    let (mut matches, mut players) = (0, 0);
    for i in 0..FIELD {
        let Some(p) = lineup.field[i] else { continue };
        let kind = tables.kind_under(lineup.spots[i]);
        let [main, second, third] = club.players[p].positions;
        if main == kind {
            cohesion += OWN_POSITION;
        } else if second != kind && third != kind {
            cohesion -= OWN_POSITION;
        }
        let s = status(p);
        for other in lineup.field[i + 1..].iter().flatten() {
            let id = status(*other).id;
            if id == 0 {
                continue;
            }
            if s.friends.contains(&id) {
                cohesion += FRIENDS;
            } else if s.enemies.contains(&id) {
                cohesion -= FRIENDS;
            }
        }
        let [a, b] = s.matches_less;
        matches += i32::from(s.matches) - i32::from(a) - i32::from(b);
        players += 1;
    }
    if players > 0 {
        cohesion += matches * 3 / players;
    }
    (cohesion + club.cohesion_bonus - injured).clamp(0, MOST)
}

/// Calendar months at the club, counting the first and the last
/// (`FUN_006ff5d5`).
fn months_at_club(joined: Option<NaiveDate>, today: NaiveDate) -> i32 {
    match joined {
        Some(joined) => {
            (today.year() - joined.year()) * 12 + today.month() as i32 - joined.month() as i32 + 1
        }
        None => (today.ordinal0() as i32 - NO_HISTORY) / 30,
    }
}

/// What a human club's training and its manager's time there add: the
/// training's part above 30 halved, and for a manager who signed before
/// July of the year (by the day of the year) a few for each week since
/// (`FUN_0069774b`).
pub fn bonus(training: i32, signed: Option<NaiveDate>, start_year: i32) -> i32 {
    let training = if training < 30 { training } else { 30 + (training - 30) / 2 };
    let july = NaiveDate::from_ymd_opt(start_year, 7, 1).map_or(181, |d| d.ordinal0() as i32);
    let tenure = match signed.map(|d| d.ordinal0() as i32) {
        Some(day) if day < july => {
            let weeks = (july - day) * 4 / 7;
            // The game halves above 40, but from 30 on.
            if weeks < 30 { weeks } else { 40 + (weeks - 40) / 2 }
        }
        _ => 0,
    };
    training + tenure
}
