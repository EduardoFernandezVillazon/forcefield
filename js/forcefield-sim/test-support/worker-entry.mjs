// A consumer-style worker entry, used by test/worker.test.mjs; lives outside test/ because node --test would run it.
import { parentPort, workerData } from 'node:worker_threads'
import { createRequire } from 'node:module'
import { serve } from '../src/worker.js'
import { forceCollide, forceLink, forceManyBody, forceX, forceY, forceCenter } from '../src/index.js'

const require = createRequire(import.meta.url)

serve({
  port: parentPort,
  wasm: async () => require(workerData.wasmPath),
  setup(sim, payload, { links }) {
    const cfg = payload?.config ?? {}
    sim.alphaDecay(cfg.alphaDecay ?? 0.015).velocityDecay(cfg.velocityDecay ?? 0.35)
    sim.force('collide', forceCollide().radius(25).iterations(2))
      .force('link', forceLink(links).id((d) => d.id).distance(80).strength(0.3))
      .force('many-body', forceManyBody().strength(-150).distanceMin(5).distanceMax(250))
      .force('x', forceX().x(0).strength(0.05))
      .force('y', forceY().y(0).strength(0.05))
      .force('center', forceCenter(400, 300))
    // a consumer JS force, the shape nemo uses
    if (payload?.pairs) {
      const nodes = sim.nodes()
      const pairs = payload.pairs.map(([l, r]) => ({ left: nodes[l], right: nodes[r] }))
      const force = (alpha) => { for (const { left, right } of pairs) { const s = 120 - (right.x - left.x); if (s <= 0) continue; const p = s * 0.1 * alpha * 0.5; left.vx -= p; right.vx += p } }
      force.initialize = () => {}
      sim.force('dag', force)
    } else sim.force('dag', null)
  },
})
