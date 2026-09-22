// cytoscape-forcefield: cytoscape.js layout driven by the forcefield wasm
// simulation. A fork of cytoscape-d3-force 1.1.4 (MIT) whose 338 lines of
// adapter stay in JS and call into wasm instead of d3.
//
//   import cytoscape from 'cytoscape'
//   import * as wasm from 'forcefield-wasm'     // wasm-pack output
//   import forcefield from 'cytoscape-forcefield'
//   forcefield(cytoscape, wasm)
//   cy.layout({ name: 'forcefield', ...options }).run()
//
// The wasm module is injected so the consumer chooses how the .wasm is
// loaded (bundler / web / node targets of wasm-pack all work).

import { createLayout } from './layout.js'

export { Simulation, forceManyBody, forceLink, forceCollide, forceX, forceY, forceCenter, forceRadial } from 'forcefield-sim'
export { createLayout }
export { default as defaults } from './defaults.js'

export default function register(cytoscape, wasm, name = 'forcefield') {
  if (!cytoscape) return
  if (!wasm || !wasm.Simulation) throw new Error('cytoscape-forcefield: pass the forcefield-wasm module as the second argument')
  cytoscape('layout', name, createLayout(wasm))
}
