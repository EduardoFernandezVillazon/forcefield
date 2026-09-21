import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import cytoscape from 'cytoscape'
import * as d3 from 'd3-force'
import register, { Simulation, forceManyBody, forceLink, forceCollide, forceX, forceY, forceCenter } from '../src/index.js'

const wasm = createRequire(import.meta.url)('../../../crates/forcefield-wasm/pkg-node/forcefield.js')

function graph(n, extra = 20) {
  let s = 7
  const rnd = () => (s = (1664525 * s + 1013904223) % 4294967296) / 4294967296
  const nodes = Array.from({ length: n }, (_, i) => ({ id: `n${i}`, x: Math.round(rnd() * 800), y: Math.round(rnd() * 600) }))
  const links = []
  for (let i = 1; i < n; i++) links.push({ source: `n${Math.floor(rnd() * i)}`, target: `n${i}` })
  for (let k = 0; k < extra; k++) {
    const a = Math.floor(rnd() * n), b = Math.floor(rnd() * n)
    if (a !== b) links.push({ source: `n${a}`, target: `n${b}` })
  }
  return { nodes, links }
}

const consumerConfig = {
  alphaDecay: 0.015, velocityDecay: 0.35,
  collideRadius: 25, collideStrength: 1, collideIterations: 2,
  manyBodyStrength: -150, manyBodyDistanceMin: 5, manyBodyDistanceMax: 250,
  linkId: (d) => d.id, linkDistance: 80,
  linkStrength: (link) => 0.3 / Math.max(1, Math.min(link.source.degree?.(false) ?? 1, link.target.degree?.(false) ?? 1)),
  xStrength: 0.05, xX: 0, yStrength: 0.05, yY: 0,
}

test('facade matches d3-force tick for tick, including a custom JS force', () => {
  const g = graph(120)
  const mk = () => ({ nodes: g.nodes.map((n) => ({ ...n })), links: g.links.map((l) => ({ ...l })) })

  const dagForce = () => {
    let pairs = []
    function force(alpha) {
      for (const { left, right } of pairs) {
        const shortfall = 120 - (right.x - left.x)
        if (shortfall <= 0) continue
        const push = shortfall * 0.1 * alpha * 0.5
        left.vx -= push
        right.vx += push
      }
    }
    force.initialize = (nodes) => { pairs = nodes.slice(1).map((n, i) => ({ left: nodes[i], right: n })) }
    return force
  }

  const a = mk()
  const ref = d3.forceSimulation(a.nodes).stop().alphaDecay(0.015).velocityDecay(0.35)
  ref.force('collide', d3.forceCollide().radius(25).iterations(2))
    .force('link', d3.forceLink(a.links).id((d) => d.id).distance(80).strength(consumerConfig.linkStrength))
    .force('many-body', d3.forceManyBody().strength(-150).distanceMin(5).distanceMax(250))
    .force('x', d3.forceX().x(0).strength(0.05))
    .force('y', d3.forceY().y(0).strength(0.05))
    .force('center', d3.forceCenter(400, 300))
    .force('dag', dagForce())

  const b = mk()
  const sim = new Simulation(wasm, b.nodes).alphaDecay(0.015).velocityDecay(0.35)
  sim.force('collide', forceCollide().radius(25).iterations(2))
    .force('link', forceLink(b.links).id((d) => d.id).distance(80).strength(consumerConfig.linkStrength))
    .force('many-body', forceManyBody().strength(-150).distanceMin(5).distanceMax(250))
    .force('x', forceX().x(0).strength(0.05))
    .force('y', forceY().y(0).strength(0.05))
    .force('center', forceCenter(400, 300))
    .force('dag', dagForce())

  for (let t = 1; t <= 150; t++) {
    ref.tick(); sim.tick()
    const rn = ref.nodes(), sn = sim.nodes()
    for (let i = 0; i < rn.length; i++) {
      assert.equal(sn[i].x, rn[i].x, `tick ${t} node ${i} x`)
      assert.equal(sn[i].y, rn[i].y, `tick ${t} node ${i} y`)
    }
  }
  assert.equal(sim.alpha(), ref.alpha())

  // pins, unpins and node lookup behave like d3's plain objects
  const n0 = sim.nodes()[0]
  n0.fx = 12; n0.fy = -3
  sim.tick()
  assert.equal(n0.x, 12); assert.equal(n0.y, -3); assert.equal(n0.vx, 0)
  n0.fx = null; delete n0.fy
  assert.equal(n0.fx, undefined); assert.equal('fy' in n0, false)
  assert.equal(n0.id, 'n0')
  assert.equal(sim.find(12, -3), n0)
  sim.free()
})

test('namespaced events and removal work like d3-dispatch', async () => {
  const sim = new Simulation(wasm, [{ x: 0, y: 0 }, { x: 10, y: 0 }]).alphaDecay(0.5)
  sim.force('charge', forceManyBody())
  let ticks = 0, ended = 0
  sim.on('tick.a', () => ticks++).on('end.b', () => ended++)
  sim.on('tick.a', null)
  sim.restart()
  await new Promise((r) => sim.on('end.wait', r))
  assert.equal(ticks, 0)
  assert.equal(ended, 1)
  assert.ok(sim.alpha() < sim.alphaMin())
  sim.free()
})

test('cytoscape layout runs headless, positions nodes, honours the consumer config', async () => {
  register(cytoscape, wasm)
  const g = graph(60)
  const cy = cytoscape({
    headless: true,
    styleEnabled: false,
    elements: [
      ...g.nodes.map((n) => ({ data: { id: n.id }, position: { x: n.x, y: n.y } })),
      ...g.links.map((l, i) => ({ data: { id: `e${i}`, source: l.source, target: l.target } })),
    ],
  })
  const before = cy.nodes().map((n) => ({ ...n.position() }))
  const layout = cy.layout({ name: 'forcefield', animate: true, infinite: false, maxIterations: 200, fit: false, ...consumerConfig })
  const stopped = new Promise((r) => layout.one('layoutstop', r))
  layout.run()
  const sim = layout.simulation
  assert.ok(sim, 'layout exposes its simulation')
  assert.equal(sim.nodes().length, 60)
  assert.equal(sim.alphaDecay(), 0.015)
  assert.deepEqual(sim.force('link').links().map((l) => l.source.id).slice(0, 2), [g.links[0].source, g.links[1].source])
  await stopped
  const after = cy.nodes().map((n) => n.position())
  let moved = 0
  after.forEach((p, i) => {
    assert.ok(Number.isFinite(p.x) && Number.isFinite(p.y))
    if (p.x !== before[i].x || p.y !== before[i].y) moved++
  })
  assert.ok(moved > 50, `expected most nodes to move, got ${moved}`)
  cy.destroy()
})
