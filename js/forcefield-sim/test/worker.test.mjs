import { test } from 'node:test'
import assert from 'node:assert/strict'
import { Worker } from 'node:worker_threads'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import { SimulationClient } from '../src/client.js'
import { Simulation, forceCollide, forceLink, forceManyBody, forceX, forceY, forceCenter } from '../src/index.js'

const wasmPath = fileURLToPath(new URL('../../../crates/forcefield-wasm/pkg-node/forcefield.js', import.meta.url))
const wasm = createRequire(import.meta.url)(wasmPath)
const entry = new URL('../test-support/worker-entry.mjs', import.meta.url)

function graph(n) {
  let s = 9
  const rnd = () => (s = (1664525 * s + 1013904223) % 4294967296) / 4294967296
  const nodes = Array.from({ length: n }, (_, i) => ({ id: `n${i}`, x: Math.round(rnd() * 800), y: Math.round(rnd() * 600) }))
  const links = []
  for (let i = 1; i < n; i++) links.push({ source: `n${Math.floor(rnd() * i)}`, target: `n${i}` })
  return { nodes, links }
}

function inline(g, pairs) {
  const nodes = g.nodes.map((d) => ({ ...d })), links = g.links.map((l) => ({ ...l }))
  const sim = new Simulation(wasm, nodes).alphaDecay(0.015).velocityDecay(0.35)
  sim.force('collide', forceCollide().radius(25).iterations(2))
    .force('link', forceLink(links).id((d) => d.id).distance(80).strength(0.3))
    .force('many-body', forceManyBody().strength(-150).distanceMin(5).distanceMax(250))
    .force('x', forceX().x(0).strength(0.05)).force('y', forceY().y(0).strength(0.05))
    .force('center', forceCenter(400, 300))
  if (pairs) {
    const ns = sim.nodes()
    const ps = pairs.map(([l, r]) => ({ left: ns[l], right: ns[r] }))
    const force = (alpha) => { for (const { left, right } of ps) { const s = 120 - (right.x - left.x); if (s <= 0) continue; const p = s * 0.1 * alpha * 0.5; left.vx -= p; right.vx += p } }
    force.initialize = () => {}
    sim.force('dag', force)
  }
  return sim
}

const nextFrame = (client) => new Promise((resolve) => { const off = client.on('frame.test', (pos, meta) => { client.on('frame.test', null); resolve({ pos: Float32Array.from(pos), meta }) }) })

test('worker frames match an inline simulation tick for tick (float32-rounded)', async () => {
  const g = graph(150)
  const pairs = [[1, 2], [3, 4], [10, 20]]
  const ref = inline(g, pairs)
  const worker = new Worker(entry, { workerData: { wasmPath } })
  const client = new SimulationClient(worker)
  const errors = []
  client.on('error', (m) => errors.push(m))
  const n = await client.init({ nodes: g.nodes.map((d) => ({ ...d })), links: g.links.map((l) => ({ ...l })), payload: { pairs }, running: false })
  assert.equal(n, 150)
  for (let t = 1; t <= 60; t++) {
    const p = nextFrame(client)
    client.step()
    const { pos, meta } = await p
    ref.tick()
    assert.equal(meta.tick, t)
    const rn = ref.nodes()
    for (let i = 0; i < n; i++) {
      assert.equal(pos[2 * i], Math.fround(rn[i].x), `tick ${t} node ${i} x`)
      assert.equal(pos[2 * i + 1], Math.fround(rn[i].y), `tick ${t} node ${i} y`)
    }
    assert.equal(meta.alpha, ref.alpha())
  }
  assert.deepEqual(errors, [])

  // pins through the client behave like fx/fy on the inline facade
  client.setFixed(0, 12, -3)
  ref.nodes()[0].fx = 12; ref.nodes()[0].fy = -3
  const p = nextFrame(client); client.step(); const { pos } = await p; ref.tick()
  assert.equal(pos[0], 12); assert.equal(pos[1], -3)
  assert.equal(await client.call('alpha'), ref.alpha())
  client.terminate()
})

test('free-running mode streams frames with back-pressure and ends at alphaMin', async () => {
  const g = graph(40)
  const worker = new Worker(entry, { workerData: { wasmPath } })
  const client = new SimulationClient(worker, { buffers: 2 })
  let frames = 0, lastTick = 0
  client.onFrame((pos, { tick }) => { frames++; assert.ok(tick > lastTick); lastTick = tick; assert.equal(pos.length, 80) })
  const ended = new Promise((r) => client.on('end', r))
  await client.init({ nodes: g.nodes, links: g.links, payload: {} })
  client.alphaDecay(0.2) // settle fast
  await ended
  assert.ok(frames > 10)
  assert.ok((await client.call('alpha')) < (await client.call('alphaMin')))
  client.terminate()
})

test('re-init with keepPositions carries positions and pins over by id', async () => {
  const g = graph(20)
  const worker = new Worker(entry, { workerData: { wasmPath } })
  const client = new SimulationClient(worker)
  await client.init({ nodes: g.nodes.map((d) => ({ ...d })), links: g.links, payload: {}, running: false })
  client.setFixed(3, 500, 500)
  let p = nextFrame(client); client.step(5); let { pos: before } = await p
  const grown = { nodes: [...g.nodes.map((d) => ({ id: d.id })), { id: 'new', x: 0, y: 0 }], links: [...g.links, { source: 'n0', target: 'new' }] }
  await client.init({ nodes: grown.nodes, links: grown.links, payload: {}, keepPositions: true, running: false })
  p = nextFrame(client); client.step(0); const { pos: after } = await p
  for (let i = 0; i < 20; i++) { assert.equal(after[2 * i], before[2 * i], `node ${i} x kept`); assert.equal(after[2 * i + 1], before[2 * i + 1]) }
  assert.equal(after[40], 0); assert.equal(after[41], 0)
  p = nextFrame(client); client.step(3); const { pos: pinned } = await p
  assert.equal(pinned[6], 500); assert.equal(pinned[7], 500) // pin survived re-init
  client.terminate()
})
