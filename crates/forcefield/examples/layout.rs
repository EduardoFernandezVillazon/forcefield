//! One-shot headless layout: the native target's reason to exist (brief §5).
//!
//! Reads `{"nodes":[{"id":…,"x"?:…,"y"?:…}],"links":[{"source":id,"target":id}]}`
//! on stdin, runs the consumer-shaped force model to convergence, writes
//! `{"positions":{id:[x,y]}}` on stdout.
//!
//! cargo run --release -p forcefield --example layout < graph.json

use forcefield::forces::{Collide, Link, ManyBody, X, Y};
use forcefield::{Node, Simulation};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;

#[derive(Deserialize)]
struct InNode {
    id: String,
    x: Option<f64>,
    y: Option<f64>,
}
#[derive(Deserialize)]
struct InLink {
    source: String,
    target: String,
}
#[derive(Deserialize)]
struct Graph {
    nodes: Vec<InNode>,
    links: Vec<InLink>,
}

fn main() {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .expect("read stdin");
    let g: Graph = serde_json::from_str(&text).expect("graph json");

    let index: HashMap<&str, usize> = g
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let nodes: Vec<Node> = g
        .nodes
        .iter()
        .map(|n| Node {
            x: n.x.unwrap_or(f64::NAN),
            y: n.y.unwrap_or(f64::NAN),
            ..Node::UNSET
        })
        .collect();
    let links: Vec<(usize, usize)> = g
        .links
        .iter()
        .map(|l| (index[l.source.as_str()], index[l.target.as_str()]))
        .collect();
    let mut degree = vec![0usize; nodes.len()];
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
    sim.add_force("collide", Box::new(Collide::new(25.0).iterations(2)))
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
        .add_force("y", Box::new(Y::new(0.0).strength(0.05)));
    let ticks = sim.run(2_000);

    let b = sim.bodies();
    let positions: serde_json::Map<String, serde_json::Value> = g
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), serde_json::json!([b.x(i), b.y(i)])))
        .collect();
    println!(
        "{}",
        serde_json::json!({ "ticks": ticks, "alpha": sim.alpha(), "positions": positions })
    );
}
