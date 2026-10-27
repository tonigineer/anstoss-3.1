//! The trainers' market: the money scale of an offer, whether a trainer
//! takes it, the last compromise he offers, and what sending the old coach
//! away costs (`CCoTrainerAngebotMachen`, `CCoTrainerListe`; KONTAKTE.md
//! §5.6, §5.7).
//!
//! The manager offers a yearly salary on a slider and years on a spin;
//! the trainer weighs the offer against what his record says he asks, takes
//! it, makes one last compromise, or turns it down for good.

use crate::random::Random;

/// The slider's money scale, counted in thousands: up to each limit in
/// steps of this much (the table at `0x87ff50`). It ends at position 315,
/// the 10,000 thousand the game lets a trainer ask for (DAT_00a3b2d0).
const SCALE: [(i64, i64); 5] = [(50, 1), (250, 5), (500, 10), (1000, 25), (i64::MAX, 50)];

/// The highest offer the slider can reach, in money.
pub const OFFER_MOST: i64 = 10_000_000;

/// A salary's position on the scale, in thousands.
pub fn position(salary: i64) -> i64 {
    let mut position = 0;
    let mut from = 0;
    for (limit, step) in SCALE {
        if salary <= limit {
            return position + (salary.max(from) - from) / step;
        }
        position += (limit - from) / step;
        from = limit;
    }
    position
}

/// The salary, in thousands, at a position on the scale.
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

/// Whether the trainer takes the money and the years offered
/// (`FUN_004854c3`), against what his record says he asks: −1 he takes it,
/// 1 he makes a last compromise, 0 he turns it down. Near what he asks he
/// always takes it; far under it he never does; between, he weighs it and
/// draws. The draws only happen when the comparison of the figures lets
/// them.
pub fn negotiate(salary: i64, years: i32, asked_salary: i64, asked_years: i32, random: &mut Random) -> i8 {
    let demand = asked_salary.max(1);
    let asked = i64::from(asked_years.max(1));
    // How well the offer fits what he asks, truncated to a short as the
    // game's does.
    let fit = (salary * 100 / demand + (i64::from(years) - asked) * 10) as i16;
    if fit >= 97 || demand - salary < 20_000 {
        return -1;
    }
    if fit > 89 && random.below(4) != 0 {
        return -1;
    }
    if fit > 79 && random.below(2) != 0 {
        return -1;
    }
    if fit >= 90 {
        return 1;
    }
    if fit > 79 && random.below(5) != 0 {
        return 1;
    }
    if fit > 69 && random.below(3) != 0 {
        return 1;
    }
    if fit > 59 && random.below(2) != 0 {
        return 1;
    }
    0
}

/// The last compromise of a trainer whose judgement came out even
/// (`FUN_00485f3a`): half between his demand and the offer, rounded down
/// onto whole ten thousands; he keeps his years, unless the manager offered
/// fewer than he asks, and then he may come down a little. The money and
/// the years, as they are to stand in the contract.
pub fn compromise(
    asked_salary: i64,
    salary: i64,
    years: i32,
    asked_years: i32,
    random: &mut Random,
) -> (i64, i32) {
    let money = (asked_salary + salary) / 2 / 10_000 * 10_000;
    let mut asked = asked_years;
    if years < asked_years && random.below(2) != 0 {
        asked -= random.below((asked_years - years) as u32) as i32;
    }
    (money, asked)
}

/// Whether the club's own coach extends his contract by a year
/// (`FUN_0048735f` and `FUN_00592662`): not while more than three years are
/// left or past his sixty-fourth year, not over the head trainer's
/// competence, and only at the pay his own asks — for a co-trainer 70,000
/// plus two thousand a point of competence up to four, six thousand up to
/// nine, ten thousand beyond; for a goalkeepers' coach 30,000 plus five
/// thousand a point.
pub fn may_extend(
    years_left: i32,
    age: i32,
    competence: i32,
    head: i32,
    salary: i64,
    keepers: bool,
) -> bool {
    if years_left > 3 || age > 64 || head < competence {
        return false;
    }
    let asked = if keepers {
        30_000 + i64::from(competence) * 5_000
    } else {
        let point = match competence { 0..=4 => 20_000, 5..=9 => 60_000, _ => 100_000 };
        70_000 + i64::from(competence) * point
    };
    salary >= asked
}

