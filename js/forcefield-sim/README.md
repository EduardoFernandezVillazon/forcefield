# forcefield-sim

The d3-force API over [forcefield](https://github.com/EduardoFernandezVillazon/forcefield),
a Rust/wasm port of d3-force with tick-for-tick parity, plus a Web Worker
runtime and client so the simulation runs off the main thread and streams
positions as transferable buffers.

## The facade

```js
import init, * as wasm from 'forcefield-wasm'
import { Simulation, forceLink, forceManyBody, forceCenter } from 'forcefield-sim'
await init()
const sim = new Simulation(wasm, nodes)          // nodes: plain objects, x/y/fx/fy optional + your data
sim.force('link', forceLink(links).id((d) => d.id).distance(80))
   .force('charge', forceManyBody().strength(-150))
   .force('center', forceCenter(0, 0))
sim.on('tick', () => …).restart()                // or sim.tick(n) if you own the loop
sim.nodes()[0].fx = 10                            // proxies: x y vx vy fx fy live in wasm memory
```

`nodes()` returns proxies over wasm memory, so code written for d3-force,
including custom JS forces that read and write `node.x`/`node.vx`, works
unchanged. One rule: never call a simulation method from inside a force.
Positions are also available as a `Float64Array` view via `sim.views().pos`.

## In a worker

Your worker entry installs the forces (including your own JS forces and
per-tick hooks) and hands the rest to the runtime:

```js
// sim.worker.js
import init, * as wasm from 'forcefield-wasm'
import { serve } from 'forcefield-sim/worker'
import { forceLink, forceManyBody } from 'forcefield-sim'
serve({
  wasm: async () => { await init(); return wasm },
  setup(sim, payload, { links }) {
    sim.force('link', forceLink(links).id((d) => d.id)).force('charge', forceManyBody())
    // custom forces, adaptive cooling, anything that touches `sim`, lives here
  },
})
```

```js
// main thread
import { SimulationClient } from 'forcefield-sim/client'
const client = new SimulationClient(new Worker(new URL('./sim.worker.js', import.meta.url), { type: 'module' }))
client.onFrame((pos, { tick, alpha }) => renderer.setPositions(pos))   // Float32Array [x0, y0, …]
await client.init({ nodes, links, payload: { config } })
client.setFixed(i, x, y); client.alphaTarget(0.3); client.stop()
await client.init({ nodes: moreNodes, links: moreLinks, payload, keepPositions: true })  // structure change
```

Frames are `Float32Array` buffers handed back to the worker after your
handler runs; with three in flight the worker skips ticks rather than
queueing stale frames when the main thread falls behind. `keepPositions`
carries position, velocity and pins over by `id` across a re-init, which
is how incremental graph growth works. No `SharedArrayBuffer` is used, so
no cross-origin isolation headers are needed.

Measured in WebKitGTK: the main thread stays at 60 Hz at 20 000 nodes
while the simulation ticks at its own rate (see the edgelit repository's
`docs/WORKER.md`).

The runtime also runs under Node's `worker_threads` (pass `port: parentPort`),
which is how it is tested. BSD-3-Clause.
