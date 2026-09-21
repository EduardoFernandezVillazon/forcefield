//! Tick-by-tick parity against the d3-force JS reference.
//!
//! Each fixture in `parity/fixtures/` was produced by `parity/gen.mjs`
//! running d3-force 2.1.1. The test rebuilds the same simulation here and
//! demands *bit-identical* positions after every tick. The only tolerance
//! is on the phyllotaxis initial positions, because `Math.cos`/`Math.sin`
//! are not guaranteed to round identically between V8 and libm; the tick
//! math itself uses nothing beyond `+ - * / sqrt`, which are correctly
//! rounded everywhere, so it must match exactly.

use forcefield::forces::{Center, Collide, Link, ManyBody, Radial, X, Y};
use forcefield::{Node, Simulation};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Fixture {
    name: String,
    params: Params,
    /// `[x, y, vx, vy, fx|null, fy|null]`
    nodes: Vec<[Option<f64>; 6]>,
    forces: Vec<ForceSpec>,
    ticks: usize,
    frames: Vec<Vec<f64>>,
    #[serde(rename = "alphaAfter")]
    alpha_after: f64,
    spiral: Option<Vec<[f64; 2]>>,
}

#[derive(Deserialize)]
struct Params {
    alpha: f64,
    #[serde(rename = "alphaMin")]
    alpha_min: f64,
    #[serde(rename = "alphaDecay")]
    alpha_decay: f64,
    #[serde(rename = "alphaTarget")]
    alpha_target: f64,
    #[serde(rename = "velocityDecay")]
    velocity_decay: f64,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
enum ForceSpec {
    #[serde(rename = "link")]
    Link {
        name: String,
        links: Vec<[usize; 2]>,
        distance: Vec<f64>,
        strength: Option<Vec<f64>>,
        iterations: usize,
    },
    #[serde(rename = "manyBody")]
    ManyBody {
        name: String,
        strength: Vec<f64>,
        theta: f64,
        #[serde(rename = "distanceMin")]
        distance_min: f64,
        #[serde(rename = "distanceMax")]
        distance_max: Option<f64>,
    },
    #[serde(rename = "center")]
    Center {
        name: String,
        x: f64,
        y: f64,
        strength: f64,
    },
    #[serde(rename = "collide")]
    Collide {
        name: String,
        radius: Vec<f64>,
        strength: f64,
        iterations: usize,
    },
    #[serde(rename = "x")]
    X {
        name: String,
        x: Vec<Option<f64>>,
        strength: Vec<f64>,
    },
    #[serde(rename = "y")]
    Y {
        name: String,
        y: Vec<Option<f64>>,
        strength: Vec<f64>,
    },
    #[serde(rename = "radial")]
    Radial {
        name: String,
        radius: Vec<f64>,
        x: f64,
        y: f64,
        strength: Vec<f64>,
    },
}

fn nan(v: Option<f64>) -> f64 {
    v.unwrap_or(f64::NAN)
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../parity/fixtures")
}

fn load(name: &str) -> Fixture {
    let path = fixtures_dir().join(format!("{name}.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}. Run `npm run gen` in parity/",
            path.display()
        )
    });
    serde_json::from_str(&text).unwrap()
}

fn ulps(a: f64, b: f64) -> i64 {
    (a.to_bits() as i64).wrapping_sub(b.to_bits() as i64).abs()
}

