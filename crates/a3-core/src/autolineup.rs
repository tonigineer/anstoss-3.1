//! The line-ups the game makes itself: "Auto." fills the bench
//! (`FUN_0042a828`), and the line-up's special variants (`CMannaufSpezial`,
//! dialog 386) put up a whole eleven by one measure — fewest matches,
//! condition, freshness, morale, age or strength — then the bench as "Auto."
//! does. A variant fills each spot with the best player left whose main
//! position is the kind of position the spot makes, and leaves it empty
//! when there is none. Players the manager marked "never" stay out; those
//! marked "always" come first.

use chrono::{Datelike, NaiveDate};

use crate::lineup::{BENCH, FIELD, Tables};
use crate::season::{ClubState, Status};
use crate::world::GOALKEEPER;

/// A player injured for this many days or more cannot sit on the bench.
const INJURED_DAYS: i32 = 7;
/// An absence marked so keeps him off the bench too.
const MARKED_AWAY: u8 = 0x10;

/// The variants, in the order the dialog offers them.
pub const FEWEST_MATCHES: usize = 0;
pub const FITTEST: usize = 1;
pub const FRESHEST: usize = 2;
pub const MOST_MOTIVATED: usize = 3;
pub const LEAST_MOTIVATED: usize = 4;
pub const YOUNG: usize = 5;
pub const EXPERIENCED: usize = 6;
pub const SAFE: usize = 7;
/// Three by the co-trainer, with the current formation, a free one, or
/// filling the gaps.
pub const CO_TRAINER: [usize; 3] = [8, 9, 10];
pub const CLEAR: usize = 11;

/// The bench, as "Auto." fills it: the best keeper, then field players —
/// by their strength on their main position and what their abilities and
/// character add — and keepers again for what is left, which leaves the
/// last place empty (the game's loop stops one short). Whoever is injured
/// for a week, suspended, away, at the end of his contract, marked "never"
/// or in the eleven does not sit there.
pub fn bench(tables: &Tables, club: &mut ClubState) {
    club.lineup.bench = [None; BENCH];
    let status = |p: usize| club.status.get(p).copied().unwrap_or_default();
    let fitness = |p: usize| crate::lineup::fitness_of(&club.fitness, p);
    let mut used: Vec<bool> =
        (0..club.players.len()).map(|p| !benchable(&status(p)) || club.lineup.playing(p)).collect();
    let ranked = |used: &[bool], keepers: bool| -> Vec<usize> {
        let mut found: Vec<(usize, i32)> = (0..club.players.len())
            .filter(|&p| !used[p] && (club.players[p].positions[0] == GOALKEEPER) == keepers)
            .map(|p| {
                let player = &club.players[p];
                let position = if keepers { GOALKEEPER } else { player.positions[0] };
                (p, tables.rank(player, fitness(p), position))
            })
            .collect();
        // Best first; a tie keeps the squad's order.
        found.sort_by_key(|&(_, rank)| std::cmp::Reverse(rank));
        found.into_iter().map(|(p, _)| p).collect()
    };
    let mut place = 0;
    if let Some(&keeper) = ranked(&used, true).first() {
        club.lineup.bench[0] = Some(keeper);
        used[keeper] = true;
        place = 1;
    }
    for p in ranked(&used, false) {
        club.lineup.bench[place] = Some(p);
        used[p] = true;
        place += 1;
        if place == BENCH {
            return;
        }
    }
    for p in ranked(&used, true) {
        if place >= BENCH - 1 {
            return;
        }
        club.lineup.bench[place] = Some(p);
        place += 1;
    }
}

fn benchable(s: &Status) -> bool {
    s.injured < INJURED_DAYS
        && s.suspended == 0
        && s.away == 0
        && s.long == 0
        && s.away_kind & MARKED_AWAY == 0
        && s.contract.is_none_or(|years| years > 0)
        && !s.never
}

