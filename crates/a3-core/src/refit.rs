//! The line-up as a list (dialog 429, `CMannaufListe`; the RE repo's
//! LINEUP-LIST.md §4 and §5). A click on a player's row takes him into the
//! eleven, onto the bench or out of the line-up; a player who comes into the
//! eleven has the game fit the whole eleven anew (`FUN_00507a31`): every
//! player back into his line by his main position, the best keeper in goal,
//! each line's size and spots changed where they must be, the spots filled
//! from the co-trainer's tables.

use crate::autolineup::Part;
use crate::lineup::{FIELD, Tables, fitness_of};
use crate::season::ClubState;
use crate::world::GOALKEEPER;

/// How many of each line the game allows: strikers, defenders, midfielders.
const MOST_ATTACK: i32 = 3;
const MOST_DEFENCE: i32 = 5;
const MOST_MIDFIELD: i32 = 8;
/// With more midfielders than that, a line of fewer than these gets two of
/// them.
const LEAST_DEFENCE: i32 = 4;
const LEAST_ATTACK: i32 = 2;
/// How the line's fill sorts the players, as the co-trainer does: those
/// marked "always" first, then the rest, those marked "never" last.
const ALWAYS_KEY: i32 = 0x3000;
const NORMAL_KEY: i32 = 0x2000;
const NEVER_KEY: i32 = 0x1000;

/// A click on a player's row (`OnSelChange`, 0x504fa8): out of the eleven
/// onto the first free place of the bench (or just out, with the bench
/// full), off the bench, or — from the rest of the squad — into the first
/// empty slot of the eleven, the eleven fitted anew, or else onto the bench.
/// In the week's turn anyone may come in; before a match the game would
/// first turn away who may not play.
pub fn click(tables: &Tables, club: &mut ClubState, player: usize) {
    let lineup = &mut club.lineup;
    let bench_place = |bench: &[Option<usize>]| bench.iter().position(Option::is_none);
    if lineup.playing(player) {
        lineup.remove(player);
        if let Some(free) = bench_place(&lineup.bench) {
            lineup.bench[free] = Some(player);
        }
    } else if lineup.on_bench(player) {
        lineup.remove(player);
    } else if let Some(empty) = lineup.field.iter().position(Option::is_none) {
        let mut want = lineup.field;
        want[empty] = Some(player);
        refit(tables, club, want);
    } else if let Some(free) = bench_place(&lineup.bench) {
        lineup.bench[free] = Some(player);
    }
}

/// The three lines behind the keeper, in the order they take the slots.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Defence,
    Midfield,
    Attack,
}

impl Kind {
    /// The positions of the line.
    fn positions(self) -> std::ops::RangeInclusive<u8> {
        match self {
            Kind::Defence => 2..=5,
            Kind::Midfield => 6..=9,
            Kind::Attack => 10..=10,
        }
    }

    /// The line a position, or the kind of a spot, belongs to: anything
    /// past the midfield to the attack.
    fn of(position: u8) -> Self {
        match position {
            2..=5 => Kind::Defence,
            6..=9 => Kind::Midfield,
            _ => Kind::Attack,
        }
    }
}

/// A line as the re-fit builds it: its spots — the formation's, or new ones
/// — and who stands on them.
struct Line {
    kind: Kind,
    spots: [(i16, i16); FIELD],
    ids: [Option<usize>; FIELD],
}

/// One player on one position of a line, as the fill weighs him.
struct Entry {
    player: usize,
    position: u8,
    key: i32,
    dropped: bool,
}

