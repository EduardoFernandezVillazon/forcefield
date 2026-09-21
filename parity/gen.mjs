// Golden-frame generator for the parity harness.
//
// Runs the real d3-force 2.1.1 on a handful of scenarios and records, for
// every tick, the position of every node. The Rust test in
// crates/forcefield/tests/parity.rs replays each scenario and demands
// bit-identical positions.
//
// Everything the Rust side needs is written into the fixture: resolved
// simulation parameters (so `Math.pow` never has to agree across
// platforms), the node state after d3's own initialisation, per-node /
// per-link accessor values, and the force order.

import * as d3 from 'd3-force'
import { writeFileSync, mkdirSync } from 'node:fs'

// Small seeded PRNG for *graph generation only*; the simulation's own
// randomness is d3's lcg, which the Rust port reproduces.
function mulberry32(a) {
  return function () {
    a |= 0; a = (a + 0x6D2B79F5) | 0
    let t = Math.imul(a ^ (a >>> 15), 1 | a)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const num = (v) => (v === Infinity ? null : Number.isNaN(v) ? null : v)
const perNode = (nodes, f) => nodes.map((n, i) => num(+f(n, i, nodes)))

function snapshotNodes(nodes) {
  return nodes.map((n) => [n.x, n.y, n.vx, n.vy, n.fx ?? null, n.fy ?? null])
}

function record(name, description, { nodes, sim, forces, ticks, spiral }) {
  const params = {
    alpha: sim.alpha(),
    alphaMin: sim.alphaMin(),
    alphaDecay: sim.alphaDecay(),
    alphaTarget: sim.alphaTarget(),
    velocityDecay: sim.velocityDecay(),
  }
  const init = snapshotNodes(nodes)
  const frames = []
  for (let t = 0; t < ticks; t++) {
    sim.tick()
    const f = new Array(nodes.length * 2)
    for (let i = 0; i < nodes.length; i++) { f[2 * i] = nodes[i].x; f[2 * i + 1] = nodes[i].y }
    frames.push(f)
  }
  const fixture = { name, description, d3Force: '2.1.1', params, nodes: init, forces, ticks, frames, alphaAfter: sim.alpha() }
  if (spiral) fixture.spiral = spiral
  mkdirSync('fixtures', { recursive: true })
  writeFileSync(`fixtures/${name}.json`, JSON.stringify(fixture))
  console.log(`${name}: ${nodes.length} nodes, ${ticks} ticks, alpha ${sim.alpha().toExponential(3)}`)
}

// ---------------------------------------------------------------- scenarios

// A. Defaults: spiral init, link + charge + center, run to the default 300-tick cooling.
{
  const n = 12
  const nodes = Array.from({ length: n }, () => ({}))
  const links = nodes.map((_, i) => ({ source: i, target: (i + 1) % n }))
  const sim = d3.forceSimulation(nodes).stop()
  const spiral = nodes.map((d) => [d.x, d.y])
  const link = d3.forceLink(links)
  const charge = d3.forceManyBody()
  const center = d3.forceCenter(0, 0)
  sim.force('link', link).force('charge', charge).force('center', center)
  record('ring-defaults', 'd3 defaults on a 12-node ring: spiral init, forceLink, forceManyBody, forceCenter', {
    nodes, sim, ticks: 300, spiral,
    forces: [
      { name: 'link', type: 'link', links: links.map((l) => [l.source.index, l.target.index]), distance: links.map((l, i) => num(+link.distance()(l, i, links))), strength: null, iterations: link.iterations() },
      { name: 'charge', type: 'manyBody', strength: perNode(nodes, charge.strength()), theta: charge.theta(), distanceMin: charge.distanceMin(), distanceMax: num(charge.distanceMax()) },
      { name: 'center', type: 'center', x: center.x(), y: center.y(), strength: center.strength() },
    ],
  })
}

// B. The consumer's shape (project brief §10) as cytoscape-d3-force installs it:
//    collide, link, many-body, x, y, center — with degree-divided link strength.
{
  const rnd = mulberry32(0xC0FFEE)
  const n = 300
  const W = 800, H = 600
  const nodes = Array.from({ length: n }, (_, i) => ({ id: `n${i}`, x: Math.round(rnd() * W), y: Math.round(rnd() * H) }))
  const edges = []
  for (let i = 1; i < n; i++) edges.push({ source: `n${Math.floor(rnd() * i)}`, target: `n${i}` })
  for (let k = 0; k < 120; k++) {
    const a = Math.floor(rnd() * n), b = Math.floor(rnd() * n)
    if (a !== b) edges.push({ source: `n${a}`, target: `n${b}` })
  }
  const degree = new Map()
  for (const e of edges) { degree.set(e.source, (degree.get(e.source) || 0) + 1); degree.set(e.target, (degree.get(e.target) || 0) + 1) }
  const fs = { centerForce: 0.05, repelForce: 150, linkForce: 0.3, linkDistance: 80 }

  const sim = d3.forceSimulation(nodes).stop()
  sim.alphaDecay(0.015).velocityDecay(0.35)
  const collide = d3.forceCollide().radius(25).strength(1).iterations(2)
  const link = d3.forceLink(edges).id((d) => d.id).distance(fs.linkDistance).strength((l) => {
    const minDeg = Math.max(1, Math.min(degree.get(l.source.id), degree.get(l.target.id)))
    return fs.linkForce / minDeg
  })
  const manyBody = d3.forceManyBody().strength(-fs.repelForce).distanceMin(5).distanceMax(250)
  const fx = d3.forceX().x(0).strength(fs.centerForce)
  const fy = d3.forceY().y(0).strength(fs.centerForce)
  const center = d3.forceCenter(W / 2, H / 2)
  sim.force('collide', collide).force('link', link).force('many-body', manyBody).force('x', fx).force('y', fy).force('center', center)
  const links = link.links()
  record('consumer', 'nemo-graph configuration through cytoscape-d3-force: collide/link/many-body/x/y/center, degree-divided link strength, 300 random-placed nodes', {
    nodes, sim, ticks: 300,
    forces: [
      { name: 'collide', type: 'collide', radius: perNode(nodes, collide.radius()), strength: collide.strength(), iterations: collide.iterations() },
      { name: 'link', type: 'link', links: links.map((l) => [l.source.index, l.target.index]), distance: links.map((l, i) => num(+link.distance()(l, i, links))), strength: links.map((l, i) => num(+link.strength()(l, i, links))), iterations: link.iterations() },
      { name: 'many-body', type: 'manyBody', strength: perNode(nodes, manyBody.strength()), theta: manyBody.theta(), distanceMin: manyBody.distanceMin(), distanceMax: num(manyBody.distanceMax()) },
      { name: 'x', type: 'x', x: perNode(nodes, fx.x()), strength: perNode(nodes, fx.strength()) },
      { name: 'y', type: 'y', y: perNode(nodes, fy.y()), strength: perNode(nodes, fy.strength()) },
      { name: 'center', type: 'center', x: center.x(), y: center.y(), strength: center.strength() },
    ],
  })
}

// C. Every node starts at the same point (jiggle everywhere), some pinned,
//    radial force, per-node collide radii, alphaTarget keeps it alive.
{
  const n = 40
  const nodes = Array.from({ length: n }, (_, i) => (i % 8 === 0 ? { fx: 30 * i - 60, fy: (i % 16) * 20 - 40 } : { x: 0, y: 0 }))
  const links = nodes.slice(1).map((_, i) => ({ source: i, target: i + 1 }))
  const sim = d3.forceSimulation(nodes).stop()
  sim.alphaTarget(0.3).velocityDecay(0.5)
  const charge = d3.forceManyBody()
  const collide = d3.forceCollide((d, i) => 3 + (i % 8))
  const radial = d3.forceRadial((d, i) => (i % 2 ? 50 : 100), 10, -5).strength(0.2)
  const link = d3.forceLink(links).distance(15)
  sim.force('charge', charge).force('collide', collide).force('radial', radial).force('link', link)
  record('coincident-radial', 'all free nodes start coincident, five pinned, forceRadial, per-node collide radii, alphaTarget 0.3', {
    nodes, sim, ticks: 200,
    forces: [
      { name: 'charge', type: 'manyBody', strength: perNode(nodes, charge.strength()), theta: charge.theta(), distanceMin: charge.distanceMin(), distanceMax: num(charge.distanceMax()) },
      { name: 'collide', type: 'collide', radius: perNode(nodes, collide.radius()), strength: collide.strength(), iterations: collide.iterations() },
      { name: 'radial', type: 'radial', radius: perNode(nodes, radial.radius()), x: radial.x(), y: radial.y(), strength: perNode(nodes, radial.strength()) },
      { name: 'link', type: 'link', links: links.map((l) => [l.source.index, l.target.index]), distance: links.map((l, i) => num(+link.distance()(l, i, links))), strength: null, iterations: link.iterations() },
    ],
  })
}

// D. Larger, no links: exercises Barnes-Hut with a loose theta, a distance
//    cap, mixed-sign per-node strengths and per-node x/y targets.
{
  const n = 500
  const nodes = Array.from({ length: n }, () => ({}))
  const sim = d3.forceSimulation(nodes).stop()
  const spiral = nodes.map((d) => [d.x, d.y])
  const charge = d3.forceManyBody().strength((d, i) => [-20, -50, 15][i % 3]).theta(0.5).distanceMax(200)
  const fx = d3.forceX((d, i) => (i % 10) * 60 - 270).strength(0.03)
  const fy = d3.forceY((d, i) => Math.floor(i / 50) * 60 - 270).strength(0.03)
  sim.force('charge', charge).force('x', fx).force('y', fy)
  record('many-body-theta', '500 spiral-initialised nodes, mixed-sign charges, theta 0.5, distanceMax 200, grid x/y targets', {
    nodes, sim, ticks: 120, spiral,
    forces: [
      { name: 'charge', type: 'manyBody', strength: perNode(nodes, charge.strength()), theta: charge.theta(), distanceMin: charge.distanceMin(), distanceMax: num(charge.distanceMax()) },
      { name: 'x', type: 'x', x: perNode(nodes, fx.x()), strength: perNode(nodes, fx.strength()) },
      { name: 'y', type: 'y', y: perNode(nodes, fy.y()), strength: perNode(nodes, fy.strength()) },
    ],
  })
}
