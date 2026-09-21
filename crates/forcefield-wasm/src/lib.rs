//! wasm-bindgen bindings for `forcefield`.
//!
//! The design decision that matters: positions, velocities and pins are
//! exposed as `Float64Array` **views onto wasm linear memory**. The JS side
//! reads coordinates every tick without a copy and without crossing the
//! boundary per node. Views are invalidated whenever wasm memory grows
//! (any allocation can do that), so JS must re-fetch `positions()` after
//! any call other than `tick` — cheap, it only constructs a view object.
//!
//! Accessor-style parameters (strength, radius, target x…) are passed as
//! arrays: length 1 means a constant, otherwise one value per node/link.
//! Custom JS forces are supported through `force_custom(name, fn)`; the
//! function receives `alpha` and mutates the views directly.

use forcefield::forces::{Accessor, Center, Collide, Force, Link, ManyBody, Radial, X, Y};
use forcefield::{Node, Random, Simulation as Core};
use js_sys::{Float64Array, Function};
use wasm_bindgen::prelude::*;

fn acc<C>(v: &[f64]) -> Accessor<C> {
    if v.len() == 1 {
        Accessor::Constant(v[0])
    } else {
        Accessor::Values(v.to_vec())
    }
}

fn nodes_from(positions: &[f64]) -> Vec<Node> {
    positions
        .chunks(2)
        .map(|c| Node {
            x: c[0],
            y: c.get(1).copied().unwrap_or(f64::NAN),
            ..Node::UNSET
        })
        .collect()
}

/// A JS force. While `tick` runs, the `Simulation` is mutably borrowed, so
/// the callback must not call back into it (wasm-bindgen would throw).
/// Instead it receives fresh `Float64Array` views of positions, velocities
/// and pins as arguments, valid for the duration of the call.
struct JsForce {
    f: Function,
}

impl Force for JsForce {
    fn apply(&mut self, bodies: &mut forcefield::Bodies, alpha: f64, _random: &mut Random) {
        let pos = unsafe { Float64Array::view(&bodies.pos) };
        let vel = unsafe { Float64Array::view(&bodies.vel) };
        let fix = unsafe { Float64Array::view(&bodies.fixed) };
        let args = js_sys::Array::of4(&JsValue::from_f64(alpha), &pos, &vel, &fix);
        let _ = self.f.apply(&JsValue::NULL, &args);
    }
}

#[wasm_bindgen]
pub struct Simulation {
    inner: Core,
}

#[wasm_bindgen]
impl Simulation {
    /// `n` nodes with unset positions (d3's phyllotaxis spiral).
    #[wasm_bindgen(constructor)]
    pub fn new(n: usize) -> Simulation {
        Simulation {
            inner: Core::with_count(n),
        }
    }

    /// Nodes from interleaved `[x0, y0, x1, y1, …]`; `NaN` entries get the spiral.
    #[wasm_bindgen(js_name = fromPositions)]
    pub fn from_positions(positions: &[f64]) -> Simulation {
        Simulation {
            inner: Core::new(&nodes_from(positions)),
        }
    }

