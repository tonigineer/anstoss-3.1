//! Job offers for managers without a club, and the negotiation of a
//! contract, as the game makes and conducts them in the first season.
//!
//! Clubs of the main country's lower leagues hire: up to three of them make
//! each clubless manager an offer of salary and bonuses, a season goal and a
//! contract length. The manager answers with a demand, and every round of
//! talks the club comes closer to it — or loses patience: past 15 the talks
//! break off. Signing accepts what the club offers at that moment.

use crate::random::Random;
use crate::season::{CHAMPION, GOALS, PROMOTED, Rules, Season};
use crate::world::World;

/// The money of a contract: salary, point bonus, per cup round, per title
/// or promotion, and the bonus for reaching the season goal.
pub const SALARY: usize = 0;
pub const POINT: usize = 1;
pub const CUP: usize = 2;
pub const TITLE: usize = 3;
pub const BONUS: usize = 4;
pub const MONEY: usize = 5;
/// The clubs that hire new managers: from this league of the main country
/// on, or only this one.
const GERMANY: u16 = 14;
const ENGLAND: u16 = 16;
const FRANCE: u16 = 20;
const ITALY: u16 = 28;
const SPAIN: u16 = 63;
/// The pool keeps five clubs per manager; a manager gets up to three
/// offers, at the easiest difficulty one of a club from the top five of its
/// league's list.
const POOL_PER_MANAGER: usize = 5;
const MAX_OFFERS: usize = 3;
const EASY_POSITION: usize = 5;
const EASIEST: usize = 0;
/// What a club's file says of last season: its result in the low bits, and
/// a further honour in this one.
const LAST_SEASON_RESULT: i64 = 7;
const CUP_WINNER_FLAG: i64 = 0x40;
/// Clubs that finished this high pay more.
const TOP_PLACES: usize = 6;
/// Salary, point bonus, cup round and title amounts are counted in these.
const AMOUNT_UNITS: [i64; 4] = [1000, 100, 100, 1000];
/// Talks break off at this much anger; rounds count up to here.
pub const BROKEN_OFF: i32 = 15;
const MAX_ROUNDS: i32 = 20;
/// How much each money item weighs in the talks.
const WEIGHTS: [u32; MONEY] = [3, 1, 1, 1, 1];
/// The money scale that offers are rounded down onto: up to each limit in
/// steps of this much.
const SCALE: [(i64, i64); 7] =
    [(10, 1), (100, 5), (1000, 10), (5000, 50), (10_000, 100), (50_000, 200), (i64::MAX, 500)];
/// The easiest goal the talks go towards, and the highest number of years.
const EASIEST_GOAL: usize = 10;
pub const MAX_YEARS: i32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terms {
    pub money: [i64; MONEY],
    pub years: i32,
    pub goal: usize,
    /// The club's option to extend: 0 none.
    pub option: i32,
    /// The manager may leave for another offer.
    pub exit: bool,
}

#[derive(PartialEq)]
pub struct Offer {
    /// Country slot and club.
    pub club: (usize, usize),
    /// The club's league among those in play.
    pub league: usize,
    /// What the club offers, and what the manager demands.
    pub offer: Terms,
    pub demand: Terms,
    /// The most the manager can demand, once talks have begun.
    pub limit: Option<[i64; MONEY]>,
    pub anger: i32,
    pub rounds: i32,
    /// The demand the club remembers from earlier talks.
    pub remembered: [i64; MONEY],
    pub remembered_goal: usize,
    /// Whether the talks are still on.
    pub open: bool,
}

