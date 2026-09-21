//! Wall-clock for the consumer-shaped configuration (project brief §10) at
//! a few sizes. Compare with `node parity/bench.mjs`, which runs the same
//! graph through d3-force.
//!
//! cargo run --release -p forcefield --example bench

use forcefield::forces::{Center, Collide, Link, ManyBody, X, Y};
use forcefield::{Lcg, Node, Simulation};
use std::time::Instant;

fn build(n: usize) -> Simulation {
    let mut rnd = Lcg::new();
    let nodes: Vec<Node> = (0..n)
        .map(|_| Node::at((rnd.next() * 800.0).round(), (rnd.next() * 600.0).round()))
        .collect();
    let mut links: Vec<(usize, usize)> = (1..n)
        .map(|i| ((rnd.next() * i as f64) as usize, i))
        .collect();
    for _ in 0..n / 3 {
        let a = (rnd.next() * n as f64) as usize;
        let b = (rnd.next() * n as f64) as usize;
        if a != b {
            links.push((a, b));
        }
    }
    let mut degree = vec![0usize; n];
    for &(a, b) in &links {
        degree[a] += 1;
        degree[b] += 1;
    }
    let strengths: Vec<f64> = links
        .iter()
        .map(|&(a, b)| 0.3 / degree[a].min(degree[b]).max(1) as f64)
        .collect();

    let mut sim = Simulation::new(&nodes);
    sim.set_alpha_decay(0.015).set_velocity_decay(0.35);
    sim.add_force(
        "collide",
        Box::new(Collide::new(25.0).strength(1.0).iterations(2)),
    )
    .add_force(
        "link",
        Box::new(Link::new(links).distance(80.0).strength(strengths)),
    )
    .add_force(
        "many-body",
        Box::new(
            ManyBody::new()
                .strength(-150.0)
                .distance_min(5.0)
                .distance_max(250.0),
        ),
    )
    .add_force("x", Box::new(X::new(0.0).strength(0.05)))
    .add_force("y", Box::new(Y::new(0.0).strength(0.05)))
    .add_force("center", Box::new(Center::new(400.0, 300.0)));
    sim
}

fn main() {
    let ticks = 300;
    println!("{:>7} {:>10} {:>12}", "nodes", "ms/tick", "total ms");
    for &n in &[500usize, 1_000, 2_000, 5_000, 10_000] {
        let mut sim = build(n);
        let t = Instant::now();
        sim.tick(ticks);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        println!("{:>7} {:>10.3} {:>12.1}", n, ms / ticks as f64, ms);
    }
}
