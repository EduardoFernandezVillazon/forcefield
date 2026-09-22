import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import cytoscape from 'cytoscape'
import register from '../src/index.js'

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
