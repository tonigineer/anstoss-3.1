//! Who takes the set pieces and who is captain ("Aufgabenverteilung",
//! dialog 418): a human club keeps six ranked lists — penalties, free kicks
//! crossed and shot, corners from the left and the right, the captain. They
//! hold the manager's order; every reader first puts a list in order
//! ([`normalise`]), which drops who left and adds the rest of the squad, so
//! a list names the whole squad when it is read. The line-up shows each
//! duty on the first of its list who stands in the eleven ([`on_pitch`]).
//!
//! Here a list holds places in the squad, as the line-up does; the save
//! holds player ids (`records::Extra::duty_lists`).

use crate::lineup::{self, FIELD};
use crate::season::{Fitness, Status};
use crate::world::{GOALKEEPER, Player};

/// The lists, and how many players each names at most.
pub const DUTIES: usize = 6;
pub const MOST: usize = 35;

/// A duty, by its list's place in the club's extra record.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Duty {
    Penalty,
    /// An indirect free kick, crossed in ("Freistoß Flanke").
    CrossFreeKick,
    /// A direct free kick ("Freistoß Schuß").
    ShotFreeKick,
    CornerLeft,
    CornerRight,
    Captain,
}

impl Duty {
    /// In the order the extra record keeps their lists.
    pub const STORED: [Duty; DUTIES] = [
        Duty::Penalty,
        Duty::CrossFreeKick,
        Duty::ShotFreeKick,
        Duty::CornerLeft,
        Duty::CornerRight,
        Duty::Captain,
    ];
    /// In the order the line-up hands them to the eleven (`FUN_0050bef1`),
    /// which numbers them so from 1: the icons' and tips' order.
    pub const ON_PITCH: [Duty; DUTIES] = [
        Duty::Penalty,
        Duty::ShotFreeKick,
        Duty::CrossFreeKick,
        Duty::CornerLeft,
        Duty::CornerRight,
        Duty::Captain,
    ];

    /// Its list's place in the extra record.
    pub fn stored(self) -> usize {
        self as usize
    }

    /// Its place among the line-up's duties, from 0.
    pub fn on_pitch(self) -> usize {
        Duty::ON_PITCH.iter().position(|&d| d == self).unwrap_or(0)
    }
}

/// The manager's lists, by [`Duty::stored`]; empty for a club without.
pub type Lists = [Vec<usize>; DUTIES];

/// Who stays on a list as it is read: no one twice, no one who left — and
/// nothing past [`MOST`].
fn kept(players: usize, list: &[usize]) -> Vec<usize> {
    let mut kept = Vec::new();
    for &p in list.iter().take(MOST) {
        if p < players && !kept.contains(&p) {
            kept.push(p);
        }
    }
    kept
}

/// A list put in order as every reader does (`FUN_006dc332`): the
/// manager's order, then the rest of the squad by strength — a keeper
/// counts a tenth of a field player of his strength — as far as there is
/// room; on a tie in squad order.
pub fn normalise(players: &[Player], list: &[usize]) -> Vec<usize> {
    let mut list = kept(players.len(), list);
    let key = |p: usize| {
        let strength = i32::from(players[p].strength) * 10;
        if players[p].positions[0] == GOALKEEPER { strength / 10 } else { strength }
    };
    let mut rest: Vec<usize> = (0..players.len()).filter(|p| !list.contains(p)).collect();
    // A stable sort, highest first, as the original's insertion sort.
    rest.sort_by_key(|&p| std::cmp::Reverse(key(p)));
    let room = MOST.saturating_sub(list.len());
    list.extend(rest.into_iter().take(room));
    list
}

/// The captain's list as dialog 418 puts it in order (`FUN_006dc7b7`):
/// those who will not be captain dropped, then the rest of the squad in
/// squad order. The original tests the flag of the squad's player at a
/// list's place rather than of the one listed there, so once it meets a
/// flagged player's place it drops the rest of the list; kept here. With
/// nobody left the whole squad is listed and the flags cleared — whether
/// they are.
pub fn normalise_captain(
    players: &[Player],
    status: &[Status],
    list: &[usize],
) -> (Vec<usize>, bool) {
    let no_captain = |p: usize| status.get(p).is_some_and(|s| s.no_captain);
    let mut list = kept(players.len(), list);
    if let Some(at) = (0..list.len()).find(|&i| no_captain(i)) {
        list.truncate(at);
    }
    for p in 0..players.len() {
        if list.len() < MOST && !no_captain(p) && !list.contains(&p) {
            list.push(p);
        }
    }
    if list.is_empty() {
        return ((0..players.len().min(MOST)).collect(), true);
    }
    (list, false)
}

