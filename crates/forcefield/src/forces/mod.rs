//! The force model. Each force is a `Force` that `initialize`s against the
//! node set (evaluating its accessors once, as d3 does) and `apply`s a
//! velocity update every tick.

use crate::bodies::Bodies;
use crate::lcg::Random;

mod center;
mod collide;
mod link;
mod many_body;
mod radial;
mod x;
mod y;

pub use center::Center;
pub use collide::Collide;
pub use link::{Link, LinkRef};
pub use many_body::ManyBody;
pub use radial::Radial;
pub use x::X;
pub use y::Y;

pub trait Force {
    /// Called when the force is installed and whenever the node set or the
    /// random source changes. Accessors are evaluated here, once.
    fn initialize(&mut self, _bodies: &Bodies, _random: &mut Random) {}

    /// Apply one step of this force at the given `alpha`.
    fn apply(&mut self, bodies: &mut Bodies, alpha: f64, random: &mut Random);
}

/// A per-item parameter: d3's "number or function" accessors, plus a
/// precomputed array (what the wasm boundary sends).
pub enum Accessor<Ctx> {
    Constant(f64),
    Values(Vec<f64>),
    Func(Box<dyn Fn(Ctx) -> f64>),
}

impl<Ctx> Accessor<Ctx> {
    #[inline]
    pub fn get(&self, index: usize, ctx: Ctx) -> f64 {
        match self {
            Accessor::Constant(v) => *v,
            Accessor::Values(v) => v.get(index).copied().unwrap_or(f64::NAN),
            Accessor::Func(f) => f(ctx),
        }
    }

    pub fn func(f: impl Fn(Ctx) -> f64 + 'static) -> Self {
        Accessor::Func(Box::new(f))
    }
}

impl<Ctx> From<f64> for Accessor<Ctx> {
    fn from(v: f64) -> Self {
        Accessor::Constant(v)
    }
}

impl<Ctx> From<Vec<f64>> for Accessor<Ctx> {
    fn from(v: Vec<f64>) -> Self {
        Accessor::Values(v)
    }
}

/// Accessor over node index.
pub type NodeAccessor = Accessor<usize>;
