# cytoscape-forcefield

A [cytoscape.js](https://js.cytoscape.org) layout driven by
[forcefield](https://github.com/EduardoFernandezVillazon/forcefield), a Rust/wasm port of d3-force that is verified
tick-for-tick against the JS reference. Fork of `cytoscape-d3-force` 1.1.4:
same options, same `layout.simulation` surface, the simulation runs in wasm
and positions are read from wasm memory without copying.

```js
import cytoscape from 'cytoscape'
import init, * as wasm from 'forcefield-wasm'
import forcefield from 'cytoscape-forcefield'

await init()
forcefield(cytoscape, wasm)          // registers layout name 'forcefield'
cy.layout({ name: 'forcefield', animate: true, infinite: true, /* cytoscape-d3-force options */ }).run()
```

`layout.simulation` is d3-shaped: `nodes()` (live x/y/vx/vy/fx/fy),
`force(name, fn)` for custom JS forces, `alphaTarget(x).restart()`,
`on('tick.namespace', fn)`, `find(x, y)`. One rule: never call a simulation
method from inside a force callback.

Options, verbatim from cytoscape-d3-force, are in `src/defaults.js`.
Docs and the parity harness: https://github.com/EduardoFernandezVillazon/forcefield. MIT, as the original adapter.