fn replay(fx: &Fixture) {
    // 1. Spiral initialisation (tolerance: transcendental functions).
    if let Some(spiral) = &fx.spiral {
        let sim = Simulation::with_count(spiral.len());
        let b = sim.bodies();
        for (i, [x, y]) in spiral.iter().enumerate() {
            assert!(
                (b.x(i) - x).abs() <= 1e-9 * x.abs().max(1.0)
                    && (b.y(i) - y).abs() <= 1e-9 * y.abs().max(1.0),
                "{}: spiral init node {i}: rust ({}, {}) vs js ({x}, {y})",
                fx.name,
                b.x(i),
                b.y(i)
            );
        }
    }

    // 2. Exact replay from the JS-initialised state.
    let nodes: Vec<Node> = fx
        .nodes
        .iter()
        .map(|n| Node {
            x: nan(n[0]),
            y: nan(n[1]),
            vx: nan(n[2]),
            vy: nan(n[3]),
            fx: nan(n[4]),
            fy: nan(n[5]),
        })
        .collect();
    let mut sim = Simulation::new(&nodes);
    sim.set_alpha(fx.params.alpha)
        .set_alpha_min(fx.params.alpha_min)
        .set_alpha_decay(fx.params.alpha_decay)
        .set_alpha_target(fx.params.alpha_target)
        .set_velocity_decay(fx.params.velocity_decay);

    for f in &fx.forces {
        match f {
            ForceSpec::Link {
                name,
                links,
                distance,
                strength,
                iterations,
            } => {
                let mut force = Link::new(links.iter().map(|[s, t]| (*s, *t)).collect())
                    .distance(distance.clone())
                    .iterations(*iterations);
                if let Some(s) = strength {
                    force = force.strength(s.clone());
                }
                sim.add_force(name.clone(), Box::new(force));
            }
            ForceSpec::ManyBody {
                name,
                strength,
                theta,
                distance_min,
                distance_max,
            } => {
                let force = ManyBody::new()
                    .strength(strength.clone())
                    .theta(*theta)
                    .distance_min(*distance_min)
                    .distance_max(distance_max.unwrap_or(f64::INFINITY));
                sim.add_force(name.clone(), Box::new(force));
            }
            ForceSpec::Center {
                name,
                x,
                y,
                strength,
            } => {
                sim.add_force(
                    name.clone(),
                    Box::new(Center::new(*x, *y).strength(*strength)),
                );
            }
            ForceSpec::Collide {
                name,
                radius,
                strength,
                iterations,
            } => {
                sim.add_force(
                    name.clone(),
                    Box::new(
                        Collide::new(radius.clone())
                            .strength(*strength)
                            .iterations(*iterations),
                    ),
                );
            }
            ForceSpec::X { name, x, strength } => {
                let xs: Vec<f64> = x.iter().map(|v| nan(*v)).collect();
                sim.add_force(
                    name.clone(),
                    Box::new(X::new(xs).strength(strength.clone())),
                );
            }
            ForceSpec::Y { name, y, strength } => {
                let ys: Vec<f64> = y.iter().map(|v| nan(*v)).collect();
                sim.add_force(
                    name.clone(),
                    Box::new(Y::new(ys).strength(strength.clone())),
                );
            }
            ForceSpec::Radial {
                name,
                radius,
                x,
                y,
                strength,
            } => {
                sim.add_force(
                    name.clone(),
                    Box::new(Radial::new(radius.clone(), *x, *y).strength(strength.clone())),
                );
            }
        }
    }

    assert_eq!(fx.frames.len(), fx.ticks);
    let n = nodes.len();
    for (t, frame) in fx.frames.iter().enumerate() {
        sim.tick(1);
        let pos = &sim.bodies().pos;
        for i in 0..n {
            for axis in 0..2 {
                let k = 2 * i + axis;
                let (got, want) = (pos[k], frame[k]);
                if got.to_bits() != want.to_bits() {
                    panic!(
                        "{}: tick {} node {} {}: rust {} vs js {} (diff {:e}, {} ulps)",
                        fx.name,
                        t + 1,
                        i,
                        ["x", "y"][axis],
                        got,
                        want,
                        got - want,
                        ulps(got, want)
                    );
                }
            }
        }
    }
    assert_eq!(
        sim.alpha().to_bits(),
        fx.alpha_after.to_bits(),
        "{}: final alpha",
        fx.name
    );
}

#[test]
fn ring_defaults() {
    replay(&load("ring-defaults"));
}

#[test]
fn consumer() {
    replay(&load("consumer"));
}

#[test]
fn coincident_radial() {
    replay(&load("coincident-radial"));
}

#[test]
fn many_body_theta() {
    replay(&load("many-body-theta"));
}
