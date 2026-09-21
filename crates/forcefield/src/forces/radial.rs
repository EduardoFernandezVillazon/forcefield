//! Port of `radial.js`: pull each node toward a circle of given radius
//! around `(x, y)`.

use super::{Force, NodeAccessor};
use crate::bodies::Bodies;
use crate::lcg::Random;

pub struct Radial {
    radius: NodeAccessor,
    x: f64,
    y: f64,
    strength: NodeAccessor,
    radiuses: Vec<f64>,
    strengths: Vec<f64>,
}

impl Radial {
    pub fn new(radius: impl Into<NodeAccessor>, x: f64, y: f64) -> Radial {
        Radial {
            radius: radius.into(),
            x,
            y,
            strength: NodeAccessor::Constant(0.1),
            radiuses: Vec::new(),
            strengths: Vec::new(),
        }
    }
    pub fn strength(mut self, s: impl Into<NodeAccessor>) -> Self {
        self.strength = s.into();
        self
    }
    pub fn set_radius(&mut self, r: impl Into<NodeAccessor>) {
        self.radius = r.into();
    }
    pub fn set_strength(&mut self, s: impl Into<NodeAccessor>) {
        self.strength = s.into();
    }
    pub fn set_center(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;
    }
}

impl Force for Radial {
    fn initialize(&mut self, bodies: &Bodies, _random: &mut Random) {
        let n = bodies.len();
        self.radiuses.clear();
        self.strengths.clear();
        for i in 0..n {
            let r = self.radius.get(i, i);
            self.radiuses.push(r);
            self.strengths.push(if r.is_nan() {
                0.0
            } else {
                self.strength.get(i, i)
            });
        }
    }

    fn apply(&mut self, bodies: &mut Bodies, alpha: f64, _random: &mut Random) {
        let n = bodies.len();
        for i in 0..n {
            let mut dx = bodies.pos[2 * i] - self.x;
            if dx == 0.0 || dx.is_nan() {
                dx = 1e-6;
            }
            let mut dy = bodies.pos[2 * i + 1] - self.y;
            if dy == 0.0 || dy.is_nan() {
                dy = 1e-6;
            }
            let r = (dx * dx + dy * dy).sqrt();
            let k = (self.radiuses[i] - r) * self.strengths[i] * alpha / r;
            bodies.vel[2 * i] += dx * k;
            bodies.vel[2 * i + 1] += dy * k;
        }
    }
}
