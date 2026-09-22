// Worker-side runtime: hosts a d3-shaped Simulation over forcefield-wasm
// and streams positions to the main thread as transferable Float32Array
// buffers. The consumer's worker entry calls `serve` with a way to load
// wasm and a `setup` hook that installs forces (including its own JS
// forces and per-tick hooks such as adaptive cooling).
//
//   import init, * as wasm from 'forcefield-wasm'
//   import { serve } from 'forcefield-sim/worker'
//   serve({
//     wasm: async () => { await init(); return wasm },
//     setup(sim, payload, { links }) { sim.force('link', forceLink(links)…) },
//   })
//
// Works with a Web Worker (`self`) or a Node `worker_threads` port.

import { Simulation } from './simulation.js'

function portOf(p) {
  // Normalise Web Worker global / MessagePort / Node parentPort to {post, on}.
  if (typeof p.addEventListener === 'function' && typeof p.postMessage === 'function' && !('on' in p)) {
    return { post: (m, t) => p.postMessage(m, t), on: (fn) => p.addEventListener('message', (e) => fn(e.data)) }
  }
  if (typeof p.on === 'function') {
    return { post: (m, t) => p.postMessage(m, t), on: (fn) => p.on('message', (m) => fn(m)) }
  }
  // Web Worker global scope without addEventListener typing (older engines)
  return { post: (m, t) => p.postMessage(m, t), on: (fn) => { p.onmessage = (e) => fn(e.data) } }
}

export function serve({ wasm, setup, port, tickScheduler } = {}) {
  if (typeof wasm !== 'function') throw new Error('forcefield-sim/worker: serve needs wasm(): Promise<module>')
  const io = portOf(port ?? globalThis)
  const schedule = tickScheduler ?? ((fn) => setTimeout(fn, 0))

  let wasmModule = null
  let sim = null
  let n = 0
  let tick = 0
  let running = false
  let context = null
  const free = []
  let ended = false

  function frame() {
    if (!sim || !running) return
    const buf = free.pop()
    if (!buf) return // back-pressure: every buffer is still on the main thread
    sim.tick(1)
    tick++
    const out = new Float32Array(buf)
    out.set(sim.views().pos)
    const alpha = sim.alpha()
    io.post({ type: 'frame', buf, tick, alpha, n }, [buf])
    if (alpha < sim.alphaMin()) {
      running = false
      if (!ended) {
        ended = true
        io.post({ type: 'end', tick })
      }
    }
  }

  function loop() {
    try { frame() } catch (e) { io.post({ type: 'error', message: String(e?.stack || e) }) }
    schedule(loop)
  }

  async function handle(m) {
    switch (m.type) {
      case 'init': {
        if (!wasmModule) wasmModule = await wasm()
        const prev = sim
        const nodes = m.nodes ?? []
        if (m.keepPositions && prev) {
          const byId = new Map(prev.nodes().map((p) => [p.id, p]))
          for (const nd of nodes) {
            const old = nd.id != null ? byId.get(nd.id) : undefined
            if (!old) continue
            if (nd.x == null) nd.x = old.x
            if (nd.y == null) nd.y = old.y
            if (nd.vx == null) nd.vx = old.vx
            if (nd.vy == null) nd.vy = old.vy
            if (nd.fx == null && old.fx != null) nd.fx = old.fx
            if (nd.fy == null && old.fy != null) nd.fy = old.fy
          }
        }
        prev?.free()
        sim = new Simulation(wasmModule, nodes)
        n = nodes.length
        tick = 0
        ended = false
        context = { links: m.links ?? [], wasm: wasmModule }
        if (setup) setup(sim, m.payload, context)
        free.length = 0
        for (let i = 0; i < (m.buffers ?? 3); i++) free.push(new ArrayBuffer(2 * n * 4))
        running = m.running ?? true
        io.post({ type: 'ready', n })
        break
      }
      case 'setup':
        if (sim && setup) setup(sim, m.payload, context)
        break
      case 'call': {
        if (!sim) break
        const value = sim[m.method](...(m.args ?? []))
        if (m.id != null) io.post({ type: 'result', id: m.id, value: value === sim ? undefined : value })
        if (m.method === 'alphaTarget' && m.args?.[0] > 0) { running = true; ended = false }
        break
      }
      case 'setFixed': {
        const node = sim?.nodes()[m.node]
        if (node) { node.fx = Number.isNaN(m.x) ? null : m.x; node.fy = Number.isNaN(m.y) ? null : m.y }
        break
      }
      case 'setPosition': {
        const node = sim?.nodes()[m.node]
        if (node) { node.x = m.x; node.y = m.y; node.vx = 0; node.vy = 0 }
        break
      }
      case 'run':
        running = !!m.running
        if (running) ended = false
        break
      case 'step': {
        // One tick and one frame regardless of `running`; for tests and manual stepping.
        if (!sim) break
        const buf = free.pop()
        if (!buf) { io.post({ type: 'error', message: 'step: no free buffer' }); break }
        sim.tick(m.iterations ?? 1)
        tick += m.iterations ?? 1
        new Float32Array(buf).set(sim.views().pos)
        io.post({ type: 'frame', buf, tick, alpha: sim.alpha(), n }, [buf])
        break
      }
      case 'buffer':
        if (m.buf.byteLength === 2 * n * 4) free.push(m.buf)
        break
      default:
        io.post({ type: 'error', message: `unknown message ${m.type}` })
    }
  }

  io.on((m) => { handle(m).catch((e) => io.post({ type: 'error', message: String(e?.stack || e) })) })
  loop()
  return { get simulation() { return sim } }
}
