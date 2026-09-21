//! The particle arrays a simulation integrates.
//!
//! d3 keeps `x`, `y`, `vx`, `vy`, `fx`, `fy` as properties on each node
//! object. Here they are flat interleaved `f64` buffers so a wasm consumer
//! can read positions as a typed-array view without copying.

/// Initial state for one node. `NaN` means "unset" exactly as `null`/
/// `undefined` do in d3: unset positions get the phyllotaxis spiral, unset
/// velocities become zero, unset `fx`/`fy` leave the node free.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub fx: f64,
    pub fy: f64,
}

impl Node {
    /// A node with everything unset.
    pub const UNSET: Node = Node {
        x: f64::NAN,
        y: f64::NAN,
        vx: f64::NAN,
        vy: f64::NAN,
        fx: f64::NAN,
        fy: f64::NAN,
    };

    /// A free node at a given position with zero velocity.
    pub fn at(x: f64, y: f64) -> Node {
        Node {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            ..Node::UNSET
        }
    }

    /// Pin the node at `(fx, fy)`.
    pub fn fixed(fx: f64, fy: f64) -> Node {
        Node {
            fx,
            fy,
            ..Node::UNSET
        }
    }
}

impl Default for Node {
    fn default() -> Self {
        Node::UNSET
    }
}

/// Positions, velocities and pins for all nodes, interleaved `[x0, y0, x1, y1, …]`.
#[derive(Clone, Debug, Default)]
pub struct Bodies {
    pub pos: Vec<f64>,
    pub vel: Vec<f64>,
    /// `NaN` where the node is free.
    pub fixed: Vec<f64>,
}

impl Bodies {
    pub fn from_nodes(nodes: &[Node]) -> Bodies {
        let mut b = Bodies {
            pos: Vec::with_capacity(nodes.len() * 2),
            vel: Vec::with_capacity(nodes.len() * 2),
            fixed: Vec::with_capacity(nodes.len() * 2),
        };
        for n in nodes {
            b.pos.extend_from_slice(&[n.x, n.y]);
            b.vel.extend_from_slice(&[n.vx, n.vy]);
            b.fixed.extend_from_slice(&[n.fx, n.fy]);
        }
        b
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.pos.len() / 2
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    #[inline]
    pub fn x(&self, i: usize) -> f64 {
        self.pos[2 * i]
    }
    #[inline]
    pub fn y(&self, i: usize) -> f64 {
        self.pos[2 * i + 1]
    }
    #[inline]
    pub fn vx(&self, i: usize) -> f64 {
        self.vel[2 * i]
    }
    #[inline]
    pub fn vy(&self, i: usize) -> f64 {
        self.vel[2 * i + 1]
    }

    pub fn node(&self, i: usize) -> Node {
        Node {
            x: self.x(i),
            y: self.y(i),
            vx: self.vx(i),
            vy: self.vy(i),
            fx: self.fixed[2 * i],
            fy: self.fixed[2 * i + 1],
        }
    }

    /// Pin node `i` at `(fx, fy)`; pass `NaN` for either to leave that axis free.
    pub fn set_fixed(&mut self, i: usize, fx: f64, fy: f64) {
        self.fixed[2 * i] = fx;
        self.fixed[2 * i + 1] = fy;
    }
}
