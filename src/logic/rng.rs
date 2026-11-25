/// Small deterministic xorshift64* generator, so battles are reproducible
/// from a seed and tests don't need an external crate.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        // SplitMix64 scramble so nearby seeds give unrelated streams.
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        Rng { state: if z == 0 { 0x2545_F491_4F6C_DD1D } else { z } }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform integer in 0..100.
    pub fn percent(&mut self) -> i32 {
        ((self.next_u64() >> 33) % 100) as i32
    }

    /// True with `chance`% probability. 0 never succeeds, 100 always does.
    pub fn roll(&mut self, chance: i32) -> bool {
        self.percent() < chance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roll_extremes() {
        let mut r = Rng::new(42);
        for _ in 0..1000 {
            assert!(r.roll(100));
            assert!(!r.roll(0));
        }
    }

    #[test]
    fn adjacent_seeds_differ() {
        let a: Vec<i32> = { let mut r = Rng::new(2); (0..8).map(|_| r.percent()).collect() };
        let b: Vec<i32> = { let mut r = Rng::new(3); (0..8).map(|_| r.percent()).collect() };
        assert_ne!(a, b);
    }

    #[test]
    fn percent_in_range_and_roughly_uniform() {
        let mut r = Rng::new(7);
        let mut hits = 0;
        for _ in 0..10_000 {
            let p = r.percent();
            assert!((0..100).contains(&p));
            if r.roll(50) {
                hits += 1;
            }
        }
        assert!((4500..5500).contains(&hits), "hits = {hits}");
    }
}
