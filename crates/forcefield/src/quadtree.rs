//! Port of d3-quadtree 2.0.0, specialised for the way d3-force uses it.
//!
//! d3-force builds a fresh tree every time a force runs (`quadtree(nodes, x, y)`),
//! then walks it with `visitAfter` (accumulate) and `visit` (apply). Only
//! that subset is ported: build from a point list, `cover`, `add`, `visit`,
//! `visit_after`, `find`. `remove`, `copy`, `extent` setters and the
//! accessor setters are not needed by any force.
//!
//! The JS tree is a graph of small objects: an internal node is an
//! `Array(4)` of children, a leaf is `{data, next}` where `next` chains
//! coincident points. Here every node lives in one arena `Vec<Quad>` and
//! links are `u32` indices, with [`NIL`] for "absent". The per-node scratch
//! fields the forces hang on quads in JS (`x`, `y`, `value`, `r`) are plain
//! fields.
//!
//! Traversal order and floating-point operation order are preserved from
//! the JS exactly, because the many-body force sums contributions in visit
//! order and the jiggle draws from a shared random stream in that order.

/// "No node" marker for child, `next` and `data` links.
pub const NIL: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
pub struct Quad {
    /// Children `[bottom<<1 | right]`; only meaningful for internal nodes.
    pub children: [u32; 4],
    /// Point index for a leaf; [`NIL`] for an internal node.
    pub data: u32,
    /// Next coincident leaf in the chain, or [`NIL`].
    pub next: u32,
    /// Scratch used by many-body: centre of charge and total charge.
    pub x: f64,
    pub y: f64,
    pub value: f64,
    /// Scratch used by collide: largest radius in the subtree.
    pub r: f64,
}

impl Quad {
    const INTERNAL: Quad = Quad {
        children: [NIL; 4],
        data: NIL,
        next: NIL,
        x: 0.0,
        y: 0.0,
        value: 0.0,
        r: 0.0,
    };

    fn leaf(data: u32) -> Quad {
        Quad {
            data,
            ..Quad::INTERNAL
        }
    }

    #[inline]
    pub fn is_leaf(&self) -> bool {
        self.data != NIL
    }
}

#[derive(Clone, Debug)]
pub struct Quadtree {
    pub nodes: Vec<Quad>,
    pub root: u32,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    // Reused traversal scratch so a tick does not allocate per visit.
    stack: Vec<Frame>,
    order: Vec<Frame>,
}

#[derive(Clone, Copy, Debug)]
struct Frame {
    node: u32,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Default for Quadtree {
    fn default() -> Self {
        Quadtree::new()
    }
}

impl Quadtree {
    pub fn new() -> Quadtree {
        Quadtree {
            nodes: Vec::new(),
            root: NIL,
            x0: f64::NAN,
            y0: f64::NAN,
            x1: f64::NAN,
            y1: f64::NAN,
            stack: Vec::new(),
            order: Vec::new(),
        }
    }

    /// Empty the tree, keeping its allocations.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.root = NIL;
        self.x0 = f64::NAN;
        self.y0 = f64::NAN;
        self.x1 = f64::NAN;
        self.y1 = f64::NAN;
    }

