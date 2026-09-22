# forcefield

A hand port of [d3-force](https://github.com/d3/d3-force) and the Barnes-Hut
quadtree it depends on ([d3-quadtree](https://github.com/d3/d3-quadtree)) to
Rust, with a wasm build and a cytoscape.js adapter.

**The port is verified, not "about right".** d3-force is fully deterministic
(phyllotaxis initialisation, an LCG for the jiggle), so the parity harness in
`parity/` runs the real d3-force 2.1.1 on four scenarios, records every
node's position after every tick, and the Rust test replays each scenario and
demands **bit-identical `f64` positions on every tick**. All four pass:
920 ticks across 852 nodes, zero mismatches. The cytoscape adapter's test does
the same through the wasm boundary, custom JS force included.

## Layout

| path | what |
|---|---|
| `crates/forcefield` | core crate: pure `f64` math, no I/O, no timers. Quadtree, simulation, seven forces. |
| `crates/forcefield-wasm` | wasm-bindgen bindings. Positions are a zero-copy `Float64Array` view onto wasm memory. |
| `js/cytoscape-forcefield` | fork of `cytoscape-d3-force` 1.1.4 with a d3-shaped `Simulation` facade over the wasm build. |
| `parity/` | golden-frame generator (Node + d3-force 2.1.1) and the committed fixtures. |
| `docs/DECISIONS.md` | decisions taken and the diagnostic that gates whether this project helps you at all. |
| `third_party/` | upstream licence notices. |

## Which d3 line

d3-force **2.1.1** is the reference, because that is what the consumer this
was written for resolves. Its `src/` is byte-identical to d3-force **3.0.0**
(the 3.x release only dropped the UMD bundle and changed the licence text to
ISC), and d3-quadtree 2.0.0 is byte-identical to 3.0.1. So the port is
equally a port of 3.x.

## Rust

```rust
use forcefield::{Simulation, Node, forces::{ManyBody, Link, Collide, X, Y}};

let nodes = vec![Node::UNSET; 500];               // NaN = unset → phyllotaxis spiral
let mut sim = Simulation::new(&nodes);
sim.set_alpha_decay(0.015).set_velocity_decay(0.35);
sim.add_force("collide", Box::new(Collide::new(25.0).iterations(2)))
   .add_force("link", Box::new(Link::new(links).distance(80.0).strength(per_link_strengths)))
   .add_force("charge", Box::new(ManyBody::new().strength(-150.0).distance_min(5.0).distance_max(250.0)))
   .add_force("x", Box::new(X::new(0.0).strength(0.05)))
   .add_force("y", Box::new(Y::new(0.0).strength(0.05)));
let ticks = sim.run(2_000);                        // until alpha < alpha_min
let (x0, y0) = (sim.bodies().x(0), sim.bodies().y(0));
```

Accessors take a constant, a `Vec<f64>` (one per node/link) or a closure, and
are evaluated once at `initialize`, as in d3. Links are `(source, target)`
index pairs; resolving ids is the caller's job. `Node::fixed(x, y)` pins.
`examples/layout.rs` is a one-shot headless layout from JSON on stdin, the
native target's reason to exist.

## wasm

```js
import * as wasm from 'forcefield-wasm'   // wasm-pack output (bundler/web/nodejs target)
const sim = wasm.Simulation.fromPositions(Float64Array.of(0, 0, 10, 3, NaN, NaN))
sim.forceManyBody('charge', [-30], 0.9, 1, Infinity)
sim.forceLink('link', Uint32Array.of(0, 1), Uint32Array.of(1, 2), [30], [], 1)  // [] = d3 default strength
sim.tick(1)
const pos = sim.positions()   // Float64Array view: [x0, y0, x1, y1, …], no copy
```

Array parameters of length 1 are constants; otherwise one value per
node/link. `positions()`, `velocities()` and `fixed()` are views onto wasm
memory: re-fetch them after any call that might allocate (anything but
`tick`/`run`), because memory growth detaches the old buffer. A JS force is
`forceCustom(name, (alpha, pos, vel, fix) => …)`; it receives fresh views and
must not call the simulation from inside the callback.

Build with `wasm-pack build crates/forcefield-wasm --release --target bundler`
(or `web`, `nodejs`).

## cytoscape

```js
import cytoscape from 'cytoscape'
import * as wasm from 'forcefield-wasm'
import forcefield from 'cytoscape-forcefield'
forcefield(cytoscape, wasm)                                  // registers layout 'forcefield'
const layout = cy.layout({ name: 'forcefield', ...sameOptionsAsCytoscapeD3Force }).run()
layout.simulation                                            // d3-shaped: nodes(), force(), alphaTarget().restart(), on('tick.ns')
```

Options are `cytoscape-d3-force`'s, verbatim. `layout.simulation.nodes()`
returns proxies whose `x y vx vy fx fy` read and write wasm memory directly,
so code written against d3 (custom forces, adaptive cooling reading
positions, `n.fx = null`, `delete n.fx`) keeps working with no per-tick
copy. The wasm module is injected so you choose how the `.wasm` is loaded.

## Speed

Consumer-shaped configuration (collide ×2 iterations, link, many-body with a
distance cap, x, y, center), 300 ticks, one core, 2026-09-21 laptop, ms per
tick:

| nodes | d3-force (node 20) | forcefield wasm (node 20) | forcefield native |
|---:|---:|---:|---:|
| 500 | 2.2 | 1.4 | 0.7 |
| 1 000 | 5.2 | 2.5 | 1.6 |
| 2 000 | 12.8 | 6.1 | 3.7 |
| 5 000 | 39.8 | 18.7 | 12.1 |
| 10 000 | 94.1 | 53.7 | 29.3 |

`cargo run --release -p forcefield --example bench`, `node parity/bench.mjs`,
`node parity/bench-wasm.mjs`. Read `docs/DECISIONS.md` before concluding this
makes a live graph view faster: at 1 000 nodes the force math is a few
milliseconds of a frame, and the rest is the renderer.

## Installing the JS packages

They are not on the npm registry. Each GitHub release attaches both as npm
tarballs, which npm installs from a URL like any other dependency (verified
with Vite 8, no wasm plugin required):

```
npm install https://github.com/EduardoFernandezVillazon/forcefield/releases/download/v0.1.0/forcefield-wasm-0.1.0.tgz \
            https://github.com/EduardoFernandezVillazon/forcefield/releases/download/v0.1.0/cytoscape-forcefield-0.1.0.tgz
```

The Rust crate is `forcefield` on crates.io.

### Working from a local checkout

For hacking on this repo alongside a consumer:

```
wasm-pack build crates/forcefield-wasm --release --target web --out-dir pkg-web --out-name forcefield
cd <consumer>/ui
npm install ../../Projects/forcefield/js/cytoscape-forcefield ../../Projects/forcefield/crates/forcefield-wasm/pkg-web
```

npm records them as `file:` dependencies and symlinks them, so edits and
rebuilds in this repo show up in the consumer without reinstalling.

### Wiring it in

Where `cytoscape.use(d3Force)` was:

```js
import init, * as wasm from 'forcefield-wasm'
import forcefield from 'cytoscape-forcefield'
await init()                    // top-level await; or call it in the app bootstrap
forcefield(cytoscape, wasm)     // registers layout name 'forcefield'
```

and change the layout config's `name: 'd3-force'` to `'forcefield'`. The
`web` target is the one that needs no bundler plugin: its glue loads the
`.wasm` through `new URL(…, import.meta.url)`, which Vite turns into an
emitted asset.

For a Rust consumer (a Tauri backend doing one-shot layouts), a path
dependency: `forcefield = { path = "../../Projects/forcefield/crates/forcefield" }`.

## Developing

```
cargo test --workspace --exclude forcefield-wasm      # unit + parity tests
cd parity && npm ci && npm run gen                     # regenerate fixtures from d3-force (must not change them)
wasm-pack build crates/forcefield-wasm --release --target nodejs --out-dir pkg-node --out-name forcefield
cd js/cytoscape-forcefield && npm ci && npm test       # facade parity + headless cytoscape
```

## Licence

BSD-3-Clause. This is a derivative work of d3-force and d3-quadtree
(BSD-3-Clause, Mike Bostock) and cytoscape-d3-force (MIT, yangdf); notices are
in `third_party/`. The D3 project does not endorse it, which is why nothing
here is called "d3".