/// The eleven fitted anew to the players wanted in it (`FUN_00507a31`).
/// Slot 0 keeps its spot; the lines follow from slot 1, defence, midfield,
/// attack, each in its spots' order. A player no line has a spot for is out
/// of the eleven, not on the bench. The formation itself does not change.
pub fn refit(tables: &Tables, club: &mut ClubState, want: [Option<usize>; FIELD]) {
    let fitness = |p: usize| fitness_of(&club.fitness, p);
    let es = |p: usize| {
        let doping = club.status.get(p).map_or(0, |s| s.doping);
        tables.strength(&club.players[p], fitness(p), doping)
    };
    let mut lines = [Kind::Defence, Kind::Midfield, Kind::Attack].map(|kind| Line {
        kind,
        spots: [(0, 0); FIELD],
        ids: [None; FIELD],
    });
    let mut spots = [0i32; 3];
    // The keepers by strength, best first; the rest by line, in slot order.
    let mut keepers: Vec<usize> = Vec::new();
    let mut groups: [Vec<usize>; 3] = Default::default();
    for (slot, wanted) in want.iter().enumerate() {
        let spot = club.lineup.spots[slot];
        let kind = tables.kind_under(spot);
        if kind != GOALKEEPER {
            let line = Kind::of(kind) as usize;
            lines[line].spots[spots[line] as usize] = spot;
            spots[line] += 1;
        }
        if let Some(p) = wanted.filter(|&p| p < club.players.len()) {
            match club.players[p].positions[0] {
                GOALKEEPER => {
                    let at = keepers.iter().position(|&k| es(k) < es(p)).unwrap_or(keepers.len());
                    keepers.insert(at, p);
                }
                main => groups[Kind::of(main) as usize].push(p),
            }
        }
        club.lineup.field[slot] = None;
    }
    let [mut sd, mut sm, mut sa] = spots;
    let [mut td, mut tm, mut ta] = groups.each_ref().map(|g| g.len() as i32);
    if let Some((&best, rest)) = keepers.split_first() {
        club.lineup.field[0] = Some(best);
        groups[2].extend(rest);
    }
    let [defence, midfield, attack] = &mut lines;
    let fill = |line: &mut Line, n: i32, group: &[usize], reshape: bool| {
        fill_line(tables, club, line, n, group, reshape);
    };
    // Too many strikers or defenders: the best stay, the others go to the
    // midfield; too many midfielders, two of them to a thin line.
    if ta > MOST_ATTACK {
        tm += ta - MOST_ATTACK;
        ta = MOST_ATTACK;
        fill(attack, MOST_ATTACK, &groups[2], MOST_ATTACK > sa);
        unpicked(attack, &mut groups, 2, 1);
    }
    if td > MOST_DEFENCE {
        tm += td - MOST_DEFENCE;
        td = MOST_DEFENCE;
        fill(defence, MOST_DEFENCE, &groups[0], MOST_DEFENCE > sd);
        unpicked(defence, &mut groups, 0, 1);
    }
    if tm > MOST_MIDFIELD && td < LEAST_DEFENCE {
        td += 2;
        tm -= 2;
        fill(midfield, tm, &groups[1], tm > sm);
        unpicked(midfield, &mut groups, 1, 0);
    }
    if tm > MOST_MIDFIELD && ta < LEAST_ATTACK {
        ta += 2;
        tm -= 2;
        fill(midfield, tm, &groups[1], tm > sm);
        unpicked(midfield, &mut groups, 1, 2);
    }
    // Each line as many spots as players, taken from the midfield first.
    let (mut rd, mut rm, mut ra) = (false, false, false);
    while sd < td {
        (rd, sd) = (true, sd + 1);
        if sm > tm {
            (sm, rm) = (sm - 1, true);
        } else {
            (sa, ra) = (sa - 1, true);
        }
    }
    while sa < ta {
        (ra, sa) = (true, sa + 1);
        if sm > tm {
            (sm, rm) = (sm - 1, true);
        } else {
            (sd, rd) = (sd - 1, true);
        }
    }
    while sm < tm {
        (rm, sm) = (true, sm + 1);
        if sd > td {
            (sd, rd) = (sd - 1, true);
        } else {
            (sa, ra) = (sa - 1, true);
        }
    }
    for (line, n, group, reshape) in
        [(&mut *defence, sd, 0, rd), (&mut *midfield, sm, 1, rm), (&mut *attack, sa, 2, ra)]
    {
        if reshape {
            fill(line, n, &groups[group], true);
        }
    }
    fill(defence, sd, &groups[0], false);
    fill(midfield, sm, &groups[1], false);
    fill(attack, sa, &groups[2], false);
    // The game writes on past slot 10 when slot 0 is no keeper's spot;
    // every formation of the game starts with the keeper.
    let mut slot = 1;
    for (line, n) in [(&*defence, sd), (&*midfield, sm), (&*attack, sa)] {
        for j in 0..usize::try_from(n).unwrap_or(0).min(FIELD) {
            if slot < FIELD {
                club.lineup.field[slot] = line.ids[j];
                club.lineup.spots[slot] = line.spots[j];
            }
            slot += 1;
        }
    }
}

/// Those of a line's group it did not pick go to another group's end
/// (`FUN_0050786c`); the group keeps the picked, in order.
fn unpicked(line: &Line, groups: &mut [Vec<usize>; 3], from: usize, to: usize) {
    let picked = |p: &usize| line.ids.contains(&Some(*p));
    let (kept, moved): (Vec<usize>, Vec<usize>) = groups[from].iter().partition(|p| picked(p));
    groups[from] = kept;
    groups[to].extend(moved);
}

