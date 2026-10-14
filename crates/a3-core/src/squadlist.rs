//! The order of the line-up's squad list, by the column title clicked last
//! (`CMannschaftsaufstellung`'s fill `FUN_0050ada7`, its sort `FUN_0050f112`
//! and comparator `FUN_0050f310`; the RE repo's LINEUP-SORT.md). The list
//! starts from the squad by position and sorts it as the original does,
//! with an exchange sort that is not stable — the order it starts from
//! shows through on ties, and some orders put equal players either way.

use chrono::NaiveDate;

use crate::lineup::{self, Tables};
use crate::season::ClubState;
use crate::world::{GOALKEEPER, Player};

/// The column titles: Name, Pos, NPs, Fähigk., St., Fo., Mo., ES, Rn.,
/// Alter, Fällt aus; and the season's row of them: Name, Pos, Nat, DForm,
/// Ko, Fr, Eins., Tore, Zuf., Stamm.
pub const COLUMNS: usize = 11;
pub const STATS_COLUMNS: usize = 10;
/// "Stamm.", which sorts as "Name" does without stepping it.
const REGULAR: usize = 9;
/// The orders "Name" steps through, one more with every click on it.
const NAME_ORDERS: usize = 5;
/// The main positions the list takes, keeper to striker.
const POSITIONS: u8 = 10;
/// Days out from which a player counts as out.
const WEEK: i32 = 7;

/// Which row of titles shows — the season's (`stats`) or the first — which
/// column the list is sorted by, and which of its orders "Name" has:
/// alphabetical (0), the eleven and the bench first (1, the order the
/// screen opens with), the bench first (2), those in neither first (3),
/// those out first (4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sort {
    pub stats: bool,
    pub column: usize,
    pub names: usize,
}

impl Default for Sort {
    fn default() -> Self {
        Self { stats: false, column: 0, names: 1 }
    }
}

impl Sort {
    /// A click on a column title of the row that shows; "Name" steps on
    /// even when it is the one sorted by, "Stamm." sorts by name.
    pub fn click(&mut self, column: usize) {
        if column >= if self.stats { STATS_COLUMNS } else { COLUMNS } {
            return;
        }
        if column == 0 {
            self.names = (self.names + 1) % NAME_ORDERS;
        }
        self.column = if self.stats && column == REGULAR { 0 } else { column };
    }
}

/// A player's name as the alphabetical order compares it, in the game's
/// bytes (Windows-1252): the nickname and whether he goes by it, his last
/// and first names.
#[derive(Clone, Default, Debug)]
pub struct Name {
    pub nicknamed: bool,
    pub nickname: Vec<u8>,
    pub last: Vec<u8>,
    pub first: Vec<u8>,
}

/// What the season's row compares of a player besides his fitness: his
/// nation's three letters, his form this season in tenths ("DForm"), his
/// appearances and goals, how content he is (0 happy to 3 wanting to go).
#[derive(Clone, Default, Debug)]
pub struct Season {
    pub nation: Vec<u8>,
    pub form: i32,
    pub appearances: i32,
    pub goals: i32,
    pub content: i32,
}

/// The squad by main position, keeper to striker, in squad order within a
/// position (`FUN_00696590`): where the list's sort starts, and the order
/// the player info pages through. A player without a main position is left
/// out.
pub fn by_position(players: &[Player]) -> Vec<usize> {
    (1..=POSITIONS)
        .flat_map(|position| {
            (0..players.len()).filter(move |&p| players[p].positions[0] == position)
        })
        .collect()
}

/// The list's rows, by place in the squad.
pub fn order(
    pitch: &Tables,
    club: &ClubState,
    names: &[Name],
    seasons: &[Season],
    today: NaiveDate,
    sort: Sort,
) -> Vec<usize> {
    let rows = by_position(&club.players);
    if sort.stats {
        let season = |p: usize| seasons.get(p).cloned().unwrap_or_default();
        let fitness = |p: usize| lineup::fitness_of(&club.fitness, p);
        exchange_sort(rows, |p, q| match sort.column {
            0 => by_name(club, names, sort.names, p, q),
            1 => club.players[p].positions[0] < club.players[q].positions[0],
            2 => season(p).nation < season(q).nation,
            3 => season(p).form > season(q).form,
            4 => lineup::condition(fitness(p)) > lineup::condition(fitness(q)),
            5 => lineup::freshness(fitness(p)) > lineup::freshness(fitness(q)),
            6 => season(p).appearances > season(q).appearances,
            7 => season(p).goals > season(q).goals,
            8 => season(p).content < season(q).content,
            _ => false,
        })
    } else {
        exchange_sort(rows, |p, q| before(pitch, club, names, today, sort, p, q))
    }
}

/// The original's sort (`FUN_0050f112`): every row against each above it,
/// swapped when it goes before — sorted, but not stably.
fn exchange_sort(mut rows: Vec<usize>, before: impl Fn(usize, usize) -> bool) -> Vec<usize> {
    for i in 1..rows.len() {
        for j in 0..i {
            if before(rows[i], rows[j]) {
                rows.swap(i, j);
            }
        }
    }
    rows
}

