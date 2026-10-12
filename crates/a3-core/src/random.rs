//! The game's random numbers: L'Ecuyer's combined generator, of whose
//! result the game takes the high bits.

const M1: i64 = 2_147_483_563;
const M2: i64 = 2_147_483_399;

#[derive(Clone, PartialEq)]
pub struct Random {
    s1: i64,
    s2: i64,
}

impl Random {
    pub fn new(seed: u64) -> Self {
        let s1 = 1 + (seed % (M1 as u64 - 1)) as i64;
        let s2 = 1 + (seed.rotate_left(29) % (M2 as u64 - 1)) as i64;
        Self { s1, s2 }
    }

    fn next(&mut self) -> i64 {
        self.s1 = 40_014 * self.s1 % M1;
        self.s2 = 40_692 * self.s2 % M2;
        let z = self.s1 - self.s2;
        if z < 1 { z + M1 - 1 } else { z }
    }

    /// A number below `n`; 0 when there is no choice.
    pub fn below(&mut self, n: u32) -> u32 {
        if n < 2 {
            return 0;
        }
        (self.next() >> 16) as u32 % n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_stay_below_their_bound() {
        let mut random = Random::new(12345);
        assert!((0..10_000).all(|_| random.below(7) < 7));
        assert_eq!(random.below(1), 0);
        let mut seen = [false; 7];
        (0..1000).for_each(|_| seen[random.below(7) as usize] = true);
        assert!(seen.iter().all(|&s| s));
    }
}