/// What sending a coach still under contract away costs (`FUN_00487424`):
/// half his yearly salary, or a twelfth per month of the season still to
/// play when only his last year is left — never under a thousand, and whole
/// thousands. `month` is the calendar month.
pub fn severance(salary: i64, years_left: i32, month: u32) -> i64 {
    let half = if month < 7 && years_left == 1 {
        salary * i64::from(7 - month.min(7)) / 12
    } else {
        salary / 2
    };
    (half / 1000 * 1000).max(1000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::Random;

    #[test]
    fn the_scale_ends_at_ten_million() {
        assert_eq!(position(OFFER_MOST / 1000), 315);
        assert_eq!(at_position(315), 10_000);
        for (salary, snapped) in [
            (1, 1),
            (49, 49),
            (50, 50),
            (51, 50),
            (250, 250),
            (254, 250),
            (499, 490),
            (999, 975),
            (4000, 4000),
            (10_000, 10_000),
        ] {
            assert_eq!(at_position(position(salary)), snapped);
        }
    }

    #[test]
    fn near_his_demand_he_takes_it_far_under_he_does_not() {
        let mut random = Random::new(1);
        // A thousand under what he asks: taken, whatever the years.
        assert_eq!(negotiate(499_000, 1, 500_000, 3, &mut random), -1);
        assert_eq!(negotiate(490_000, 5, 500_000, 1, &mut random), -1);
        // Nothing at all: turned down, no compromise offered.
        assert_eq!(negotiate(0, 3, 500_000, 3, &mut random), 0);
    }

    #[test]
    fn the_compromise_is_half_and_even() {
        let mut random = Random::new(7);
        assert_eq!(compromise(500_000, 400_000, 3, 3, &mut random), (450_000, 3));
        assert_eq!(compromise(500_000, 33_000, 3, 3, &mut random), (260_000, 3));
    }

    #[test]
    fn a_coach_extends_only_on_terms() {
        // The Probe1 co-trainer of 1953: past the age, however modestly
        // he stands.
        assert!(!may_extend(0, 73, 5, 12, 182_400, false));
        // Too many years already left, and one who knows more than the
        // head trainer.
        assert!(!may_extend(4, 50, 5, 12, 1_000_000, false));
        assert!(!may_extend(1, 50, 13, 12, 1_470_000, false));
        // The pay floor climbs with competence: 150,000 at four, 430,000
        // at six, 1,270,000 at twelve.
        assert!(!may_extend(1, 59, 4, 12, 149_000, false));
        assert!(may_extend(1, 59, 4, 12, 259_200, false));
        assert!(!may_extend(1, 59, 6, 12, 429_000, false));
        assert!(may_extend(1, 59, 6, 12, 430_000, false));
        assert!(may_extend(1, 59, 12, 12, 1_270_000, false));
        // A goalkeepers' coach: 30,000 and 5,000 a point, whatever his
        // competence against the head trainer.
        assert!(!may_extend(1, 63, 15, 15, 104_000, true));
        assert!(may_extend(1, 63, 15, 15, 465_000, true));
        assert!(may_extend(3, 64, 5, 5, 55_000, true));
    }

    #[test]
    fn the_old_coach_costs_half_a_year_or_less() {
        // In the autumn, a coach with years to go: half.
        assert_eq!(severance(500_000, 2, 9), 250_000);
        // In spring, only his last year left: the months he is sent away early.
        assert_eq!(severance(500_000, 1, 3), 166_000);
        assert_eq!(severance(1_000, 1, 1), 1_000);
    }
}
