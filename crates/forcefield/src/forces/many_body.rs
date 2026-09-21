//! Port of `manyBody.js`: Barnes-Hut n-body charge (repulsion when negative).

use super::{Force, NodeAccessor};
use crate::bodies::Bodies;
use crate::lcg::{jiggle, Random};
use crate::quadtree::{Quadtree, NIL};

pub struct ManyBody {
    strength: NodeAccessor,
    strengths: Vec<f64>,
    distance_min2: f64,
    distance_max2: f64,
    theta2: f64,
    tree: Quadtree,
}

impl Default for ManyBody {
    fn default() -> Self {
        ManyBody::new()
    }
}

impl ManyBody {
    pub fn new() -> ManyBody {
        ManyBody {
            strength: NodeAccessor::Constant(-30.0),
            strengths: Vec::new(),
            distance_min2: 1.0,
            distance_max2: f64::INFINITY,
            theta2: 0.81,
            tree: Quadtree::new(),
        }
    }

    pub fn strength(mut self, s: impl Into<NodeAccessor>) -> Self {
        self.strength = s.into();
        self
    }
    pub fn distance_min(mut self, d: f64) -> Self {
        self.distance_min2 = d * d;
        self
    }
    pub fn distance_max(mut self, d: f64) -> Self {
        self.distance_max2 = d * d;
        self
    }
    pub fn theta(mut self, t: f64) -> Self {
        self.theta2 = t * t;
        self
    }

    pub fn set_strength(&mut self, s: impl Into<NodeAccessor>) {
        self.strength = s.into();
    }
}

impl Force for ManyBody {
    fn initialize(&mut self, bodies: &Bodies, _random: &mut Random) {
        let n = bodies.len();
        self.strengths.clear();
        self.strengths
            .extend((0..n).map(|i| self.strength.get(i, i)));
    }

    fn apply(&mut self, bodies: &mut Bodies, alpha: f64, random: &mut Random) {
        let n = bodies.len();
        let tree = &mut self.tree;
        {
            let pos = &bodies.pos;
            tree.build(n, |i| (pos[2 * i], pos[2 * i + 1]));
        }

        // accumulate
        let strengths = &self.strengths;
        let pos = &bodies.pos;
        tree.visit_after(|quads, id, _, _, _, _| {
            let mut strength = 0.0;
            if !quads[id as usize].is_leaf() {
                let (mut weight, mut x, mut y) = (0.0, 0.0, 0.0);
                for i in 0..4 {
                    let q = quads[id as usize].children[i];
                    if q == NIL {
                        continue;
                    }
                    let c = quads[q as usize].value.abs();
                    if c != 0.0 && !c.is_nan() {
                        strength += quads[q as usize].value;
                        weight += c;
                        x += c * quads[q as usize].x;
                        y += c * quads[q as usize].y;
                    }
                }
                quads[id as usize].x = x / weight;
                quads[id as usize].y = y / weight;
            } else {
                let d = quads[id as usize].data as usize;
                quads[id as usize].x = pos[2 * d];
                quads[id as usize].y = pos[2 * d + 1];
                let mut q = id;
                while q != NIL {
                    strength += strengths[quads[q as usize].data as usize];
                    q = quads[q as usize].next;
                }
            }
            quads[id as usize].value = strength;
        });

        // apply
        let (distance_min2, distance_max2, theta2) =
            (self.distance_min2, self.distance_max2, self.theta2);
        for node in 0..n {
            let (nx, ny) = (bodies.pos[2 * node], bodies.pos[2 * node + 1]);
            let vel = &mut bodies.vel;
            tree.visit(|quads, id, x1, _, x2, _| {
                let quad = &quads[id as usize];
                if quad.value == 0.0 || quad.value.is_nan() {
                    return true;
                }

                let mut x = quad.x - nx;
                let mut y = quad.y - ny;
                let mut w = x2 - x1;
                let mut l = x * x + y * y;

                // Apply the Barnes-Hut approximation if possible.
                if w * w / theta2 < l {
                    if l < distance_max2 {
                        if x == 0.0 {
                            x = jiggle(random);
                            l += x * x;
                        }
                        if y == 0.0 {
                            y = jiggle(random);
                            l += y * y;
                        }
                        if l < distance_min2 {
                            l = (distance_min2 * l).sqrt();
                        }
                        vel[2 * node] += x * quad.value * alpha / l;
                        vel[2 * node + 1] += y * quad.value * alpha / l;
                    }
                    return true;
                }
                // Otherwise, process points directly.
                else if !quad.is_leaf() || l >= distance_max2 {
                    return false;
                }

                // Limit forces for very close nodes; randomize direction if coincident.
                if quad.data as usize != node || quad.next != NIL {
                    if x == 0.0 {
                        x = jiggle(random);
                        l += x * x;
                    }
                    if y == 0.0 {
                        y = jiggle(random);
                        l += y * y;
                    }
                    if l < distance_min2 {
                        l = (distance_min2 * l).sqrt();
                    }
                }

                let mut q = id;
                while q != NIL {
                    let d = quads[q as usize].data as usize;
                    if d != node {
                        w = strengths[d] * alpha / l;
                        vel[2 * node] += x * w;
                        vel[2 * node + 1] += y * w;
                    }
                    q = quads[q as usize].next;
                }
                false
            });
        }
    }
}
