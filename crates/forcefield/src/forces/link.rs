//! Port of `link.js`: springs between node pairs.

use super::{Accessor, Force};
use crate::bodies::Bodies;
use crate::lcg::{jiggle, Random};

/// What a link accessor sees: the link's index, its endpoints, and the
/// endpoints' link counts (degree within this force's link set).
#[derive(Clone, Copy, Debug)]
pub struct LinkRef {
    pub index: usize,
    pub source: usize,
    pub target: usize,
    pub source_count: usize,
    pub target_count: usize,
}

pub type LinkAccessor = Accessor<LinkRef>;

pub struct Link {
    links: Vec<(usize, usize)>,
    /// `None` = d3's default `1 / min(count[source], count[target])`.
    strength: Option<LinkAccessor>,
    strengths: Vec<f64>,
    distance: LinkAccessor,
    distances: Vec<f64>,
    count: Vec<usize>,
    bias: Vec<f64>,
    iterations: usize,
    initialized: bool,
}

impl Link {
    /// Links as `(source index, target index)` pairs. Resolving ids to
    /// indices is the caller's job (d3's `id` accessor).
    pub fn new(links: Vec<(usize, usize)>) -> Link {
        Link {
            links,
            strength: None,
            strengths: Vec::new(),
            distance: LinkAccessor::Constant(30.0),
            distances: Vec::new(),
            count: Vec::new(),
            bias: Vec::new(),
            iterations: 1,
            initialized: false,
        }
    }

    pub fn strength(mut self, s: impl Into<LinkAccessor>) -> Self {
        self.strength = Some(s.into());
        self
    }
    pub fn distance(mut self, d: impl Into<LinkAccessor>) -> Self {
        self.distance = d.into();
        self
    }
    pub fn iterations(mut self, n: usize) -> Self {
        self.iterations = n;
        self
    }

    pub fn links(&self) -> &[(usize, usize)] {
        &self.links
    }

    pub fn set_links(&mut self, links: Vec<(usize, usize)>) {
        self.links = links;
        self.initialized = false;
    }
    pub fn set_strength(&mut self, s: impl Into<LinkAccessor>) {
        self.strength = Some(s.into());
    }
    pub fn set_distance(&mut self, d: impl Into<LinkAccessor>) {
        self.distance = d.into();
    }
    pub fn set_iterations(&mut self, n: usize) {
        self.iterations = n;
    }

    fn link_ref(&self, i: usize) -> LinkRef {
        let (s, t) = self.links[i];
        LinkRef {
            index: i,
            source: s,
            target: t,
            source_count: self.count[s],
            target_count: self.count[t],
        }
    }
}

impl Force for Link {
    fn initialize(&mut self, bodies: &Bodies, _random: &mut Random) {
        let n = bodies.len();
        let m = self.links.len();

        self.count.clear();
        self.count.resize(n, 0);
        for &(s, t) in &self.links {
            assert!(
                s < n && t < n,
                "link endpoint out of range: ({s}, {t}) with {n} nodes"
            );
            self.count[s] += 1;
            self.count[t] += 1;
        }

        self.bias.clear();
        self.bias.extend(
            self.links
                .iter()
                .map(|&(s, t)| self.count[s] as f64 / (self.count[s] + self.count[t]) as f64),
        );

        self.strengths.clear();
        for i in 0..m {
            let r = self.link_ref(i);
            let v = match &self.strength {
                None => 1.0 / (r.source_count.min(r.target_count)) as f64,
                Some(a) => a.get(i, r),
            };
            self.strengths.push(v);
        }

        self.distances.clear();
        for i in 0..m {
            let r = self.link_ref(i);
            self.distances.push(self.distance.get(i, r));
        }
        self.initialized = true;
    }

    fn apply(&mut self, bodies: &mut Bodies, alpha: f64, random: &mut Random) {
        debug_assert!(self.initialized, "Link force applied before initialize");
        let pos = &bodies.pos;
        let vel = &mut bodies.vel;
        for _ in 0..self.iterations {
            for (i, &(source, target)) in self.links.iter().enumerate() {
                let (sx, sy) = (2 * source, 2 * source + 1);
                let (tx, ty) = (2 * target, 2 * target + 1);
                let mut x = pos[tx] + vel[tx] - pos[sx] - vel[sx];
                if x == 0.0 || x.is_nan() {
                    x = jiggle(random);
                }
                let mut y = pos[ty] + vel[ty] - pos[sy] - vel[sy];
                if y == 0.0 || y.is_nan() {
                    y = jiggle(random);
                }
                let mut l = (x * x + y * y).sqrt();
                l = (l - self.distances[i]) / l * alpha * self.strengths[i];
                x *= l;
                y *= l;
                let mut b = self.bias[i];
                vel[tx] -= x * b;
                vel[ty] -= y * b;
                b = 1.0 - b;
                vel[sx] += x * b;
                vel[sy] += y * b;
            }
        }
    }
}
