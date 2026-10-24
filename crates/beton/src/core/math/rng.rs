//! Tiny deterministic RNG for gameplay.

/// A 64-bit xorshift generator.
pub struct Rng(u64);

impl Rng {
    /// Seed the generator. Zero is replaced with a fixed non-zero value.
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0x4d59_5df4_d0f3_3173 } else { seed })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform value in `[lo, hi)`.
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        let t = (self.next_u64() >> 11) as f32 / ((1u64 << 53) as f32);
        lo + (hi - lo) * t
    }
}