/// Whether `p` goes before `q` (`FUN_0050f310`).
fn before(
    pitch: &Tables,
    club: &ClubState,
    names: &[Name],
    today: NaiveDate,
    sort: Sort,
    p: usize,
    q: usize,
) -> bool {
    let (a, b) = (&club.players[p], &club.players[q]);
    let fitness = |p: usize| lineup::fitness_of(&club.fitness, p);
    let status = |p: usize| club.status.get(p).copied().unwrap_or_default();
    match sort.column {
        0 => by_name(club, names, sort.names, p, q),
        1 => a.positions[0] < b.positions[0],
        2 => {
            let ([_, p2, p3], [_, q2, q3]) = (a.positions, b.positions);
            match (p2, q2) {
                (0, _) => false,
                (_, 0) => true,
                _ if p2 < q2 => true,
                _ if p2 > q2 => false,
                _ => p3 != 0 && (q3 == 0 || p3 < q3),
            }
        }
        3 => {
            let (ca, cb) = (abilities(a), abilities(b));
            ca > cb || ca == cb && a.abilities[0] as i16 > b.abilities[0] as i16
        }
        4 => a.strength > b.strength,
        5 => lineup::form(fitness(p)) > lineup::form(fitness(q)),
        6 => fitness(p).morale > fitness(q).morale,
        7 => {
            pitch.strength(a, fitness(p), status(p).doping)
                > pitch.strength(b, fitness(q), status(q).doping)
        }
        8 => a.shirt < b.shirt,
        9 => a.age(today) < b.age(today),
        10 => status(p).injured < status(q).injured,
        _ => false,
    }
}

/// How many abilities a player has, good and bad: of a keeper's nine, of a
/// field player's sixteen (`FUN_0074b34c`).
fn abilities(player: &Player) -> u32 {
    let mask = if player.positions[0] == GOALKEEPER { 0x1ff } else { 0xffff };
    (player.abilities[0] & mask).count_ones() + (player.abilities[1] & mask).count_ones()
}

/// The orders of "Name", as the comparator has them: several answer yes
/// for two players alike, which the sort then swaps.
fn by_name(club: &ClubState, names: &[Name], order: usize, p: usize, q: usize) -> bool {
    let lineup = &club.lineup;
    let slot = |places: &[Option<usize>], p: usize| places.iter().position(|&s| s == Some(p));
    let first_by_slot = |places: &[Option<usize>]| match (slot(places, p), slot(places, q)) {
        (Some(a), Some(b)) => Some(b >= a),
        (Some(_), None) => Some(true),
        (None, Some(_)) => Some(false),
        (None, None) => None,
    };
    let in_team = |p: usize| slot(&lineup.field, p).is_some() || slot(&lineup.bench, p).is_some();
    let out = |p: usize| {
        let status = club.status.get(p).copied().unwrap_or_default();
        status.suspended != 0 || status.injured >= WEEK
    };
    match order {
        // Last names, or nicknames — as `p` goes by his, for both (a slip
        // of the original's) — then first names, byte by byte.
        0 => {
            let empty = Name::default();
            let (a, b) = (names.get(p).unwrap_or(&empty), names.get(q).unwrap_or(&empty));
            let name = |n: &Name| if a.nicknamed { n.nickname.clone() } else { n.last.clone() };
            match name(a).cmp(&name(b)) {
                std::cmp::Ordering::Less => true,
                std::cmp::Ordering::Equal => a.first < b.first,
                std::cmp::Ordering::Greater => false,
            }
        }
        1 => first_by_slot(&lineup.field).or_else(|| first_by_slot(&lineup.bench)).unwrap_or(true),
        2 => first_by_slot(&lineup.bench).unwrap_or(true),
        3 => !in_team(p) && in_team(q),
        _ => out(p) && !out(q),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lineup::{BENCH, FIELD, Lineup};

    fn player(position: u8) -> Player {
        Player {
            last_name: String::new(),
            first_name: String::new(),
            strength: 5,
            positions: [position, 0, 0],
            abilities: [0, 0],
            traits: [0, 0],
            birthday: None,
            shirt: 0,
        }
    }

    #[test]
    fn the_sort_is_not_stable() {
        let key = [1, 1, 0];
        assert_eq!(exchange_sort(vec![0, 1, 2], |p, q| key[p] < key[q]), [2, 1, 0]);
    }

    #[test]
    fn the_list_opens_on_the_eleven_the_bench_and_the_rest_backwards() {
        // By position: 1, 4 (keepers), then 0, 2, 3, 5 (position 3).
        let players: Vec<Player> = [3, 1, 3, 3, 1, 3].into_iter().map(player).collect();
        let mut field = [None; FIELD];
        field[0] = Some(3);
        field[4] = Some(1);
        let mut bench = [None; BENCH];
        bench[2] = Some(5);
        let lineup = Lineup { spots: [(0, 0); FIELD], field, bench };
        let club = ClubState::new(players, lineup, Vec::new(), 0);
        assert_eq!(by_position(&club.players), [1, 4, 0, 2, 3, 5]);
        let names = vec![Name::default(); 6];
        let rows = exchange_sort(by_position(&club.players), |p, q| {
            by_name(&club, &names, Sort::default().names, p, q)
        });
        assert_eq!(rows, [3, 1, 5, 2, 0, 4]);
    }

    #[test]
    fn the_seasons_row_sorts_by_name_under_its_last_title() {
        let mut sort = Sort { stats: true, ..Sort::default() };
        sort.click(4);
        assert_eq!(sort.column, 4);
        sort.click(REGULAR);
        assert_eq!(sort, Sort { stats: true, column: 0, names: 1 });
        sort.click(STATS_COLUMNS);
        assert_eq!(sort.column, 0);
    }

    #[test]
    fn name_steps_through_its_orders() {
        let mut sort = Sort::default();
        sort.click(4);
        assert_eq!(sort, Sort { stats: false, column: 4, names: 1 });
        for names in [2, 3, 4, 0, 1] {
            sort.click(0);
            assert_eq!(sort, Sort { stats: false, column: 0, names });
        }
    }
}