    /// Replace the node set (re-initialises every force).
    #[wasm_bindgen(js_name = setNodes)]
    pub fn set_nodes(&mut self, positions: &[f64]) {
        self.inner.set_nodes(&nodes_from(positions));
    }

    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.inner.len()
    }

    /// Zero-copy view of `[x0, y0, x1, y1, …]`. Re-fetch after any call that
    /// may allocate (everything except `tick`/`run`).
    pub fn positions(&self) -> Float64Array {
        unsafe { Float64Array::view(&self.inner.bodies().pos) }
    }

    /// Zero-copy view of `[vx0, vy0, …]`.
    pub fn velocities(&self) -> Float64Array {
        unsafe { Float64Array::view(&self.inner.bodies().vel) }
    }

    /// Zero-copy view of `[fx0, fy0, …]`; `NaN` means free. Writable from JS.
    pub fn fixed(&self) -> Float64Array {
        unsafe { Float64Array::view(&self.inner.bodies().fixed) }
    }

    #[wasm_bindgen(js_name = setPosition)]
    pub fn set_position(&mut self, i: usize, x: f64, y: f64) {
        let b = self.inner.bodies_mut();
        b.pos[2 * i] = x;
        b.pos[2 * i + 1] = y;
    }

    #[wasm_bindgen(js_name = setVelocity)]
    pub fn set_velocity(&mut self, i: usize, vx: f64, vy: f64) {
        let b = self.inner.bodies_mut();
        b.vel[2 * i] = vx;
        b.vel[2 * i + 1] = vy;
    }

    /// Pin node `i`; pass `NaN` to free an axis.
    #[wasm_bindgen(js_name = setFixed)]
    pub fn set_fixed(&mut self, i: usize, fx: f64, fy: f64) {
        self.inner.bodies_mut().set_fixed(i, fx, fy);
    }

    pub fn tick(&mut self, iterations: Option<usize>) {
        self.inner.tick(iterations.unwrap_or(1));
    }

    /// Tick until `alpha < alphaMin` or `max_ticks`; returns ticks run.
    pub fn run(&mut self, max_ticks: usize) -> usize {
        self.inner.run(max_ticks)
    }

    pub fn alpha(&self) -> f64 {
        self.inner.alpha()
    }
    #[wasm_bindgen(js_name = setAlpha)]
    pub fn set_alpha(&mut self, v: f64) {
        self.inner.set_alpha(v);
    }
    #[wasm_bindgen(js_name = alphaMin)]
    pub fn alpha_min(&self) -> f64 {
        self.inner.alpha_min()
    }
    #[wasm_bindgen(js_name = setAlphaMin)]
    pub fn set_alpha_min(&mut self, v: f64) {
        self.inner.set_alpha_min(v);
    }
    #[wasm_bindgen(js_name = alphaDecay)]
    pub fn alpha_decay(&self) -> f64 {
        self.inner.alpha_decay()
    }
    #[wasm_bindgen(js_name = setAlphaDecay)]
    pub fn set_alpha_decay(&mut self, v: f64) {
        self.inner.set_alpha_decay(v);
    }
    #[wasm_bindgen(js_name = alphaTarget)]
    pub fn alpha_target(&self) -> f64 {
        self.inner.alpha_target()
    }
    #[wasm_bindgen(js_name = setAlphaTarget)]
    pub fn set_alpha_target(&mut self, v: f64) {
        self.inner.set_alpha_target(v);
    }
    #[wasm_bindgen(js_name = velocityDecay)]
    pub fn velocity_decay(&self) -> f64 {
        self.inner.velocity_decay()
    }
    #[wasm_bindgen(js_name = setVelocityDecay)]
    pub fn set_velocity_decay(&mut self, v: f64) {
        self.inner.set_velocity_decay(v);
    }

    // --- forces ---------------------------------------------------------

    #[wasm_bindgen(js_name = forceManyBody)]
    pub fn force_many_body(
        &mut self,
        name: &str,
        strength: &[f64],
        theta: f64,
        distance_min: f64,
        distance_max: f64,
    ) {
        let f = ManyBody::new()
            .strength(acc(strength))
            .theta(theta)
            .distance_min(distance_min)
            .distance_max(distance_max);
        self.inner.add_force(name, Box::new(f));
    }

    /// `strength` empty selects d3's default `1 / min(degree)`.
    #[wasm_bindgen(js_name = forceLink)]
    pub fn force_link(
        &mut self,
        name: &str,
        sources: &[u32],
        targets: &[u32],
        distance: &[f64],
        strength: &[f64],
        iterations: usize,
    ) {
        let links: Vec<(usize, usize)> = sources
            .iter()
            .zip(targets)
            .map(|(s, t)| (*s as usize, *t as usize))
            .collect();
        let mut f = Link::new(links)
            .distance(acc(distance))
            .iterations(iterations);
        if !strength.is_empty() {
            f = f.strength(acc(strength));
        }
        self.inner.add_force(name, Box::new(f));
    }

    #[wasm_bindgen(js_name = forceCollide)]
    pub fn force_collide(&mut self, name: &str, radius: &[f64], strength: f64, iterations: usize) {
        self.inner.add_force(
            name,
            Box::new(
                Collide::new(acc(radius))
                    .strength(strength)
                    .iterations(iterations),
            ),
        );
    }

    #[wasm_bindgen(js_name = forceX)]
    pub fn force_x(&mut self, name: &str, x: &[f64], strength: &[f64]) {
        self.inner
            .add_force(name, Box::new(X::new(acc(x)).strength(acc(strength))));
    }

    #[wasm_bindgen(js_name = forceY)]
    pub fn force_y(&mut self, name: &str, y: &[f64], strength: &[f64]) {
        self.inner
            .add_force(name, Box::new(Y::new(acc(y)).strength(acc(strength))));
    }

    #[wasm_bindgen(js_name = forceCenter)]
    pub fn force_center(&mut self, name: &str, x: f64, y: f64, strength: f64) {
        self.inner
            .add_force(name, Box::new(Center::new(x, y).strength(strength)));
    }

    #[wasm_bindgen(js_name = forceRadial)]
    pub fn force_radial(&mut self, name: &str, radius: &[f64], x: f64, y: f64, strength: &[f64]) {
        self.inner.add_force(
            name,
            Box::new(Radial::new(acc(radius), x, y).strength(acc(strength))),
        );
    }

    /// A JS force: `f(alpha, positions, velocities, fixed)` is called each
    /// tick in installation order with fresh views and mutates them directly.
    /// It must not call any method of this `Simulation` (see `JsForce`).
    #[wasm_bindgen(js_name = forceCustom)]
    pub fn force_custom(&mut self, name: &str, f: Function) {
        self.inner.add_force(name, Box::new(JsForce { f }));
    }

    #[wasm_bindgen(js_name = removeForce)]
    pub fn remove_force(&mut self, name: &str) -> bool {
        self.inner.remove_force(name).is_some()
    }

    #[wasm_bindgen(js_name = forceNames)]
    pub fn force_names(&self) -> Vec<String> {
        self.inner.force_names().map(String::from).collect()
    }

    /// Closest node to `(x, y)` within `radius` (omit for unbounded); -1 if none.
    pub fn find(&self, x: f64, y: f64, radius: Option<f64>) -> i32 {
        self.inner
            .find(x, y, radius)
            .map(|i| i as i32)
            .unwrap_or(-1)
    }
}