/// Puts up the variant chosen in the special dialog, and the bench after it;
/// whether there was such a variant to put up (the co-trainer's are not
/// built yet).
pub fn variant(tables: &Tables, club: &mut ClubState, variant: usize, today: NaiveDate) -> bool {
    let status = |p: usize| club.status.get(p).copied().unwrap_or_default();
    let fitness = |p: usize| crate::lineup::fitness_of(&club.fitness, p);
    // Who may play: everyone but those marked "never" (`FUN_005129e3` …;
    // before a match the game also leaves out who is out).
    let squad: Vec<usize> = (0..club.players.len()).filter(|&p| !status(p).never).collect();
    let main = |p: usize| club.players[p].positions[0];
    let field = match variant {
        FEWEST_MATCHES => {
            // Fewest matches first, those marked "always" before the rest.
            let key = |p: usize| {
                let mark = if status(p).always { 0 } else { 0x2000 };
                i32::from(status(p).matches) | mark
            };
            by_buckets(tables, club, &squad, key, false)
        }
        SAFE => {
            // The strongest first, those marked "always" before the rest.
            let key = |p: usize| {
                let mark = if status(p).always { 0x3000 } else { 0x2000 };
                i32::from(club.players[p].strength) | mark
            };
            by_buckets(tables, club, &squad, key, true)
        }
        FITTEST | FRESHEST | MOST_MOTIVATED | LEAST_MOTIVATED | YOUNG | EXPERIENCED => {
            let key = |p: usize| -> i32 {
                let f = fitness(p);
                match variant {
                    FITTEST => {
                        let condition = f.condition / 100;
                        if f.worn { (condition - 5).max(0) } else { condition }
                    }
                    FRESHEST => {
                        let fresh: i32 = f.freshness.iter().sum();
                        if fresh < 0 {
                            0
                        } else if fresh <= 10_000 {
                            fresh / 100
                        } else {
                            100
                        }
                    }
                    MOST_MOTIVATED => f.morale,
                    LEAST_MOTIVATED => -f.morale,
                    YOUNG => -age(club.players[p].birthday, today),
                    _ => age(club.players[p].birthday, today),
                }
            };
            // By main position, those marked "always" first, then the best
            // (`FUN_00512195`), every spot from its position's run
            // (`FUN_0051259d`).
            let mut rows: Vec<(u8, bool, i32, usize)> =
                squad.iter().map(|&p| (main(p), status(p).always, key(p), p)).collect();
            let better = |a: &(u8, bool, i32, usize), b: &(u8, bool, i32, usize)| {
                a.0 < b.0 || a.0 == b.0 && (a.1 && !b.1 || a.1 == b.1 && a.2 > b.2)
            };
            exchange_sort(&mut rows, better);
            fill(tables, club, |kind| rows.iter().filter(move |r| r.0 == kind).map(|r| r.3))
        }
        CLEAR => {
            club.lineup.field = [None; FIELD];
            club.lineup.bench = [None; BENCH];
            return true;
        }
        _ => return false,
    };
    club.lineup.field = field;
    bench(tables, club);
    true
}

/// The squad by main position, each bucket in the order of `key` — the
/// highest first, or the lowest — and every spot from its kind's bucket
/// (`FUN_00511674`, `FUN_00511d01`).
fn by_buckets(
    tables: &Tables,
    club: &ClubState,
    squad: &[usize],
    key: impl Fn(usize) -> i32,
    highest: bool,
) -> [Option<usize>; FIELD] {
    let mut rows: Vec<(u8, i32, usize)> =
        squad.iter().map(|&p| (club.players[p].positions[0], key(p), p)).collect();
    exchange_sort(&mut rows, |a, b| {
        a.0 < b.0 || a.0 == b.0 && if highest { a.1 > b.1 } else { a.1 < b.1 }
    });
    fill(tables, club, |kind| rows.iter().filter(move |r| r.0 == kind).map(|r| r.2))
}

/// Every spot of the eleven from the players of its kind of position, in
/// their order, each taken once; a spot without one stays empty.
fn fill<'a, I: Iterator<Item = usize> + 'a>(
    tables: &Tables,
    club: &ClubState,
    of_kind: impl Fn(u8) -> I,
) -> [Option<usize>; FIELD] {
    let mut taken: Vec<usize> = Vec::new();
    let mut field = [None; FIELD];
    for (place, &spot) in field.iter_mut().zip(&club.lineup.spots) {
        let kind = tables.kind_under(spot);
        *place = of_kind(kind).find(|p| !taken.contains(p));
        taken.extend(*place);
    }
    field
}

/// The game's sort: every later row swapped in front that comes before it.
fn exchange_sort<T>(rows: &mut [T], before: impl Fn(&T, &T) -> bool) {
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            if before(&rows[j], &rows[i]) {
                rows.swap(i, j);
            }
        }
    }
}

/// How old a player is on a day, as the game counts it (`FUN_0067582a`).
fn age(birthday: Option<NaiveDate>, today: NaiveDate) -> i32 {
    let Some(born) = birthday else { return 0 };
    let (day, month) = (born.day(), born.month());
    let had_birthday = month < today.month() || month == today.month() && day <= today.day();
    (today.year() - born.year() - 1 + i32::from(had_birthday)).max(0)
}