/// Every clubless manager's offers.
pub fn offer_jobs(season: &mut Season, world: &World, rules: &Rules) {
    let clubless: Vec<usize> =
        (0..season.managers.len()).filter(|&m| season.managers[m].club.is_none()).collect();
    let main = season.main;
    let Some(country) = world.country(main) else { return };
    let Some(hiring) = hiring_leagues(country.nation) else { return };
    let mut pool: Vec<(usize, usize, usize)> = world
        .leagues_in_play(main, main)
        .iter()
        .enumerate()
        .filter(|&(i, _)| hiring(i))
        .flat_map(|(i, league)| league.clubs.iter().enumerate().map(move |(pos, &c)| (i, c, pos)))
        .collect();
    let random = &mut season.random;
    // Shuffled the game's way: each place drawn until a free one comes up.
    let mut free = vec![true; pool.len()];
    let mut shuffled = Vec::with_capacity(pool.len());
    while shuffled.len() < pool.len() {
        let i = random.below(pool.len() as u32) as usize;
        if free[i] {
            free[i] = false;
            shuffled.push(pool[i]);
        }
    }
    pool = shuffled;
    pool.truncate(POOL_PER_MANAGER * season.managers.len());

    let mut taken = vec![false; pool.len()];
    for (k, &m) in clubless.iter().enumerate() {
        let mut picked = Vec::new();
        if season.managers[m].difficulty == EASIEST {
            let near_top: Vec<usize> =
                (0..pool.len()).filter(|&i| !taken[i] && pool[i].2 < EASY_POSITION).collect();
            if !near_top.is_empty() {
                let i = near_top[season.random.below(near_top.len() as u32) as usize];
                taken[i] = true;
                picked.push(i);
            }
        }
        for _ in 0..POOL_PER_MANAGER * (k + 1) {
            if picked.len() >= MAX_OFFERS {
                break;
            }
            let i = season.random.below(pool.len() as u32) as usize;
            if !taken.get(i).copied().unwrap_or(true) {
                taken[i] = true;
                picked.push(i);
            }
        }
        if picked.len() < MAX_OFFERS
            && let Some(i) = (0..pool.len()).find(|&i| !taken[i])
        {
            taken[i] = true;
            picked.push(i);
        }
        let level = season.managers[m].level;
        let offers = picked
            .into_iter()
            .map(|i| {
                let (league, club, place) = pool[i];
                let goal = season.expected.get(&(main, club)).map_or(0, |&(_, goal)| goal);
                let at = Hirer { club: (main, club), league, place, goal };
                make_offer(world, rules, at, level, &mut season.random)
            })
            .collect();
        season.managers[m].offers = offers;
    }
}

/// Which leagues of a main country hire new managers, by their index.
fn hiring_leagues(nation: u16) -> Option<fn(usize) -> bool> {
    match nation {
        GERMANY => Some(|league| league >= 2),
        ENGLAND => Some(|league| league >= 3),
        FRANCE | ITALY | SPAIN => Some(|league| league == 1),
        _ => None,
    }
}

/// A club that makes an offer: its country slot and index, its league among
/// those in play, its place in the league's list — where it finished last
/// season — and its own goal for the season.
struct Hirer {
    club: (usize, usize),
    league: usize,
    place: usize,
    goal: usize,
}

/// One club's offer, and the demand the manager starts the talks with.
fn make_offer(
    world: &World,
    rules: &Rules,
    hirer: Hirer,
    level: i32,
    random: &mut Random,
) -> Offer {
    let Hirer { club: (slot, index), league, place, goal: club_goal } = hirer;
    let country = world.country(slot).expect("the club's country");
    let club = &country.clubs[index];
    let class = if [ITALY, SPAIN].contains(&country.nation) { 3 } else { league.min(2) };
    let mut amount = |kind: usize| {
        let [base, step, count] = rules.amounts[kind][class];
        // The point bonus steps by its count.
        let step = if kind == POINT { count } else { step };
        (i64::from(base) + i64::from(step) * i64::from(random.below(u32::from(count))))
            * AMOUNT_UNITS[kind]
    };
    let (mut salary, point, cup, title) =
        (amount(SALARY), amount(POINT), amount(CUP), amount(TITLE));
    let mut bonus = title * 10 / (i64::from(random.below(5)) + 18);
    if bonus == 0 {
        bonus = 100;
    }
    salary = match club.money {
        m if m < -3_000_000 => salary * 75 / 100,
        m if m < -1_000_000 => salary * 9 / 10,
        m if m > 8_000_000 => salary * 125 / 100,
        m if m > 1_000_000 => salary * 11 / 10,
        _ => salary,
    };
    let percent = attraction(club, place, level);
    bonus = bonus * percent / 100;
    let r = random.below(100);
    let years = match level {
        l if l < 5 => [(40, 1), (90, 2), (100, 3)],
        l if l < 10 => [(15, 1), (85, 2), (100, 3)],
        _ => [(60, 2), (90, 3), (100, 4)],
    }
    .iter()
    .find(|&&(below, _)| r < below)
    .map_or(2, |&(_, years)| years);
    let lower = usize::from(league > 0);
    let floor = if league > 0 { 2 } else { 0 };
    let row = &rules.offer_goals[lower];
    let mut goal = usize::from(row[club_goal.max(floor).min(GOALS - 1)]);
    if goal == 2 && club.affiliate >= 1 {
        goal = 4;
    }
    if goal == GOALS - 1 {
        bonus *= 3;
    }
    let exit = random.below(50) == 0;
    // The club decides what the manager is to be — always both here.
    let _ = (random.below(10), random.below(10));
    let option = match r {
        r if r < 50 => 0,
        r if r < 80 => 1,
        _ => 2,
    };
    let money = [
        salary * 11 / 10 * percent / 100,
        point * percent / 100,
        cup * percent / 100,
        title * percent / 100,
        bonus,
    ];
    let offer = Terms { money: money.map(round_down), years, goal, option, exit };
    let demand = Terms {
        money: money.map(|v| round_down(v * 11 / 10 / 5 * 5)),
        years: random.below(2) as i32 + 2,
        goal,
        option: 0,
        exit: true,
    };
    Offer {
        club: (slot, index),
        league,
        offer,
        demand,
        limit: None,
        anger: 0,
        rounds: 0,
        remembered: [0; MONEY],
        remembered_goal: 0,
        open: true,
    }
}

