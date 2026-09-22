// Messages between the main-thread client and the worker runtime.
//
// main → worker
//   { type: 'init', nodes, links, payload, buffers, keepPositions }
//       nodes: plain objects (x, y, vx, vy, fx, fy optional, plus data; `id` recommended)
//       links: plain objects with source/target ids (resolved by the consumer's setup)
//       payload: anything the consumer's setup(sim, payload) needs (config, pairs…)
//       buffers: number of frame buffers in flight (default 3)
//       keepPositions: when true, nodes whose id existed keep their position/velocity/pins
//   { type: 'setup', payload }            re-run the consumer's setup on the live simulation
//   { type: 'call', method, args }        call a Simulation facade method (alpha, alphaTarget, …)
//   { type: 'setFixed', node, x, y }      pin (NaN frees an axis)
//   { type: 'setPosition', node, x, y }
//   { type: 'run', running }              start/stop ticking
//   { type: 'step', iterations }          tick and send one frame even when stopped
//   { type: 'buffer', buf }               a drawn frame buffer, returned for reuse
// worker → main
//   { type: 'ready', n }
//   { type: 'frame', buf, tick, alpha, n }  buf: ArrayBuffer of Float32 [x0, y0, x1, y1, …]
//   { type: 'end', tick }                  alpha fell below alphaMin
//   { type: 'result', id, value }          reply to a 'call' that asked for one
//   { type: 'error', message }

export const PROTOCOL_VERSION = 1
