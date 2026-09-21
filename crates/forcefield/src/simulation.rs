//! Port of d3-force `simulation.js`, minus the timer and event dispatch:
//! the caller owns the loop and calls [`Simulation::tick`].

use crate::bodies::{Bodies, Node};
use crate::forces::Force;
use crate::lcg::Random;

const INITIAL_RADIUS: f64 = 10.0;

/// `Math.PI * (3 - Math.sqrt(5))`
fn initial_angle() -> f64 {
    std::f64::consts::PI * (3.0 - 5f64.sqrt())
}

pub struct Simulation {
    bodies: Bodies,
    alpha: f64,
    alpha_min: f64,
    alpha_decay: f64,
    alpha_target: f64,
    /// Stored as d3 stores it: `1 - velocityDecay`.
    velocity_decay: f64,
    /// Insertion-ordered, like the JS `Map`; replacing a name keeps its slot.
    forces: Vec<(String, Box<dyn Force>)>,
    random: Random,
}

impl Simulation {
    /// `forceSimulation(nodes)`: index the nodes and initialise unset
    /// positions on the phyllotaxis spiral.
    pub fn new(nodes: &[Node]) -> Simulation {
        let alpha_min: f64 = 0.001;
        let mut sim = Simulation {
            bodies: Bodies::from_nodes(nodes),
            alpha: 1.0,
            alpha_min,
            alpha_decay: 1.0 - alpha_min.powf(1.0 / 300.0),
            alpha_target: 0.0,
            velocity_decay: 0.6,
            forces: Vec::new(),
            random: Random::lcg(),
        };
        sim.initialize_nodes();
        sim
    }

    /// `n` nodes, all unset (spiral initialisation).
    pub fn with_count(n: usize) -> Simulation {
        Simulation::new(&vec![Node::UNSET; n])
    }

    fn initialize_nodes(&mut self) {
        let b = &mut self.bodies;
        let angle = initial_angle();
        for i in 0..b.len() {
            let (fx, fy) = (b.fixed[2 * i], b.fixed[2 * i + 1]);
            if !fx.is_nan() {
                b.pos[2 * i] = fx;
            }
            if !fy.is_nan() {
                b.pos[2 * i + 1] = fy;
            }
            if b.pos[2 * i].is_nan() || b.pos[2 * i + 1].is_nan() {
                let radius = INITIAL_RADIUS * (0.5 + i as f64).sqrt();
                let a = i as f64 * angle;
                b.pos[2 * i] = radius * a.cos();
                b.pos[2 * i + 1] = radius * a.sin();
            }
            if b.vel[2 * i].is_nan() || b.vel[2 * i + 1].is_nan() {
                b.vel[2 * i] = 0.0;
                b.vel[2 * i + 1] = 0.0;
            }
        }
    }

    /// `simulation.tick(iterations)`.
    pub fn tick(&mut self, iterations: usize) {
        for _ in 0..iterations {
            self.alpha += (self.alpha_target - self.alpha) * self.alpha_decay;

            for (_, force) in self.forces.iter_mut() {
                force.apply(&mut self.bodies, self.alpha, &mut self.random);
            }

            let b = &mut self.bodies;
            let vd = self.velocity_decay;
            for i in 0..b.len() {
                for axis in 0..2 {
                    let k = 2 * i + axis;
                    if b.fixed[k].is_nan() {
                        b.vel[k] *= vd;
                        b.pos[k] += b.vel[k];
                    } else {
                        b.pos[k] = b.fixed[k];
                        b.vel[k] = 0.0;
                    }
                }
            }
        }
    }

    /// Tick until `alpha < alphaMin`, as the JS timer loop would. Returns
    /// the number of ticks run. With `alphaDecay` 0 this never stops, so a
    /// `max_ticks` cap is required.
    pub fn run(&mut self, max_ticks: usize) -> usize {
        let mut n = 0;
        while self.alpha >= self.alpha_min && n < max_ticks {
            self.tick(1);
            n += 1;
        }
        n
    }

    // --- nodes ---------------------------------------------------------

    pub fn bodies(&self) -> &Bodies {
        &self.bodies
    }

