//! Port of `y.js`: pull each node toward a target y.

use super::{Force, NodeAccessor};
use crate::bodies::Bodies;
use crate::lcg::Random;

pub struct Y {
    y: NodeAccessor,
    strength: NodeAccessor,
    yz: Vec<f64>,
    strengths: Vec<f64>,
}

impl Default for Y {
    fn default() -> Self {
        Y::new(0.0)
    }
}

impl Y {
    pub fn new(y: impl Into<NodeAccessor>) -> Y {
        Y {
            y: y.into(),
            strength: NodeAccessor::Constant(0.1),
            yz: Vec::new(),
            strengths: Vec::new(),
        }
    }
    pub fn strength(mut self, s: impl Into<NodeAccessor>) -> Self {
        self.strength = s.into();
        self
    }
    pub fn set_y(&mut self, y: impl Into<NodeAccessor>) {
        self.y = y.into();
    }
    pub fn set_strength(&mut self, s: impl Into<NodeAccessor>) {
        self.strength = s.into();
    }
}

impl Force for Y {
    fn initialize(&mut self, bodies: &Bodies, _random: &mut Random) {
        let n = bodies.len();
        self.yz.clear();
        self.strengths.clear();
        for i in 0..n {
            let yz = self.y.get(i, i);
            self.yz.push(yz);
            self.strengths.push(if yz.is_nan() {
                0.0
            } else {
                self.strength.get(i, i)
            });
        }
    }

    fn apply(&mut self, bodies: &mut Bodies, alpha: f64, _random: &mut Random) {
        let n = bodies.len();
        for i in 0..n {
            bodies.vel[2 * i + 1] +=
                (self.yz[i] - bodies.pos[2 * i + 1]) * self.strengths[i] * alpha;
        }
    }
}
