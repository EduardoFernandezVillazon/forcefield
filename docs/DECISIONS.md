# Decisions

Written 2026-09-21 from the project brief that started this repository.

## 0. The diagnostic that gates whether this helps you (NOT YET RUN)

The premise is that a graph view running `cytoscape-d3-force` gets clunky
because the force simulation runs in JS on the main thread. The brief's own
numbers say the force math is only a few milliseconds per frame at ~1 000
nodes; cytoscape writing every node's position each tick (invalidating style
and bounding boxes) plus the redraw is the rest, and no simulation in any
language touches that.

**Run this before integrating:** open a large graph, let the simulation
settle (adaptive cooling stops it), then pan, zoom and drag.

- Still clunky with nothing simulating ⇒ the cost is element count and
  rendering. This project does not help; replace the renderer instead.
- Smooth when settled, clunky only while it moves ⇒ the tick loop is the
  cost, and this project is aimed correctly. Expect roughly 2× on the force
  math from wasm (see the README table), not 2× on the frame.

Write the answer here when you have it.

## 1. Port d3-force 2.1.1, which is also 3.0.0

Verified by `diff -r` on the npm tarballs: d3-force 2.1.1 and 3.0.0 have
identical `src/`; d3-quadtree 2.0.0 and 3.0.1 likewise. The 3.x bump was
packaging (ESM only) and licence text (BSD-3 → ISC). The port cites 2.1.1
because that is the consumer's resolved version and the BSD-3 notice is the
one vendored.

## 2. Name: forcefield

No "d3" in the name (BSD-3 clause 3, no implied endorsement). `forcefield`
was free on crates.io and npm on 2026-09-21. The adapter is
`cytoscape-forcefield`, the wasm package `forcefield-wasm`.

## 3. `f64`, not `f32`, across the wasm boundary

The brief said "expose positions as a `Float32Array` view". Using `f32`
would break bit-parity with d3 (JS numbers are doubles) and the parity
harness is the whole point. The essential property, a zero-copy view onto
wasm memory, holds equally for `Float64Array`. Cost: 16 KB instead of 8 KB
per 1 000 nodes, irrelevant.

## 4. Exact parity, and what had to be true for it

The tick math uses only `+ - * / sqrt`, all correctly rounded in both
runtimes, so results match to the bit once operation order and traversal
order are preserved. Things that mattered:

- **Operation order** is copied from the JS, including left-to-right
  associativity such as `x * value * alpha / l`.
- **Quadtree traversal order**: `visit` pops children 0,1,2,3 (pushed
  3,2,1,0); the many-body force sums contributions in that order and draws
  jiggle from the shared LCG in that order.
- **Integer extents and doubling** in `cover` are what keep quadrant
  midpoints exact; ported verbatim.
- **Coincident points** chain through `next` exactly as in JS, including
  d3-force's own quirk that `forceCollide` only looks at the head of a chain.
- **Transcendentals** appear only in initialisation (`cos`/`sin` for the
  spiral, `pow` for the default `alphaDecay`) and may differ by an ULP
  between V8 and libm. The harness therefore compares the spiral with a
  tolerance and starts the exact replay from d3's initialised state with
  d3's numeric `alphaDecay`.
- **serde_json** must be built with `float_roundtrip`, otherwise fixture
  values parse one ULP off and every scenario fails at tick 1. (Found the
  hard way.)

## 5. The core owns no loop and no events

d3-timer and d3-dispatch are not ported. Rust exposes `tick(n)` and
`run(max)`; the JS facade in `js/cytoscape-forcefield/src/simulation.js`
provides `restart()`/`stop()`/`on('tick.ns')` on top, using
`requestAnimationFrame` when present and `setTimeout` otherwise.

## 6. Custom JS forces get views as arguments

The consumer installs its own JS force (`sim.force('dagDirection', fn)`).
While `tick` runs, the wasm `Simulation` is mutably borrowed, so a callback
that calls back into it (for instance to re-fetch a detached memory view)
makes wasm-bindgen throw. The Rust side therefore passes fresh
`positions/velocities/fixed` views to the callback each tick, and the node
proxies read those. Rule: never call a `Simulation` method from inside a
force.

## 7. Accessors are evaluated once, in JS, into arrays

d3 evaluates number-or-function accessors at `initialize`. The facade does
the same and ships the resulting `Float64Array`s across the boundary. That
keeps the wasm API flat and means the consumer's degree-divided
`linkStrength` function works unchanged (it sees link objects whose
`source`/`target` are the node proxies, exactly as with d3).

## 8. Do not stream positions over IPC

Unchanged from the brief: ~1 000 nodes at 60 Hz through a Tauri invoke
would cost more than it saves. The native target is for one-shot layouts
(`examples/layout.rs`), not for feeding a live view.

## 9. wasm-opt is off

`wasm-pack` shells out to `wasm-opt`; on this machine a `mise` shim without
a version broke the build, so `wasm-opt = false` is set in the crate
metadata. Turn it on (`["-O3"]`) when a working binaryen is on PATH; expect
a few percent.

## Not done

- The gating diagnostic (§0).
- Publishing: crates.io for `forcefield`, npm for `forcefield-wasm` and
  `cytoscape-forcefield`. Nothing has been published.
- Copyright holder in `LICENSE` is "forcefield contributors"; put a name.
- Trying the adapter inside the real consumer (nemo-graph): it exposes the
  same `layout.simulation` surface the consumer uses (`nodes()`, `force()`,
  `alphaTarget().restart()`, `on('tick.adaptive')`, `n.fx = null`), but that
  has been exercised headless in Node, not in the app.