/// The duty each of the eleven shows on the line-up (`FUN_0050bef1`):
/// every duty goes to the first of its list who stands in the eleven, the
/// lists taken rank by rank in [`Duty::ON_PITCH`] order, and a player shows
/// only the first that reaches him — a duty whose taker shows another
/// shows nowhere.
pub fn on_pitch(
    players: &[Player],
    lists: &Lists,
    field: &[Option<usize>; FIELD],
) -> [Option<Duty>; FIELD] {
    let lists: Vec<Vec<usize>> =
        Duty::ON_PITCH.iter().map(|d| normalise(players, &lists[d.stored()])).collect();
    let mut shown = [None; FIELD];
    let mut found = [false; DUTIES];
    for rank in 0..MOST {
        for (d, duty) in Duty::ON_PITCH.into_iter().enumerate() {
            if found[d] {
                continue;
            }
            let Some(&p) = lists[d].get(rank) else { continue };
            if let Some(slot) = field.iter().position(|&f| f == Some(p)) {
                found[d] = true;
                shown[slot].get_or_insert(duty);
            }
        }
    }
    shown
}

/// The lists a manager's new club starts with (`FUN_006fd390` runs
/// `FUN_006a25c2`, `FUN_006a22e1` and `FUN_006a2446`): the captain by
/// strength and form, leaders first; penalties by strength and form, the
/// nerveless first; free kicks and corners by strength and form, field
/// players before keepers and free-kick specialists before them. Who will
/// not be captain is skipped — unless that is everyone, whose flags are
/// cleared.
pub fn choose(players: &[Player], fitness: &[Fitness], status: &mut [Status]) -> Lists {
    let form = |p: usize| lineup::form(lineup::fitness_of(fitness, p));
    let field_player = |p: usize| players[p].positions[0] != GOALKEEPER;
    let character = |p: usize| (players[p].traits[1] >> 1) as u16;
    let ranked = |key: &dyn Fn(usize) -> i32| {
        let mut order: Vec<usize> = (0..players.len()).collect();
        order.sort_by_key(|&p| std::cmp::Reverse(key(p)));
        order.truncate(MOST);
        order
    };
    let penalties = ranked(&|p| {
        let key = i32::from(players[p].strength) * 10 + form(p);
        if field_player(p) && character(p) & NERVELESS != 0 { key + 100 } else { key }
    });
    let free_kicks = ranked(&|p| {
        let key = i32::from(players[p].strength) * 10 + form(p);
        match field_player(p) {
            true if players[p].abilities[0] & FREE_KICKS != 0 => key + 130,
            true => key + 30,
            false => key,
        }
    });

    if status.iter().take(players.len()).all(|s| s.no_captain) {
        status.iter_mut().for_each(|s| s.no_captain = false);
    }
    let no_captain = |p: usize| status.get(p).is_some_and(|s| s.no_captain);
    // The keys go by who is eligible, but the sort reads them by the place
    // in the squad (at 0x6a2780): alike while nobody is skipped. A key not
    // written yet is the stack's; here 0.
    let mut keys: Vec<i32> = Vec::new();
    let mut captains: Vec<usize> = Vec::new();
    for p in (0..players.len()).filter(|&p| !no_captain(p)) {
        let mut key = i32::from(players[p].strength) * 10 + form(p) / 5;
        if character(p) & LEADER != 0 {
            key += 11;
        }
        keys.push(key);
        let read = |q: usize| keys.get(q).copied().unwrap_or(0);
        let at = captains.iter().rposition(|&q| read(q) >= key).map_or(0, |i| i + 1);
        captains.insert(at, p);
    }
    captains.truncate(MOST);

    let mut lists: Lists = Default::default();
    lists[Duty::Penalty.stored()] = penalties;
    for duty in [Duty::CrossFreeKick, Duty::ShotFreeKick, Duty::CornerLeft, Duty::CornerRight] {
        lists[duty.stored()].clone_from(&free_kicks);
    }
    lists[Duty::Captain.stored()] = captains;
    lists
}

/// Bits of a player's second word of character, as the record keeps it:
/// a leader ("Führungsperson"), without nerves ("Mann ohne Nerven").
const LEADER: u16 = 0x1;
const NERVELESS: u16 = 0x8;
/// A field player's ability: free kicks.
const FREE_KICKS: u16 = 0x20;

