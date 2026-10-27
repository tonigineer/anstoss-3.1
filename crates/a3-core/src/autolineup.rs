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

/// Puts up the variant chosen in the special dialog, and the bench after it
/// — the co-trainer's with the formation as it stands, in a system of his
/// choosing, or only filling the gaps (no bench then); whether there was
/// such a variant to put up.
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
                        if f.smoker { (condition - 5).max(0) } else { condition }
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
        v if CO_TRAINER.contains(&v) => {
            let mode = match v {
                8 => FOREIGNERS,
                9 => FREE | FOREIGNERS,
                _ => KEEP | FOREIGNERS,
            };
            co_trainer(tables, club, mode);
            if v != 10 {
                bench(tables, club);
            }
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
pub(crate) fn age(birthday: Option<NaiveDate>, today: NaiveDate) -> i32 {
    let Some(born) = birthday else { return 0 };
    let (day, month) = (born.day(), born.month());
    let had_birthday = month < today.month() || month == today.month() && day <= today.day();
    (today.year() - born.year() - 1 + i32::from(had_birthday)).max(0)
}

/// The co-trainer's line-ups (`FUN_0042c802`), by these bits: keep the
/// eleven and only fill its gaps; no more than three foreigners; choose the
/// system too, rather than keep the formation's spots.
pub const KEEP: u8 = 1;
pub const FOREIGNERS: u8 = 2;
pub const FREE: u8 = 4;
/// What the line-up's special settings ask of him before a match: a free
/// system, or the formation as it stands (the club's line-up flags).
pub const AUTO_FREE: u16 = 2;
pub const AUTO_OLD: u16 = 4;
/// How he likes a system (`FUN_004300b0`): strikers at most, —,
/// midfielders at most, defenders and strikers at least.
const MOST_STRIKERS: i32 = 3;
const MOST_MIDFIELD: i32 = 6;
const LEAST_DEFENCE: i32 = 3;
const LEAST_STRIKERS: i32 = 2;
const MOST_FOREIGNERS: i32 = 3;
/// The keeper's spot in a system he chooses.
const KEEPER_SPOT: (i16, i16) = (198, 216);
/// Where his tables are: the most of each kind, the defences of three to
/// five, the midfields of two to six, the attacks of one to three.
const MOST_AT: u32 = 0x8784f0;
const DEFENCES_AT: [u32; 3] = [0x8776b8, 0x8777a8, 0x877840];
const DEFENCE_ROW: u32 = 0x1e;
const MIDFIELDS_AT: [u32; 5] = [0x877890, 0x877a88, 0x877c80, 0x877ff8, 0x8782c8];
const MIDFIELD_ROW: u32 = 0x2a;
const ATTACKS_AT: u32 = 0x8774e8;
const ATTACK_ROW: u32 = 0x12;
const ATTACKS: usize = 3;
/// A table ends with the row whose first count is -1, the one taken when
/// none fits.
const LAST_ROW: i16 = -1;
const MOST_ROWS: usize = 40;
/// How he sorts players: those marked "always" first, then the rest, those
/// marked "never" last.
const ALWAYS_KEY: i32 = 0x3000;
const NORMAL_KEY: i32 = 0x2000;
const NEVER_KEY: i32 = 0x1000;

/// A system's part: how many of four kinds it has, and their spots.
#[derive(Clone, Debug, Default)]
pub(crate) struct Part {
    pub counts: [i16; 4],
    pub spots: Vec<(i16, i16)>,
}

#[derive(Clone, Debug, Default)]
pub struct CoTrainerTables {
    pub(crate) most: Vec<i16>,
    defences: Vec<Vec<Part>>,
    midfields: Vec<Vec<Part>>,
    pub(crate) attacks: Vec<Vec<(i16, i16)>>,
    /// Every size of defence, one to five, and of midfield, one to eight,
    /// as the list view's re-fit reads them (`crate::refit`).
    pub(crate) every_defence: Vec<Vec<Part>>,
    pub(crate) every_midfield: Vec<Vec<Part>>,
}

/// Where every size of defence and midfield is, from one.
const EVERY_DEFENCE_AT: [u32; 5] = [0x877530, 0x8775c8, 0x8776b8, 0x8777a8, 0x877840];
const EVERY_MIDFIELD_AT: [u32; 8] =
    [0x877860, 0x877890, 0x877a88, 0x877c80, 0x877ff8, 0x8782c8, 0x878498, 0x8784c4];

impl CoTrainerTables {
    pub fn load(exe: &a3_files::exe::Exe) -> std::io::Result<Self> {
        let shorts = |at: u32, n: usize| -> std::io::Result<Vec<i16>> {
            Ok(exe.u16s(at, n)?.into_iter().map(|v| v as i16).collect())
        };
        let parts = |at: u32, row: u32| -> std::io::Result<Vec<Part>> {
            let mut parts = Vec::new();
            for j in 0..MOST_ROWS as u32 {
                let e = shorts(at + j * row, row as usize / 2)?;
                let n = usize::try_from(e[0]).unwrap_or(0).min((e.len() - 5) / 2);
                let spots = (0..n).map(|i| (e[5 + 2 * i], e[6 + 2 * i])).collect();
                parts.push(Part { counts: [e[1], e[2], e[3], e[4]], spots });
                if e[1] == LAST_ROW {
                    break;
                }
            }
            Ok(parts)
        };
        let mut attacks = Vec::new();
        for k in 0..ATTACKS as u32 {
            let e = shorts(ATTACKS_AT + k * ATTACK_ROW, ATTACK_ROW as usize / 2)?;
            let n = usize::try_from(e[0]).unwrap_or(0).min((e.len() - 1) / 2);
            attacks.push((0..n).map(|i| (e[1 + 2 * i], e[2 + 2 * i])).collect());
        }
        Ok(Self {
            most: shorts(MOST_AT, FIELD)?,
            defences: DEFENCES_AT
                .iter()
                .map(|&at| parts(at, DEFENCE_ROW))
                .collect::<Result<_, _>>()?,
            midfields: MIDFIELDS_AT
                .iter()
                .map(|&at| parts(at, MIDFIELD_ROW))
                .collect::<Result<_, _>>()?,
            attacks,
            every_defence: EVERY_DEFENCE_AT
                .iter()
                .map(|&at| parts(at, DEFENCE_ROW))
                .collect::<Result<_, _>>()?,
            every_midfield: EVERY_MIDFIELD_AT
                .iter()
                .map(|&at| parts(at, MIDFIELD_ROW))
                .collect::<Result<_, _>>()?,
        })
    }
}

/// Whether a player may play the next match (`FUN_0069692b` for a club):
/// not suspended, not injured for a week, not away, not marked away, his
/// contract not run out.
pub fn may_play(s: &Status) -> bool {
    s.suspended == 0
        && (s.injured < INJURED_DAYS || s.injected)
        && s.away == 0
        && s.away_kind & MARKED_AWAY == 0
        && s.long == 0
        && s.contract.is_none_or(|years| years > 0)
}

/// Whether a player counts as a foreigner: neither of his nations is of the
/// European Union (`FUN_00706f44`). Without his nations, from a new game,
/// he does not.
fn foreigner(tables: &Tables, s: &Status) -> bool {
    let european = |n: u8| tables.european.get(usize::from(n)).copied().unwrap_or(false);
    let [first, second] = s.nations;
    first != 0 && !european(first) && (second == 0 || !european(second))
}

/// One player on one position, as the co-trainer weighs him.
struct Entry {
    player: usize,
    position: u8,
    key: i32,
    dropped: bool,
}

/// The co-trainer's line-up (`FUN_0042c802`): every player who may play
/// weighed on each position by his strength there — less when he is not
/// fresh — those marked "always" first, "never" last; in a free system the
/// keeper first, then as many of each kind as he likes, the system's spots
/// from his tables; then every spot from the best entry of its kind. The
/// bench he leaves empty; three foreigners play at most, where the club's
/// league counts them.
pub fn co_trainer(tables: &Tables, club: &mut ClubState, mode: u8) {
    let mut mode = if club.foreigner_rule { mode } else { mode & !FOREIGNERS };
    let count = club.players.len();
    if count < FIELD {
        mode &= !FREE;
    }
    if mode & KEEP == 0 {
        club.lineup.field = [None; FIELD];
    }
    club.lineup.bench = [None; BENCH];
    let status = |p: usize| club.status.get(p).copied().unwrap_or_default();
    let fitness = |p: usize| crate::lineup::fitness_of(&club.fitness, p);
    let mut spots = club.lineup.spots;
    spots[0] = KEEPER_SPOT;
    let mut used = vec![false; count];
    let mut foreigners = 0;
    for (p, taken) in used.iter_mut().enumerate() {
        if !may_play(&status(p)) {
            *taken = true;
            club.lineup.remove(p);
        } else if club.lineup.playing(p) {
            *taken = true;
            if foreigner(tables, &status(p)) {
                foreigners += 1;
            }
        }
    }
    // Every free player on every position, best first; a tie goes after
    // those already in.
    let mut list: Vec<Entry> = Vec::new();
    for position in 1..FIELD as u8 {
        for p in (0..count).filter(|&p| !used[p]) {
            let mut value = tables.strength_at(&club.players[p], fitness(p), position);
            let fresh = fresh_percent(fitness(p).freshness);
            if fresh == 0 {
                value /= 5;
            } else if fresh < 30 {
                value /= 2;
            }
            let s = status(p);
            let mark = if s.always {
                ALWAYS_KEY
            } else if s.never {
                NEVER_KEY
            } else {
                NORMAL_KEY
            };
            let key = value | mark;
            let at = list.iter().position(|e| e.key < key).unwrap_or(list.len());
            list.insert(at, Entry { player: p, position, key, dropped: false });
        }
    }
    let drop_player = |list: &mut [Entry], p: usize| {
        list.iter_mut().filter(|e| e.player == p).for_each(|e| e.dropped = true);
    };
    let drop_foreigners = |list: &mut [Entry]| {
        for e in list.iter_mut() {
            if foreigner(tables, &status(e.player)) {
                e.dropped = true;
            }
        }
    };
    // A foreigner taken counts; the third keeps the others out.
    let take_foreigner = |list: &mut [Entry], foreigners: &mut i32, p: usize| {
        if mode & FOREIGNERS != 0 && foreigner(tables, &status(p)) {
            *foreigners += 1;
            if *foreigners >= MOST_FOREIGNERS {
                drop_foreigners(list);
            }
        }
    };
    if mode & FREE != 0 {
        if mode & FOREIGNERS != 0 && foreigners >= MOST_FOREIGNERS {
            drop_foreigners(&mut list);
        }
        let mut c = [0i32; FIELD];
        let keeper = list
            .iter()
            .find(|e| !e.dropped && club.players[e.player].positions[0] == GOALKEEPER)
            .map(|e| e.player);
        if let Some(keeper) = keeper {
            drop_player(&mut list, keeper);
            c[1] = 1;
            take_foreigner(&mut list, &mut foreigners, keeper);
        }
        let most = &tables.co_trainer.most;
        let (mut n, mut pending) = (0i32, false);
        let mut i = 0;
        while i < list.len() {
            let (p, pos) = (list[i].player, usize::from(list[i].position));
            i += 1;
            if list[i - 1].dropped || c[pos] >= i32::from(most.get(pos).copied().unwrap_or(0)) {
                continue;
            }
            let left = 10 - n;
            let defence = c[2] + c[3] + c[4] + c[5];
            let midfield = c[6] + c[7] + c[8] + c[9];
            let (left_side, right_side) = (c[4] + c[7], c[5] + c[6]);
            let behind = midfield < MOST_MIDFIELD
                && defence + left > LEAST_DEFENCE
                && c[10] + left >= LEAST_STRIKERS
                && defence + c[10] + left > LEAST_DEFENCE + LEAST_STRIKERS;
            let mut restart = false;
            let mut side = |mine: i32, other: i32, also: bool| -> bool {
                if mine < 2 && c[10] + left > LEAST_STRIKERS && also {
                    if mine > other {
                        pending = true;
                        return false;
                    }
                    restart = mine < other && pending;
                    return true;
                }
                false
            };
            let ok = match pos {
                2 | 3 => c[10] + left > LEAST_STRIKERS,
                4 => side(left_side, right_side, true),
                5 => side(right_side, left_side, true),
                7 => side(left_side, right_side, behind),
                6 => side(right_side, left_side, behind),
                8 | 9 => behind,
                10 => c[10] < MOST_STRIKERS && defence + left > LEAST_DEFENCE,
                _ => false,
            };
            if !ok {
                continue;
            }
            drop_player(&mut list, p);
            n += 1;
            c[pos] += 1;
            take_foreigner(&mut list, &mut foreigners, p);
            if n >= 10 {
                break;
            }
            if restart {
                pending = false;
                i = 0;
            }
        }
        if c[1] == 0 {
            // No keeper: the best left keeps goal.
            if let Some(first) = list.iter().find(|e| !e.dropped).map(|e| e.player) {
                drop_player(&mut list, first);
                c[1] = 1;
            }
        }
        if n < 10 && c[2] == 0 {
            c[2] = 1;
            n += 1;
        }
        if n < 10 && c[10] < MOST_STRIKERS {
            c[10] += 1;
            n += 1;
        }
        for k in (6..=9).rev() {
            if n < 10 && c[k] < 2 {
                c[k] += 1;
                n += 1;
            }
        }
        // The system's spots, from slot 1 on: defence, midfield, attack.
        let mut slot = 1;
        let mut put = |part: &[(i16, i16)]| {
            for &spot in part {
                if slot < FIELD {
                    spots[slot] = spot;
                    slot += 1;
                }
            }
        };
        let pick = |rows: &[Part], wanted: [i32; 4]| -> Vec<(i16, i16)> {
            rows.iter()
                .find(|r| r.counts[0] == LAST_ROW || r.counts.map(i32::from) == wanted)
                .map_or_else(Vec::new, |r| r.spots.clone())
        };
        let defence = (c[2] + c[3] + c[4] + c[5]) as usize;
        if let Some(rows) = defence.checked_sub(3).and_then(|d| tables.co_trainer.defences.get(d)) {
            put(&pick(rows, [c[2], c[3], c[4], c[5]]));
        }
        let midfield = (c[6] + c[7] + c[8] + c[9]) as usize;
        if let Some(rows) = midfield.checked_sub(2).and_then(|m| tables.co_trainer.midfields.get(m))
        {
            put(&pick(rows, [c[6], c[7], c[8], c[9]]));
        }
        if let Some(attack) =
            (c[10] as usize).checked_sub(1).and_then(|a| tables.co_trainer.attacks.get(a))
        {
            put(attack);
        }
        list.iter_mut().for_each(|e| e.dropped = false);
    }
    // The spots are filled anew: only those kept count as foreigners.
    let kept = club.lineup.field.iter().flatten().filter(|&&p| foreigner(tables, &status(p)));
    foreigners = kept.count() as i32;
    club.lineup.spots = spots;
    // Every free spot from the best entry of its kind.
    let mut free: Vec<Vec<usize>> = vec![Vec::new(); FIELD];
    for (slot, &spot) in spots.iter().enumerate() {
        if club.lineup.field[slot].is_none()
            && let Some(group) = free.get_mut(usize::from(tables.kind_under(spot)))
        {
            group.push(slot);
        }
    }
    let mut filled = [0; FIELD];
    let mut i = 0;
    while i < list.len() {
        let (p, pos) = (list[i].player, usize::from(list[i].position));
        let dropped = list[i].dropped;
        i += 1;
        if dropped || filled[pos] >= free[pos].len() {
            continue;
        }
        club.lineup.field[free[pos][filled[pos]]] = Some(p);
        filled[pos] += 1;
        if filled[pos] >= free[pos].len() {
            list.iter_mut()
                .filter(|e| usize::from(e.position) == pos)
                .for_each(|e| e.dropped = true);
        }
        drop_player(&mut list, p);
        take_foreigner(&mut list, &mut foreigners, p);
    }
    club.strengths = None;
}

/// Freshness in percent (`FUN_006ff649`).
fn fresh_percent(freshness: [i32; 3]) -> i32 {
    let fresh: i32 = freshness.iter().sum();
    if fresh < 0 {
        0
    } else if fresh <= 10_000 {
        fresh / 100
    } else {
        100
    }
}

/// Before a match, if the manager left it to his co-trainer: the line-up
/// the line-up's special settings ask for, and the bench as "Auto." fills
/// it; then whoever may not play leaves the eleven and the bench
/// (`FUN_00652a86`, `FUN_00696631`).
pub fn before_match(tables: &Tables, club: &mut ClubState) {
    if club.co_trainer && club.line_up_flags & (AUTO_FREE | AUTO_OLD) != 0 {
        let mode = if club.line_up_flags & AUTO_OLD != 0 { FOREIGNERS } else { FREE | FOREIGNERS };
        co_trainer(tables, club, mode);
        bench(tables, club);
    }
    for p in 0..club.players.len() {
        if !may_play(&club.status.get(p).copied().unwrap_or_default()) {
            club.lineup.remove(p);
        }
    }
}
