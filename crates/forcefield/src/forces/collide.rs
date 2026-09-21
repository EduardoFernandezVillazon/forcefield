//! Port of `collide.js`: circle collision with per-node radius. Works on
//! the *predicted* positions `x + vx`.

use super::{Force, NodeAccessor};
use crate::bodies::Bodies;
use crate::lcg::{jiggle, Random};
use crate::quadtree::{Quadtree, NIL};

pub struct Collide {
    radius: NodeAccessor,
    radii: Vec<f64>,
    strength: f64,
    iterations: usize,
    tree: Quadtree,
}

impl Default for Collide {
    fn default() -> Self {
        Collide::new(1.0)
    }
}

impl Collide {
    pub fn new(radius: impl Into<NodeAccessor>) -> Collide {
        Collide {
            radius: radius.into(),
            radii: Vec::new(),
            strength: 1.0,
            iterations: 1,
            tree: Quadtree::new(),
        }
    }
    pub fn strength(mut self, s: f64) -> Self {
        self.strength = s;
        self
    }
    pub fn iterations(mut self, n: usize) -> Self {
        self.iterations = n;
        self
    }
    pub fn set_radius(&mut self, r: impl Into<NodeAccessor>) {
        self.radius = r.into();
    }
    pub fn set_strength(&mut self, s: f64) {
        self.strength = s;
    }
    pub fn set_iterations(&mut self, n: usize) {
        self.iterations = n;
    }
}

impl Force for Collide {
    fn initialize(&mut self, bodies: &Bodies, _random: &mut Random) {
        let n = bodies.len();
        self.radii.clear();
        self.radii.extend((0..n).map(|i| self.radius.get(i, i)));
    }

    fn apply(&mut self, bodies: &mut Bodies, _alpha: f64, random: &mut Random) {
        let n = bodies.len();
        let radii = &self.radii;
        let strength = self.strength;
        let tree = &mut self.tree;

        for _ in 0..self.iterations {
            {
                let (pos, vel) = (&bodies.pos, &bodies.vel);
                tree.build(n, |i| {
                    (pos[2 * i] + vel[2 * i], pos[2 * i + 1] + vel[2 * i + 1])
                });
            }
            // prepare
            tree.visit_after(|quads, id, _, _, _, _| {
                let q = quads[id as usize];
                if q.is_leaf() {
                    quads[id as usize].r = radii[q.data as usize];
                    return;
                }
                let mut r = 0.0;
                for c in q.children {
                    if c != NIL && quads[c as usize].r > r {
                        r = quads[c as usize].r;
                    }
                }
                quads[id as usize].r = r;
            });

            for node in 0..n {
                let ri = radii[node];
                let ri2 = ri * ri;
                let xi = bodies.pos[2 * node] + bodies.vel[2 * node];
                let yi = bodies.pos[2 * node + 1] + bodies.vel[2 * node + 1];
                let pos = &bodies.pos;
                let vel = &mut bodies.vel;
                tree.visit(|quads, id, x0, y0, x1, y1| {
                    let quad = &quads[id as usize];
                    let mut rj = quad.r;
                    let mut r = ri + rj;
                    if quad.is_leaf() {
                        let data = quad.data as usize;
                        if data > node {
                            let mut x = xi - pos[2 * data] - vel[2 * data];
                            let mut y = yi - pos[2 * data + 1] - vel[2 * data + 1];
                            let mut l = x * x + y * y;
                            if l < r * r {
                                if x == 0.0 {
                                    x = jiggle(random);
                                    l += x * x;
                                }
                                if y == 0.0 {
                                    y = jiggle(random);
                                    l += y * y;
                                }
                                l = l.sqrt();
                                l = (r - l) / l * strength;
                                x *= l;
                                y *= l;
                                rj *= rj;
                                r = rj / (ri2 + rj);
                                vel[2 * node] += x * r;
                                vel[2 * node + 1] += y * r;
                                r = 1.0 - r;
                                vel[2 * data] -= x * r;
                                vel[2 * data + 1] -= y * r;
                            }
                        }
                        return false;
                    }
                    x0 > xi + r || x1 < xi - r || y0 > yi + r || y1 < yi - r
                });
            }
        }
    }
}
