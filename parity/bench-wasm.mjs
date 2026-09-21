// The wasm side of the benchmark: same graph as bench.mjs, run through the
// Node build of forcefield-wasm. Reads the position view every tick, as a
// renderer would.
import { createRequire } from 'node:module'
const { Simulation } = createRequire(import.meta.url)('../crates/forcefield-wasm/pkg-node/forcefield.js')

function lcg() { let s = 1; return () => (s = (1664525 * s + 1013904223) % 4294967296) / 4294967296 }

function build(n) {
  const rnd = lcg()
  const pos = new Float64Array(2 * n)
  for (let i = 0; i < n; i++) { pos[2 * i] = Math.round(rnd() * 800); pos[2 * i + 1] = Math.round(rnd() * 600) }
  const src = [], tgt = []
  for (let i = 1; i < n; i++) { src.push(Math.floor(rnd() * i)); tgt.push(i) }
  for (let k = 0; k < Math.floor(n / 3); k++) {
    const a = Math.floor(rnd() * n), b = Math.floor(rnd() * n)
    if (a !== b) { src.push(a); tgt.push(b) }
  }
  const degree = new Array(n).fill(0)
  for (let i = 0; i < src.length; i++) { degree[src[i]]++; degree[tgt[i]]++ }
  const strength = src.map((s, i) => 0.3 / Math.max(1, Math.min(degree[s], degree[tgt[i]])))
  const sim = Simulation.fromPositions(pos)
  sim.setAlphaDecay(0.015); sim.setVelocityDecay(0.35)
  sim.forceCollide('collide', [25], 1, 2)
  sim.forceLink('link', Uint32Array.from(src), Uint32Array.from(tgt), [80], strength, 1)
  sim.forceManyBody('many-body', [-150], 0.9, 5, 250)
  sim.forceX('x', [0], [0.05]); sim.forceY('y', [0], [0.05])
  sim.forceCenter('center', 400, 300, 1)
  return sim
}

const ticks = 300
console.log('  nodes    ms/tick     total ms')
for (const n of [500, 1000, 2000, 5000, 10000]) {
  const sim = build(n)
  let checksum = 0
  const t = performance.now()
  for (let k = 0; k < ticks; k++) { sim.tick(1); const p = sim.positions(); checksum += p[0] }
  const ms = performance.now() - t
  console.log(`${String(n).padStart(7)} ${(ms / ticks).toFixed(3).padStart(10)} ${ms.toFixed(1).padStart(12)}`)
  if (!Number.isFinite(checksum)) throw new Error('non-finite')
}
