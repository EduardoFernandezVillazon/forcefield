// Main-thread client for the worker runtime. Frames arrive as Float32Array
// views; the buffer is returned to the worker after the frame handler runs.
//
//   const client = new SimulationClient(new Worker(new URL('./sim.worker.js', import.meta.url), { type: 'module' }))
//   client.onFrame((pos, { tick, alpha }) => renderer.setPositions(pos))
//   await client.init({ nodes, links, payload: { config } })
//   client.setFixed(i, x, y); client.alphaTarget(0.3)

import { dispatch } from './dispatch.js'

function portOf(p) {
  if (typeof p.addEventListener === 'function' && !('on' in p)) {
    return { post: (m, t) => p.postMessage(m, t), on: (fn) => p.addEventListener('message', (e) => fn(e.data)) }
  }
  return { post: (m, t) => p.postMessage(m, t), on: (fn) => p.on('message', (m) => fn(m)) }
}

export class SimulationClient {
  constructor(worker, { buffers = 3 } = {}) {
    this._worker = worker
    this._io = portOf(worker)
    this._buffers = buffers
    this._events = dispatch('frame', 'ready', 'end', 'error')
    this._pending = new Map()
    this._callId = 0
    this._frameHandler = null
    this.tick = 0
    this.alpha = 1
    this.n = 0
    this._io.on((m) => this._onMessage(m))
  }

  _onMessage(m) {
    switch (m.type) {
      case 'frame': {
        this.tick = m.tick
        this.alpha = m.alpha
        this.n = m.n
        const pos = new Float32Array(m.buf)
        try {
          this._frameHandler?.(pos, { tick: m.tick, alpha: m.alpha })
          this._events.call('frame', this, pos, { tick: m.tick, alpha: m.alpha })
        } finally {
          this._io.post({ type: 'buffer', buf: m.buf }, [m.buf])
        }
        break
      }
      case 'ready': {
        this.n = m.n
        const r = this._pending.get('ready')
        this._pending.delete('ready')
        r?.(m.n)
        this._events.call('ready', this, m.n)
        break
      }
      case 'result': {
        const r = this._pending.get(m.id)
        this._pending.delete(m.id)
        r?.(m.value)
        break
      }
      case 'end':
        this._events.call('end', this, m.tick)
        break
      case 'error':
        this._events.call('error', this, m.message)
        break
    }
  }

  /** The one place frames go; return value ignored. Called before `on('frame')` listeners. */
  onFrame(fn) {
    this._frameHandler = fn
    return this
  }

  on(typename, fn) {
    if (arguments.length < 2) return this._events.on(typename)
    this._events.on(typename, fn)
    return this
  }

  /** Create (or replace) the simulation. Resolves when the worker is ready. */
  init({ nodes, links = [], payload, keepPositions = false, running = true }) {
    return new Promise((resolve) => {
      this._pending.set('ready', resolve)
      this._io.post({ type: 'init', nodes, links, payload, buffers: this._buffers, keepPositions, running })
    })
  }

  /** Re-run the worker's setup hook with a new payload (e.g. new DAG pairs, new force settings). */
  setup(payload) {
    this._io.post({ type: 'setup', payload })
  }

  /** Call a facade method and await its return value (for getters). */
  call(method, ...args) {
    const id = ++this._callId
    return new Promise((resolve) => {
      this._pending.set(id, resolve)
      this._io.post({ type: 'call', id, method, args })
    })
  }

  /** Fire-and-forget facade call. */
  send(method, ...args) {
    this._io.post({ type: 'call', method, args })
    return this
  }

  alphaTarget(v) { return this.send('alphaTarget', v) }
  alphaDecay(v) { return this.send('alphaDecay', v) }
  alphaMin(v) { return this.send('alphaMin', v) }
  restart() { return this.run(true) }
  stop() { return this.run(false) }
  run(running) { this._io.post({ type: 'run', running }); return this }
  setFixed(node, x, y) { this._io.post({ type: 'setFixed', node, x, y }); return this }
  setPosition(node, x, y) { this._io.post({ type: 'setPosition', node, x, y }); return this }
  /** Tick once (or `iterations`) and deliver a frame even when stopped. */
  step(iterations = 1) { this._io.post({ type: 'step', iterations }); return this }

  terminate() {
    this._worker.terminate?.()
  }
}
