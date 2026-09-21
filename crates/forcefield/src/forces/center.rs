//! Port of `center.js`: translate all nodes so their mean sits at `(x, y)`.
//! Note this moves positions directly, not velocities, and ignores alpha.

use super::Force;
use crate::bodies::Bodies;
use crate::lcg::Random;

pub struct Center {
    x: f64,
    y: f64,
    strength: f64,
}

impl Default for Center {
    fn default() -> Self {
        Center::new(0.0, 0.0)
    }
}

impl Center {
    pub fn new(x: f64, y: f64) -> Center {
        Center {
            x,
            y,
            strength: 1.0,
        }
    }
    pub fn strength(mut self, s: f64) -> Self {
        self.strength = s;
        self
    }
    pub fn set_center(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;
    }
    pub fn set_strength(&mut self, s: f64) {
        self.strength = s;
    }
}

impl Force for Center {
    fn apply(&mut self, bodies: &mut Bodies, _alpha: f64, _random: &mut Random) {
        let n = bodies.len();
        let pos = &mut bodies.pos;
        let (mut sx, mut sy) = (0.0, 0.0);
        for i in 0..n {
            sx += pos[2 * i];
            sy += pos[2 * i + 1];
        }
        sx = (sx / n as f64 - self.x) * self.strength;
        sy = (sy / n as f64 - self.y) * self.strength;
        for i in 0..n {
            pos[2 * i] -= sx;
            pos[2 * i + 1] -= sy;
        }
    }
}