    pub fn bodies_mut(&mut self) -> &mut Bodies {
        &mut self.bodies
    }

    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// `simulation.nodes(nodes)`: replace the node set and re-initialise
    /// every force.
    pub fn set_nodes(&mut self, nodes: &[Node]) {
        self.bodies = Bodies::from_nodes(nodes);
        self.initialize_nodes();
        self.reinitialize_forces();
    }

    fn reinitialize_forces(&mut self) {
        for (_, force) in self.forces.iter_mut() {
            force.initialize(&self.bodies, &mut self.random);
        }
    }

    // --- parameters ----------------------------------------------------

    pub fn alpha(&self) -> f64 {
        self.alpha
    }
    pub fn set_alpha(&mut self, v: f64) -> &mut Self {
        self.alpha = v;
        self
    }
    pub fn alpha_min(&self) -> f64 {
        self.alpha_min
    }
    pub fn set_alpha_min(&mut self, v: f64) -> &mut Self {
        self.alpha_min = v;
        self
    }
    pub fn alpha_decay(&self) -> f64 {
        self.alpha_decay
    }
    pub fn set_alpha_decay(&mut self, v: f64) -> &mut Self {
        self.alpha_decay = v;
        self
    }
    pub fn alpha_target(&self) -> f64 {
        self.alpha_target
    }
    pub fn set_alpha_target(&mut self, v: f64) -> &mut Self {
        self.alpha_target = v;
        self
    }
    /// The user-facing value (d3 default 0.4).
    pub fn velocity_decay(&self) -> f64 {
        1.0 - self.velocity_decay
    }
    pub fn set_velocity_decay(&mut self, v: f64) -> &mut Self {
        self.velocity_decay = 1.0 - v;
        self
    }

    /// `simulation.randomSource(fn)`: swap the random stream and
    /// re-initialise forces.
    pub fn set_random_source(&mut self, random: Random) -> &mut Self {
        self.random = random;
        self.reinitialize_forces();
        self
    }

    // --- forces --------------------------------------------------------

    /// `simulation.force(name, force)`: install (or replace in place) a
    /// named force, initialising it against the current nodes.
    pub fn add_force(&mut self, name: impl Into<String>, mut force: Box<dyn Force>) -> &mut Self {
        let name = name.into();
        force.initialize(&self.bodies, &mut self.random);
        if let Some(slot) = self.forces.iter_mut().find(|(n, _)| *n == name) {
            slot.1 = force;
        } else {
            self.forces.push((name, force));
        }
        self
    }

    /// `simulation.force(name, null)`.
    pub fn remove_force(&mut self, name: &str) -> Option<Box<dyn Force>> {
        let i = self.forces.iter().position(|(n, _)| n == name)?;
        Some(self.forces.remove(i).1)
    }

    pub fn force(&self, name: &str) -> Option<&dyn Force> {
        self.forces
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, f)| f.as_ref())
    }

    pub fn force_mut(&mut self, name: &str) -> Option<&mut Box<dyn Force>> {
        self.forces
            .iter_mut()
            .find(|(n, _)| n == name)
            .map(|(_, f)| f)
    }

    /// Re-run `initialize` on one force after changing its accessors.
    pub fn reinitialize_force(&mut self, name: &str) {
        if let Some(i) = self.forces.iter().position(|(n, _)| n == name) {
            self.forces[i].1.initialize(&self.bodies, &mut self.random);
        }
    }

    pub fn force_names(&self) -> impl Iterator<Item = &str> {
        self.forces.iter().map(|(n, _)| n.as_str())
    }

    // --- queries -------------------------------------------------------

    /// `simulation.find(x, y, radius)`: linear scan for the closest node.
    pub fn find(&self, x: f64, y: f64, radius: Option<f64>) -> Option<usize> {
        let mut radius = match radius {
            None => f64::INFINITY,
            Some(r) => r * r,
        };
        let mut closest = None;
        for i in 0..self.bodies.len() {
            let dx = x - self.bodies.x(i);
            let dy = y - self.bodies.y(i);
            let d2 = dx * dx + dy * dy;
            if d2 < radius {
                closest = Some(i);
                radius = d2;
            }
        }
        closest
    }
}
