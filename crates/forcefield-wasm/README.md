# forcefield-wasm

wasm-bindgen build of [forcefield](https://github.com/EduardoFernandezVillazon/forcefield), a Rust port of d3-force with
tick-for-tick parity against the JS reference. Positions, velocities and
pins are `Float64Array` views onto wasm memory: no per-tick copy.

```js
import init, { Simulation } from 'forcefield-wasm'
await init()
const sim = Simulation.fromPositions(Float64Array.of(0, 0, 10, 3, NaN, NaN)) // NaN = spiral init
sim.forceManyBody('charge', [-30], 0.9, 1, Infinity)          // length-1 array = constant
sim.forceLink('link', Uint32Array.of(0, 1), Uint32Array.of(1, 2), [30], [], 1)
sim.tick(1)
const pos = sim.positions()   // [x0, y0, x1, y1, …] view; re-fetch after any non-tick call
```

Custom JS forces: `sim.forceCustom(name, (alpha, pos, vel, fix) => …)`, called
each tick with fresh views; do not call the simulation from inside it. For
cytoscape.js use `cytoscape-forcefield`. Full API in `forcefield.d.ts`.
BSD-3-Clause; derivative of d3-force (Mike Bostock).
