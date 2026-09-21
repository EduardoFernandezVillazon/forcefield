// The JS side of the benchmark in crates/forcefield/examples/bench.rs:
// same graph generator (d3's lcg, seeded 1), same forces, 300 ticks.
import * as d3 from 'd3-force'

function lcg() { let s = 1; return () => (s = (1664525 * s + 1013904223) % 4294967296) / 4294967296 }

function build(n) {
  const rnd = lcg()
  const nodes = Array.from({ length: n }, () => ({ x: Math.round(rnd() * 800), y: Math.round(rnd() * 600) }))
  const links = []
  for (let i = 1; i < n; i++) links.push({ source: Math.floor(rnd() * i), target: i })
  for (let k = 0; k < Math.floor(n / 3); k++) {
    const a = Math.floor(rnd() * n), b = Math.floor(rnd() * n)
    if (a !== b) links.push({ source: a, target: b })
  }
  const degree = new Array(n).fill(0)
  for (const l of links) { degree[l.source]++; degree[l.target]++ }
  const sim = d3.forceSimulation(nodes).stop().alphaDecay(0.015).velocityDecay(0.35)
  sim.force('collide', d3.forceCollide().radius(25).strength(1).iterations(2))
    .force('link', d3.forceLink(links).distance(80).strength((l) => 0.3 / Math.max(1, Math.min(degree[l.source.index], degree[l.target.index]))))
    .force('many-body', d3.forceManyBody().strength(-150).distanceMin(5).distanceMax(250))
    .force('x', d3.forceX().x(0).strength(0.05))
    .force('y', d3.forceY().y(0).strength(0.05))
    .force('center', d3.forceCenter(400, 300))
  return sim
}

const ticks = 300
console.log('  nodes    ms/tick     total ms')
for (const n of [500, 1000, 2000, 5000, 10000]) {
  const sim = build(n)
  const t = performance.now()
  sim.tick(ticks)
  const ms = performance.now() - t
  console.log(`${String(n).padStart(7)} ${(ms / ticks).toFixed(3).padStart(10)} ${ms.toFixed(1).padStart(12)}`)
}
