//! Port of `x.js`: pull each node toward a target x.

use super::{Force, NodeAccessor};
use crate::bodies::Bodies;
use crate::lcg::Random;

pub struct X {
    x: NodeAccessor,
    strength: NodeAccessor,
    xz: Vec<f64>,
    strengths: Vec<f64>,
}

impl Default for X {
    fn default() -> Self {
        X::new(0.0)
    }
}

impl X {
    pub fn new(x: impl Into<NodeAccessor>) -> X {
        X {
            x: x.into(),
            strength: NodeAccessor::Constant(0.1),
            xz: Vec::new(),
            strengths: Vec::new(),
        }
    }
    pub fn strength(mut self, s: impl Into<NodeAccessor>) -> Self {
        self.strength = s.into();
        self
    }
    pub fn set_x(&mut self, x: impl Into<NodeAccessor>) {
        self.x = x.into();
    }
    pub fn set_strength(&mut self, s: impl Into<NodeAccessor>) {
        self.strength = s.into();
    }
}

impl Force for X {
    fn initialize(&mut self, bodies: &Bodies, _random: &mut Random) {
        let n = bodies.len();
        self.xz.clear();
        self.strengths.clear();
        for i in 0..n {
            let xz = self.x.get(i, i);
            self.xz.push(xz);
            self.strengths.push(if xz.is_nan() {
                0.0
            } else {
                self.strength.get(i, i)
            });
        }
    }

    fn apply(&mut self, bodies: &mut Bodies, alpha: f64, _random: &mut Random) {
        let n = bodies.len();
        for i in 0..n {
            bodies.vel[2 * i] += (self.xz[i] - bodies.pos[2 * i]) * self.strengths[i] * alpha;
        }
    }
}