    #[inline]
    fn alloc(&mut self, q: Quad) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(q);
        id
    }

    /// Port of `addAll`: compute the extent of the valid points, cover it,
    /// then add every point in order. `point(i)` returns the coordinates of
    /// point `i`; a `NaN` coordinate makes the point ignored, as in d3.
    pub fn build(&mut self, n: usize, point: impl Fn(usize) -> (f64, f64)) {
        self.clear();
        let (mut x0, mut y0, mut x1, mut y1) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for i in 0..n {
            let (x, y) = point(i);
            if x.is_nan() || y.is_nan() {
                continue;
            }
            if x < x0 {
                x0 = x;
            }
            if x > x1 {
                x1 = x;
            }
            if y < y0 {
                y0 = y;
            }
            if y > y1 {
                y1 = y;
            }
        }
        if x0 > x1 || y0 > y1 {
            return;
        }
        self.cover(x0, y0);
        self.cover(x1, y1);
        for i in 0..n {
            let (x, y) = point(i);
            self.add(x, y, i as u32, &point);
        }
    }

    /// Port of `cover.js`: grow the extent (by doubling) until it contains `(x, y)`.
    pub fn cover(&mut self, x: f64, y: f64) {
        if x.is_nan() || y.is_nan() {
            return;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (self.x0, self.y0, self.x1, self.y1);

        if x0.is_nan() {
            // Integer extent so that later doublings keep quadrant boundaries exact.
            x0 = x.floor();
            x1 = x0 + 1.0;
            y0 = y.floor();
            y1 = y0 + 1.0;
        } else {
            let mut z = x1 - x0;
            if z == 0.0 {
                z = 1.0;
            }
            let mut node = self.root;
            let grow = self.root != NIL && !self.nodes[self.root as usize].is_leaf();
            while x0 > x || x >= x1 || y0 > y || y >= y1 {
                let i = (((y < y0) as usize) << 1) | ((x < x0) as usize);
                if grow {
                    let mut parent = Quad::INTERNAL;
                    parent.children[i] = node;
                    node = self.alloc(parent);
                }
                z *= 2.0;
                match i {
                    0 => {
                        x1 = x0 + z;
                        y1 = y0 + z;
                    }
                    1 => {
                        x0 = x1 - z;
                        y1 = y0 + z;
                    }
                    2 => {
                        x1 = x0 + z;
                        y0 = y1 - z;
                    }
                    _ => {
                        x0 = x1 - z;
                        y0 = y1 - z;
                    }
                }
            }
            if grow {
                self.root = node;
            }
        }

        self.x0 = x0;
        self.y0 = y0;
        self.x1 = x1;
        self.y1 = y1;
    }

    /// Port of the inner `add(tree, x, y, d)` in `add.js`. The caller must
    /// have covered `(x, y)` already (as `build` does).
    pub fn add(&mut self, x: f64, y: f64, d: u32, point: &impl Fn(usize) -> (f64, f64)) {
        if x.is_nan() || y.is_nan() {
            return;
        }
        let leaf = self.alloc(Quad::leaf(d));
        let mut node = self.root;
        if node == NIL {
            self.root = leaf;
            return;
        }

        let (mut x0, mut y0, mut x1, mut y1) = (self.x0, self.y0, self.x1, self.y1);
        let mut parent = NIL;
        let mut i = 0usize;
        let (mut xm, mut ym);

        // Find the existing leaf for the new point, or add it.
        while !self.nodes[node as usize].is_leaf() {
            xm = (x0 + x1) / 2.0;
            let right = x >= xm;
            if right {
                x0 = xm;
            } else {
                x1 = xm;
            }
            ym = (y0 + y1) / 2.0;
            let bottom = y >= ym;
            if bottom {
                y0 = ym;
            } else {
                y1 = ym;
            }
            parent = node;
            i = ((bottom as usize) << 1) | (right as usize);
            node = self.nodes[parent as usize].children[i];
            if node == NIL {
                self.nodes[parent as usize].children[i] = leaf;
                return;
            }
        }

        // Is the new point exactly coincident with the existing point?
        let (xp, yp) = point(self.nodes[node as usize].data as usize);
        if x == xp && y == yp {
            self.nodes[leaf as usize].next = node;
            if parent != NIL {
                self.nodes[parent as usize].children[i] = leaf;
            } else {
                self.root = leaf;
            }
            return;
        }

        // Otherwise, split the leaf node until the old and new point are separated.
        let mut j;
        loop {
            let fresh = self.alloc(Quad::INTERNAL);
            if parent != NIL {
                self.nodes[parent as usize].children[i] = fresh;
            } else {
                self.root = fresh;
            }
            parent = fresh;
            xm = (x0 + x1) / 2.0;
            let right = x >= xm;
            if right {
                x0 = xm;
            } else {
                x1 = xm;
            }
            ym = (y0 + y1) / 2.0;
            let bottom = y >= ym;
            if bottom {
                y0 = ym;
            } else {
                y1 = ym;
            }
            i = ((bottom as usize) << 1) | (right as usize);
            j = (((yp >= ym) as usize) << 1) | ((xp >= xm) as usize);
            if i != j {
                break;
            }
        }
        self.nodes[parent as usize].children[j] = node;
        self.nodes[parent as usize].children[i] = leaf;
    }

    /// Port of `visit.js`: pre-order traversal. The callback receives the
    /// arena, the quad id and its bounds; returning `true` prunes the subtree.
    /// Children are visited in order 0, 1, 2, 3 exactly as in JS.
    pub fn visit(&mut self, mut callback: impl FnMut(&[Quad], u32, f64, f64, f64, f64) -> bool) {
        let Quadtree { nodes, stack, .. } = self;
        stack.clear();
        if self.root != NIL {
            stack.push(Frame {
                node: self.root,
                x0: self.x0,
                y0: self.y0,
                x1: self.x1,
                y1: self.y1,
            });
        }
        while let Some(q) = stack.pop() {
            let node = q.node;
            let prune = callback(nodes, node, q.x0, q.y0, q.x1, q.y1);
            let quad = &nodes[node as usize];
            if !prune && !quad.is_leaf() {
                let xm = (q.x0 + q.x1) / 2.0;
                let ym = (q.y0 + q.y1) / 2.0;
                let c = quad.children;
                if c[3] != NIL {
                    stack.push(Frame {
                        node: c[3],
                        x0: xm,
                        y0: ym,
                        x1: q.x1,
                        y1: q.y1,
                    });
                }
                if c[2] != NIL {
                    stack.push(Frame {
                        node: c[2],
                        x0: q.x0,
                        y0: ym,
                        x1: xm,
                        y1: q.y1,
                    });
                }
                if c[1] != NIL {
                    stack.push(Frame {
                        node: c[1],
                        x0: xm,
                        y0: q.y0,
                        x1: q.x1,
                        y1: ym,
                    });
                }
                if c[0] != NIL {
                    stack.push(Frame {
                        node: c[0],
                        x0: q.x0,
                        y0: q.y0,
                        x1: xm,
                        y1: ym,
                    });
                }
            }
        }
    }

    /// Port of `visitAfter.js`: post-order traversal with mutable access to
    /// the arena, so callbacks can fill in the scratch fields.
    pub fn visit_after(&mut self, mut callback: impl FnMut(&mut [Quad], u32, f64, f64, f64, f64)) {
        let Quadtree {
            nodes,
            stack,
            order,
            ..
        } = self;
        stack.clear();
        order.clear();
        if self.root != NIL {
            stack.push(Frame {
                node: self.root,
                x0: self.x0,
                y0: self.y0,
                x1: self.x1,
                y1: self.y1,
            });
        }
        while let Some(q) = stack.pop() {
            let quad = &nodes[q.node as usize];
            if !quad.is_leaf() {
                let xm = (q.x0 + q.x1) / 2.0;
                let ym = (q.y0 + q.y1) / 2.0;
                let c = quad.children;
                if c[0] != NIL {
                    stack.push(Frame {
                        node: c[0],
                        x0: q.x0,
                        y0: q.y0,
                        x1: xm,
                        y1: ym,
                    });
                }
                if c[1] != NIL {
                    stack.push(Frame {
                        node: c[1],
                        x0: xm,
                        y0: q.y0,
                        x1: q.x1,
                        y1: ym,
                    });
                }
                if c[2] != NIL {
                    stack.push(Frame {
                        node: c[2],
                        x0: q.x0,
                        y0: ym,
                        x1: xm,
                        y1: q.y1,
                    });
                }
                if c[3] != NIL {
                    stack.push(Frame {
                        node: c[3],
                        x0: xm,
                        y0: ym,
                        x1: q.x1,
                        y1: q.y1,
                    });
                }
            }
            order.push(q);
        }
        while let Some(q) = order.pop() {
            callback(nodes, q.node, q.x0, q.y0, q.x1, q.y1);
        }
    }

    /// Port of `find.js`: the closest point to `(x, y)` within `radius`
    /// (`None` radius means unbounded).
    pub fn find(
        &mut self,
        x: f64,
        y: f64,
        radius: Option<f64>,
        point: impl Fn(usize) -> (f64, f64),
    ) -> Option<usize> {
        let mut data = None;
        let (mut x0, mut y0, mut x3, mut y3) = (self.x0, self.y0, self.x1, self.y1);
        let Quadtree { nodes, stack, .. } = self;
        stack.clear();
        if self.root != NIL {
            stack.push(Frame {
                node: self.root,
                x0,
                y0,
                x1: x3,
                y1: y3,
            });
        }
        let mut radius = match radius {
            None => f64::INFINITY,
            Some(r) => {
                x0 = x - r;
                y0 = y - r;
                x3 = x + r;
                y3 = y + r;
                r * r
            }
        };
        while let Some(q) = stack.pop() {
            if q.node == NIL || q.x0 > x3 || q.y0 > y3 || q.x1 < x0 || q.y1 < y0 {
                continue;
            }
            let (x1, y1, x2, y2) = (q.x0, q.y0, q.x1, q.y1);
            let quad = &nodes[q.node as usize];
            if !quad.is_leaf() {
                let xm = (x1 + x2) / 2.0;
                let ym = (y1 + y2) / 2.0;
                let c = quad.children;
                stack.push(Frame {
                    node: c[3],
                    x0: xm,
                    y0: ym,
                    x1: x2,
                    y1: y2,
                });
                stack.push(Frame {
                    node: c[2],
                    x0: x1,
                    y0: ym,
                    x1: xm,
                    y1: y2,
                });
                stack.push(Frame {
                    node: c[1],
                    x0: xm,
                    y0: y1,
                    x1: x2,
                    y1: ym,
                });
                stack.push(Frame {
                    node: c[0],
                    x0: x1,
                    y0: y1,
                    x1: xm,
                    y1: ym,
                });
                // Visit the closest quadrant first.
                let i = (((y >= ym) as usize) << 1) | ((x >= xm) as usize);
                if i != 0 {
                    let top = stack.len() - 1;
                    stack.swap(top, top - i);
                }
            } else {
                let (px, py) = point(quad.data as usize);
                let dx = x - px;
                let dy = y - py;
                let d2 = dx * dx + dy * dy;
                if d2 < radius {
                    radius = d2;
                    let d = d2.sqrt();
                    x0 = x - d;
                    y0 = y - d;
                    x3 = x + d;
                    y3 = y + d;
                    data = Some(quad.data as usize);
                }
            }
        }
        data
    }

    /// Number of points in the tree (port of `size.js`).
    pub fn size(&mut self) -> usize {
        let mut size = 0;
        self.visit(|nodes, id, _, _, _, _| {
            let mut q = id;
            while q != NIL {
                let quad = &nodes[q as usize];
                if quad.is_leaf() {
                    size += 1;
                }
                q = quad.next;
            }
            false
        });
        size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_counts_coincident_points() {
        let pts = [
            (0.0, 0.0),
            (1.0, 1.0),
            (1.0, 1.0),
            (3.0, -2.0),
            (f64::NAN, 1.0),
        ];
        let mut t = Quadtree::new();
        t.build(pts.len(), |i| pts[i]);
        assert_eq!(t.size(), 4);
        assert_eq!((t.x0, t.y0, t.x1, t.y1), (0.0, -2.0, 4.0, 2.0));
        assert_eq!(t.find(2.9, -2.0, None, |i| pts[i]), Some(3));
        assert_eq!(t.find(2.9, -2.0, Some(0.05), |i| pts[i]), None);
    }
}