/// In percent, how much a club offers a manager of `level`: more for a
/// manager of a high level, a club with many fans, last season's champion, a
/// club that finished in the top six without coming up, and a club of a low
/// class.
fn attraction(club: &crate::world::Club, place: usize, level: i32) -> i64 {
    let level = i64::from(level.clamp(0, 12));
    let mut percent = 70 + 2 * level;
    if level > 11 {
        percent = percent * 12 / 10;
    }
    if level > 8 {
        percent = percent * 12 / 10;
    }
    match club.fans {
        0 => percent = percent * 12 / 10,
        1 => percent = percent * 11 / 10,
        _ => {}
    }
    let result = club.last_season & LAST_SEASON_RESULT;
    if result == CHAMPION {
        percent = percent * 13 / 10;
    }
    if club.last_season & CUP_WINNER_FLAG != 0 {
        percent = percent * 12 / 10;
    }
    if place < TOP_PLACES && result != PROMOTED {
        percent = percent * 11 / 10;
    }
    match club.class {
        0 => percent += percent / 20 * 3,
        1 => percent += percent / 10,
        2 => percent += percent / 20,
        _ => {}
    }
    percent
}

/// Rounds down onto the game's money scale.
pub fn round_down(value: i64) -> i64 {
    let step = SCALE.iter().find(|&&(limit, _)| value <= limit).map_or(500, |&(_, s)| s);
    value.max(0) / step * step
}

/// A value's position on the money scale.
pub fn position(value: i64) -> i64 {
    let mut position = 0;
    let mut from = 0;
    for (limit, step) in SCALE {
        if value <= limit {
            return position + (value.max(from) - from) / step;
        }
        position += (limit - from) / step;
        from = limit;
    }
    position
}

/// The value at a position on the money scale.
pub fn at_position(position: i64) -> i64 {
    let mut left = position.max(0);
    let mut from = 0;
    for (limit, step) in SCALE {
        let steps = (limit - from) / step;
        if left <= steps {
            return from + left * step;
        }
        left -= steps;
        from = limit;
    }
    from
}

/// What a round of talks came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Round {
    Going,
    BrokenOff,
}