/// One line filled (`FUN_0042dbe7` defence, `FUN_0042e36f` midfield,
/// `FUN_0042ebd4` attack): every player of its group on each of its
/// positions, weighed by his strength there; reshaped, as many of each
/// position as the best entries ask for and the co-trainer allows, the
/// spots from his tables; then each of the first `n` spots from the best
/// entry of its kind.
fn fill_line(
    tables: &Tables,
    club: &ClubState,
    line: &mut Line,
    n: i32,
    group: &[usize],
    reshape: bool,
) {
    let mut entries: Vec<Entry> = Vec::new();
    for &p in group {
        let status = club.status.get(p).copied().unwrap_or_default();
        let mark = if status.always {
            ALWAYS_KEY
        } else if status.never {
            NEVER_KEY
        } else {
            NORMAL_KEY
        };
        for position in line.kind.positions() {
            let value =
                tables.strength_at(&club.players[p], fitness_of(&club.fitness, p), position);
            let key = value | mark;
            let at = entries.iter().position(|e| e.key < key).unwrap_or(entries.len());
            entries.insert(at, Entry { player: p, position, key, dropped: false });
        }
    }
    let drop_player = |entries: &mut [Entry], p: usize| {
        entries.iter_mut().filter(|e| e.player == p).for_each(|e| e.dropped = true);
    };
    line.ids = [None; FIELD];
    let co_trainer = &tables.co_trainer;
    if reshape {
        let most = |pos: u8| i32::from(co_trainer.most.get(usize::from(pos)).copied().unwrap_or(0));
        let mut c = [0i32; FIELD];
        let mut t = 0;
        for i in 0..entries.len() {
            let (p, pos) = (entries[i].player, entries[i].position);
            if entries[i].dropped || c[usize::from(pos)] >= most(pos) {
                continue;
            }
            drop_player(&mut entries, p);
            c[usize::from(pos)] += 1;
            t += 1;
            if t >= n {
                break;
            }
        }
        match line.kind {
            Kind::Defence => {
                for (pos, below) in [(2, 1), (3, 2), (3, 2), (4, 1), (5, 1)] {
                    if t < n && c[pos] < below {
                        c[pos] += 1;
                        t += 1;
                    }
                }
            }
            Kind::Midfield => {
                while t < n {
                    let Some(pos) =
                        (6..=9).find(|&k| c[k] == 0).or_else(|| (6..=9).find(|&k| c[k] == 1))
                    else {
                        break;
                    };
                    c[pos] += 1;
                    t += 1;
                }
            }
            Kind::Attack => {}
        }
        // The first row of the size with these counts, or the closing one.
        let row = |rows: &[Vec<Part>], counts: [i32; 4]| -> Option<Vec<(i16, i16)>> {
            let sum: i32 = counts.iter().sum();
            let rows = usize::try_from(sum - 1).ok().and_then(|s| rows.get(s))?;
            let row = rows.iter().find(|r| r.counts[0] == -1 || r.counts.map(i32::from) == counts);
            row.map(|r| r.spots.clone())
        };
        let new_spots = match line.kind {
            Kind::Defence => row(&co_trainer.every_defence, [c[2], c[3], c[4], c[5]]),
            Kind::Midfield => row(&co_trainer.every_midfield, [c[6], c[7], c[8], c[9]]),
            Kind::Attack => {
                usize::try_from(n - 1).ok().and_then(|a| co_trainer.attacks.get(a)).cloned()
            }
        };
        // The row for eight midfielders lists seven spots: the eighth stays.
        for (spot, new) in line.spots.iter_mut().zip(new_spots.unwrap_or_default()) {
            *spot = new;
        }
        entries.iter_mut().for_each(|e| e.dropped = false);
    }
    let n = usize::try_from(n).unwrap_or(0).min(FIELD);
    let mut free: Vec<Vec<usize>> = vec![Vec::new(); FIELD];
    for (j, &spot) in line.spots.iter().enumerate().take(n) {
        if let Some(kind) = free.get_mut(usize::from(tables.kind_under(spot))) {
            kind.push(j);
        }
    }
    let mut filled = [0; FIELD];
    for i in 0..entries.len() {
        let (p, pos) = (entries[i].player, usize::from(entries[i].position));
        if entries[i].dropped || filled[pos] >= free[pos].len() {
            continue;
        }
        line.ids[free[pos][filled[pos]]] = Some(p);
        filled[pos] += 1;
        if filled[pos] == free[pos].len() {
            entries
                .iter_mut()
                .filter(|e| usize::from(e.position) == pos)
                .for_each(|e| e.dropped = true);
        }
        drop_player(&mut entries, p);
    }
}