/// A save's list, ids to places in the squad: up to its first 0, who is
/// still in the squad.
pub fn from_ids(squad: &[u16], ids: &[u16]) -> Vec<usize> {
    let ids = ids.iter().take_while(|&&id| id != 0);
    kept(squad.len(), &ids.filter_map(|id| squad.iter().position(|s| s == id)).collect::<Vec<_>>())
}

/// A list as the save keeps it: ids, then zeros.
pub fn to_ids(squad: &[u16], list: &[usize]) -> [u16; MOST] {
    let mut ids = [0; MOST];
    for (id, &p) in ids.iter_mut().zip(list) {
        *id = squad.get(p).copied().unwrap_or(0);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::season::Fitness;

    /// A player of this strength on this main position.
    fn player(strength: u8, position: u8) -> Player {
        Player {
            last_name: String::new(),
            first_name: String::new(),
            strength,
            positions: [position, 0, 0],
            abilities: [0, 0],
            traits: [0, 0],
            birthday: None,
            shirt: 0,
            feet: 0,
        }
    }

    #[test]
    fn a_list_is_the_managers_order_then_the_rest_by_strength() {
        // A keeper of 9 counts less than a field player of 2; ties keep
        // the squad's order.
        let players =
            [player(9, GOALKEEPER), player(5, 3), player(7, 4), player(5, 6), player(2, 8)];
        assert_eq!(normalise(&players, &[3, 3, 9]), [3, 2, 1, 4, 0]);
        assert_eq!(normalise(&players, &[]), [2, 1, 3, 4, 0]);
    }

    #[test]
    fn the_captains_list_drops_the_rest_from_a_flagged_players_place() {
        let players = [player(5, 3), player(6, 4), player(7, 5), player(8, 6)];
        let mut status = vec![Status::default(); 4];
        // Squad player 1 will not be captain: the list loses its entries
        // from place 1 on, whoever they are; the rest follow in squad order.
        status[1].no_captain = true;
        assert_eq!(normalise_captain(&players, &status, &[3, 2, 0]), (vec![3, 0, 2], false));
        // With nobody left, everybody, and the flags go.
        status.iter_mut().for_each(|s| s.no_captain = true);
        assert_eq!(normalise_captain(&players, &status, &[2]), (vec![0, 1, 2, 3], true));
    }

    #[test]
    fn each_duty_shows_once_and_a_player_shows_one() {
        let players: Vec<Player> = (0..13).map(|i| player(5 + i % 3, 3)).collect();
        let mut field = [None; FIELD];
        for (slot, place) in field.iter_mut().enumerate() {
            *place = Some(slot + 2);
        }
        // The same first in every list: the penalty's icon alone.
        let lists: Lists = std::array::from_fn(|_| vec![4]);
        let shown = on_pitch(&players, &lists, &field);
        assert_eq!(shown[2], Some(Duty::Penalty));
        assert_eq!(shown.iter().flatten().count(), 1);
        // Those not in the eleven are passed over; a taker of two shows the
        // one that reaches him first — by rank, then in the line-up's order
        // of duties — and the other shows nowhere. Lists without an order
        // go by strength.
        let mut lists: Lists = Default::default();
        lists[Duty::Penalty.stored()] = vec![0, 5];
        lists[Duty::CornerLeft.stored()] = vec![5];
        lists[Duty::Captain.stored()] = vec![1, 6];
        let shown = on_pitch(&players, &lists, &field);
        assert_eq!(shown[0], Some(Duty::ShotFreeKick));
        assert_eq!(shown[3], Some(Duty::CornerLeft));
        assert_eq!(shown[4], Some(Duty::Captain));
        assert!(!shown.contains(&Some(Duty::Penalty)));
    }

    #[test]
    fn a_new_club_ranks_its_takers_by_their_keys() {
        let mut players = vec![player(6, 3), player(7, 4), player(9, GOALKEEPER), player(6, 5)];
        // Without nerves, on free kicks, a leader.
        players[0].traits[1] = i64::from(NERVELESS) << 1;
        players[3].abilities[0] = FREE_KICKS;
        players[3].traits[1] = i64::from(LEADER) << 1;
        let fitness = vec![Fitness::new(5000, 5000); 4];
        let mut status = vec![Status::default(); 4];
        let lists = choose(&players, &fitness, &mut status);
        assert_eq!(lists[Duty::Penalty.stored()], [0, 2, 1, 3]);
        assert_eq!(lists[Duty::ShotFreeKick.stored()], [3, 1, 0, 2]);
        assert_eq!(lists[Duty::CornerRight.stored()], lists[Duty::CrossFreeKick.stored()]);
        assert_eq!(lists[Duty::Captain.stored()], [2, 3, 1, 0]);
    }
}