/// One round of talks: the club moves towards the demand and grows angry
/// the more the demand asks of it. `skill` is three times the manager's
/// negotiating ability; `board` how kindly the club's board looks on its
/// manager; `fresh` whether these talks began with the first round, when
/// the club compares the demand with nothing earlier.
pub fn negotiate(
    offer: &mut Offer,
    rules: &Rules,
    skill: i32,
    board: i64,
    fresh: bool,
    random: &mut Random,
) -> Round {
    let skill = skill.max(0) as u32;
    let board_i = board as i32;
    if offer.rounds < MAX_ROUNDS {
        offer.rounds += 1;
    }
    let n = offer.rounds;
    let start_anger = offer.anger;
    let (club, demand) = (&mut offer.offer, offer.demand);
    let mut a = offer.anger;
    let mut sides = [(0u32, 0i32); MONEY];
    for (i, side) in sides.iter_mut().enumerate() {
        let (asked, given) = (demand.money[i], club.money[i]);
        *side = if given < 1 {
            (60, 1)
        } else {
            let r = asked * 100 / given;
            if r >= 101 {
                ((r - 90) as u32, 1)
            } else if r < 100 {
                ((110 - r) as u32, -1)
            } else {
                (0, 0)
            }
        };
    }
    for i in 0..MONEY {
        let (mut q, s) = sides[i];
        let asked = demand.money[i];
        let given = &mut club.money[i];
        match s {
            -1 => {
                *given = asked;
                for _ in 0..WEIGHTS[i] {
                    if a > 0 && (q >= 80 || random.below(100) < q) {
                        a -= 1;
                    }
                }
            }
            0 => *given = (*given).max(asked),
            _ if q <= 60 => {
                let mut added = false;
                for _ in 0..WEIGHTS[i] {
                    if !added && random.below(100) < q + 5 {
                        a += 1;
                        added = true;
                    }
                }
                if *given == 0 {
                    *given = asked / 4 + 1;
                }
                *given += *given * i64::from(random.below(5) + 4) / 100;
            }
            _ => {
                let mut first = q;
                for _ in 0..WEIGHTS[i] {
                    first = q;
                    while q > 100 {
                        if random.below(10) != 0 {
                            a += 1;
                        }
                        q -= 100;
                    }
                    if random.below(100) < 5 * q {
                        a += 1;
                    }
                }
                let raise = if first < 150 { random.below(9) + 7 } else { random.below(11) + 16 };
                *given += *given * i64::from(raise) / 100;
            }
        }
        *given = (*given).min(asked);
    }
    // A demand that went up since the club last heard it angers it, unless
    // it gave way elsewhere.
    if !fresh {
        for (i, &weight) in WEIGHTS.iter().enumerate() {
            for _ in 0..weight {
                if offer.remembered[i] < demand.money[i] {
                    for (c, &(q, s)) in sides.iter().enumerate().take(4) {
                        if c != i && s < 0 && q > 14 && a > 0 && (q >= 100 || random.below(100) < q)
                        {
                            a -= 1;
                        }
                    }
                    a += 1;
                    offer.remembered[i] = demand.money[i];
                }
            }
        }
    }
    if a > 3 && random.below(100) < 40 && club.years != demand.years {
        club.years =
            if demand.years == 0 { 0 } else { club.years + (demand.years - club.years).signum() };
        if random.below(2) != 0 {
            a += 1;
        }
    }
    if n > 3 {
        if !fresh && offer.remembered_goal < demand.goal {
            a += 1;
        }
        if !fresh {
            offer.remembered_goal = demand.goal;
        }
        if club.goal < demand.goal && random.below(30) < skill + board.max(0) as u32 {
            club.goal = usize::from(rules.easier[club.goal.min(GOALS - 1)]);
        } else if demand.goal < club.goal
            && club.goal < EASIEST_GOAL
            && random.below(7) as i32 > board_i + 1
        {
            club.goal = usize::from(rules.harder[club.goal.min(GOALS - 1)]);
            a = (a - 1).max(0);
        }
    }
    if n > 1 {
        if !demand.exit {
            club.exit = false;
            a = (a - 1).max(0);
        } else if random.below(100) < 20 {
            club.exit = true;
            a += 1;
        }
        if demand.option == 1 {
            club.option = 1;
            a = (a - 1).max(0);
        } else if random.below(100) < 20 {
            club.option = 0;
            a += 1;
        }
    }
    if a < start_anger && random.below(200) < skill && a > 0 {
        a -= 1;
    }
    if n > 3 {
        a += 1;
    }
    offer.anger = a;
    if a >= BROKEN_OFF {
        offer.open = false;
        Round::BrokenOff
    } else {
        Round::Going
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn clubless_managers_get_offers_from_the_lower_leagues() {
        use crate::season::NewManager;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let world = World::load(&dir.join("Data.a3")).unwrap();
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let rules = Rules::load(&exe).unwrap();
        let manager = |name: &str, difficulty| NewManager {
            first_name: name.into(),
            last_name: "Test".into(),
            club: None,
            national_team: None,
            level: 2,
            difficulty,
            negotiating: 2,
        };
        let managers = vec![manager("A", 0), manager("B", 2)];
        let season = Season::first(&world, &rules, vec![0], 0, managers, Random::new(7));
        for m in &season.managers {
            assert!(!m.offers.is_empty() && m.offers.len() <= MAX_OFFERS);
            for o in &m.offers {
                let club = &world.country(0).unwrap().clubs[o.club.1];
                assert!(o.league >= 2, "{} is in league {}", club.name, o.league);
                println!(
                    "{}: {} {:?} demand {:?}",
                    m.first_name, club.name, o.offer, o.demand.money
                );
            }
        }
    }

    #[test]
    fn the_money_scale_goes_both_ways() {
        for value in
            [0, 7, 10, 15, 100, 110, 990, 1000, 1050, 5000, 10_000, 49_800, 50_000, 350_500]
        {
            assert_eq!(at_position(position(value)), value, "{value}");
        }
        assert_eq!(round_down(12_345), 12_200);
        assert_eq!(round_down(351_234), 351_000);
        assert_eq!(position(11), position(10));
    }
}
