//! Deterministic random source.
//!
//! Port of d3-force `lcg.js`: a linear congruential generator with the
//! Numerical Recipes parameters, seeded with 1. d3 uses it for `jiggle`,
//! the ±5e-7 nudge that separates coincident nodes. Reproducing it exactly
//! is what makes tick-for-tick parity with the JS reference possible.

/// A source of numbers in `[0, 1)`. The default is [`Lcg`]; anything
/// implementing `FnMut() -> f64` can be boxed into one.
pub struct Random(Box<dyn FnMut() -> f64>);

impl Random {
    /// d3's default source: `lcg()` seeded with 1.
    pub fn lcg() -> Self {
        let mut s = Lcg::new();
        Random(Box::new(move || s.next()))
    }

    /// Wrap an arbitrary source (must return values in `[0, 1)`).
    pub fn custom(f: impl FnMut() -> f64 + 'static) -> Self {
        Random(Box::new(f))
    }

    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f64 {
        (self.0)()
    }
}

impl Default for Random {
    fn default() -> Self {
        Random::lcg()
    }
}

/// The generator itself, exposed for tests and for callers that want to
/// drive it directly.
#[derive(Clone, Debug)]
pub struct Lcg {
    s: u64,
}

const A: u64 = 1_664_525;
const C: u64 = 1_013_904_223;
const M: u64 = 4_294_967_296; // 2^32

impl Lcg {
    pub fn new() -> Self {
        Lcg { s: 1 }
    }

    /// `(s = (a * s + c) % m) / m`. In JS `a * s + c` stays below 2^53 so
    /// the double arithmetic is exact; u64 arithmetic here is identical.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f64 {
        self.s = (A * self.s + C) % M;
        self.s as f64 / M as f64
    }
}

impl Default for Lcg {
    fn default() -> Self {
        Lcg::new()
    }
}

/// Port of `jiggle.js`: `(random() - 0.5) * 1e-6`.
#[inline]
pub fn jiggle(random: &mut Random) -> f64 {
    (random.next() - 0.5) * 1e-6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_values_match_js() {
        // Values printed by `d3.forceSimulation().randomSource()()` in node.
        let mut r = Lcg::new();
        assert_eq!(r.next(), 1015568748.0 / 4294967296.0);
        assert_eq!(r.next(), 1586005467.0 / 4294967296.0);
    }
}
