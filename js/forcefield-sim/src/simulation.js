// A d3-force-shaped facade over the forcefield wasm simulation.
//
// Consumers written against d3-force use: simulation.nodes() → objects with
// live x/y/vx/vy/fx/fy; force(name, force) including custom JS forces;
// alpha*/velocityDecay getters and setters; restart()/stop(); on('tick.ns').
// This module provides all of that on top of the wasm `Simulation`, whose
// positions live in wasm linear memory and are read through Float64Array
// views without copying.
//
// The wasm module is injected (`new Simulation(wasm, nodes)`) rather than
// imported, so the consumer chooses how the .wasm is loaded (bundler,
// web, or node target of wasm-pack).

import { dispatch } from './dispatch.js'

const AXIS = { x: 0, y: 1 }
const FIELDS = new Set(['x', 'y', 'vx', 'vy', 'fx', 'fy'])

function lcg() {
  let s = 1
  return () => (s = (1664525 * s + 1013904223) % 4294967296) / 4294967296
}

const isFn = (v) => typeof v === 'function'
const constant = (v) => () => v

/** Evaluate a number-or-function accessor once per item, like d3 does at initialize. */
function perItem(accessor, items, fallback) {
  if (accessor == null) accessor = fallback
  const f = isFn(accessor) ? accessor : constant(+accessor)
  return Float64Array.from(items, (item, i) => {
    const v = +f(item, i, items)
    return Number.isNaN(v) ? NaN : v
  })
}

export class Simulation {
  constructor(wasm, nodes = []) {
    this._wasm = wasm
    this._core = null
    this._nodes = []
    this._proxies = []
    this._forces = new Map() // name → descriptor (built-in) | function (custom)
    this._event = dispatch('tick', 'end')
    this._random = lcg()
    this._running = false
    this._frame = null
    this._views = null
    this.nodes(nodes)
  }

  // --- memory views ----------------------------------------------------

  /** Views onto wasm memory; re-fetched when the buffer was detached by growth. */
  views() {
    const v = this._views
    if (v && v.pos.buffer.byteLength !== 0) return v
    return (this._views = { pos: this._core.positions(), vel: this._core.velocities(), fix: this._core.fixed() })
  }

  _proxy(i, data) {
    const sim = this
    const target = { ...data, index: i }
    for (const k of FIELDS) delete target[k]
    return new Proxy(target, {
      get(t, key) {
        if (typeof key === 'string' && FIELDS.has(key)) {
          const views = sim.views()
          if (key === 'x' || key === 'y') return views.pos[2 * i + AXIS[key]]
          if (key === 'vx' || key === 'vy') return views.vel[2 * i + AXIS[key[1]]]
          const f = views.fix[2 * i + AXIS[key[1]]]
          return Number.isNaN(f) ? undefined : f
        }
        return t[key]
      },
      set(t, key, value) {
        if (typeof key === 'string' && FIELDS.has(key)) {
          const views = sim.views()
          if (key === 'x' || key === 'y') views.pos[2 * i + AXIS[key]] = +value
          else if (key === 'vx' || key === 'vy') views.vel[2 * i + AXIS[key[1]]] = +value
          else views.fix[2 * i + AXIS[key[1]]] = value == null ? NaN : +value
          return true
        }
        t[key] = value
        return true
      },
      deleteProperty(t, key) {
        if (key === 'fx' || key === 'fy') { sim.views().fix[2 * i + AXIS[key[1]]] = NaN; return true }
        if (typeof key === 'string' && FIELDS.has(key)) return false
        return delete t[key]
      },
      has(t, key) {
        if (key === 'fx' || key === 'fy') return !Number.isNaN(sim.views().fix[2 * i + AXIS[key[1]]])
        return FIELDS.has(key) || key in t
      },
    })
  }

  // --- nodes -----------------------------------------------------------

  /**
   * Get the node proxies, or set the node list. Input objects may carry
   * x, y, vx, vy, fx, fy (unset → d3 semantics) plus any data; the returned
   * proxies expose the six simulation fields live from wasm memory and
   * everything else as ordinary properties.
   */
  nodes(_) {
    if (_ === undefined) return this._proxies
    this._nodes = _
    const n = _.length
    const pos = new Float64Array(2 * n)
    for (let i = 0; i < n; i++) {
      pos[2 * i] = _[i].x == null ? NaN : +_[i].x
      pos[2 * i + 1] = _[i].y == null ? NaN : +_[i].y
    }
    if (this._core) this._core.setNodes(pos)
    else this._core = this._wasm.Simulation.fromPositions(pos)
    this._views = null
    for (let i = 0; i < n; i++) {
      const d = _[i]
      if (d.vx != null || d.vy != null) this._core.setVelocity(i, +d.vx || 0, +d.vy || 0)
      if (d.fx != null || d.fy != null) this._core.setFixed(i, d.fx == null ? NaN : +d.fx, d.fy == null ? NaN : +d.fy)
    }
    this._proxies = _.map((d, i) => this._proxy(i, d))
    for (const name of this._forces.keys()) this._install(name)
    return this
  }

  // --- parameters ------------------------------------------------------

  alpha(_) { return _ === undefined ? this._core.alpha() : (this._core.setAlpha(+_), this) }
  alphaMin(_) { return _ === undefined ? this._core.alphaMin() : (this._core.setAlphaMin(+_), this) }
  alphaDecay(_) { return _ === undefined ? this._core.alphaDecay() : (this._core.setAlphaDecay(+_), this) }
  alphaTarget(_) { return _ === undefined ? this._core.alphaTarget() : (this._core.setAlphaTarget(+_), this) }
  velocityDecay(_) { return _ === undefined ? this._core.velocityDecay() : (this._core.setVelocityDecay(+_), this) }
  randomSource(_) { return _ === undefined ? this._random : (this._random = _, this) }

  // --- forces ----------------------------------------------------------

  /** d3 semantics: `force(name)` gets, `force(name, null)` removes, `force(name, f)` installs in place. */
  force(name, _) {
    if (arguments.length < 2) return this._forces.get(name)
    if (_ == null) {
      this._forces.delete(name)
      this._core.removeForce(name)
    } else {
      this._forces.set(name, _)
      this._install(name)
    }
    return this
  }

  _install(name) {
    const f = this._forces.get(name)
    const nodes = this._proxies
    if (isFn(f)) {
      // Custom JS force: initialize(nodes, random) then apply(alpha) per tick,
      // mutating the live proxies (i.e. wasm memory) directly.
      if (f.initialize) f.initialize(nodes, this._random)
      // Rust passes fresh views for the duration of the call; the proxies
      // read them through views(), so the force never re-enters wasm.
      this._core.forceCustom(name, (alpha, pos, vel, fix) => {
        this._views = { pos, vel, fix }
        f(alpha)
      })
      this._views = null
      return
    }
    f._attach(this, name)
    const c = this._core
    switch (f.type) {
      case 'manyBody':
        c.forceManyBody(name, perItem(f._strength, nodes, -30), f._theta, f._distanceMin, f._distanceMax)
        break
      case 'link': {
        const links = f._resolve(nodes)
        const src = Uint32Array.from(links, (l) => l.source.index)
        const tgt = Uint32Array.from(links, (l) => l.target.index)
        const distance = perItem(f._distance, links, 30)
        const strength = f._strength == null ? new Float64Array(0) : perItem(f._strength, links, 0)
        c.forceLink(name, src, tgt, distance, strength, f._iterations)
        break
      }
      case 'collide':
        c.forceCollide(name, perItem(f._radius, nodes, 1), f._strength, f._iterations)
        break
      case 'x':
        c.forceX(name, perItem(f._x, nodes, 0), perItem(f._strength, nodes, 0.1))
        break
      case 'y':
        c.forceY(name, perItem(f._y, nodes, 0), perItem(f._strength, nodes, 0.1))
        break
      case 'center':
        c.forceCenter(name, f._x, f._y, f._strength)
        break
      case 'radial':
        c.forceRadial(name, perItem(f._radius, nodes, 0), f._x, f._y, perItem(f._strength, nodes, 0.1))
        break
      default:
        throw new Error(`unknown force type: ${f.type}`)
    }
    this._views = null
  }

  // --- stepping --------------------------------------------------------

  tick(iterations = 1) {
    this._core.tick(iterations)
    return this
  }

  /** Run to alphaMin synchronously (bounded), without events. Returns ticks run. */
  run(maxTicks = 10_000) {
    return this._core.run(maxTicks)
  }

  restart() {
    if (this._running) return this
    this._running = true
    const schedule = typeof requestAnimationFrame === 'function'
      ? (fn) => { this._frame = requestAnimationFrame(fn) }
      : (fn) => { this._frame = setTimeout(fn, 16) }
    const step = () => {
      if (!this._running) return
      this._core.tick(1)
      this._event.call('tick', this)
      if (this._core.alpha() < this._core.alphaMin()) {
        this._running = false
        this._frame = null
        this._event.call('end', this)
        return
      }
      schedule(step)
    }
    schedule(step)
    return this
  }

  stop() {
    this._running = false
    if (this._frame != null) {
      if (typeof cancelAnimationFrame === 'function') cancelAnimationFrame(this._frame)
      clearTimeout(this._frame)
      this._frame = null
    }
    return this
  }

  on(typename, _) {
    if (arguments.length < 2) return this._event.on(typename)
    this._event.on(typename, _)
    return this
  }

  find(x, y, radius) {
    const i = this._core.find(x, y, radius)
    return i < 0 ? undefined : this._proxies[i]
  }

  /** Release wasm memory. The simulation is unusable afterwards. */
  free() {
    this.stop()
    this._core?.free()
    this._core = null
  }
}

// --- force descriptors: the d3 factory API, evaluated at install time ---

class Descriptor {
  constructor(type) { this.type = type; this._sim = null; this._name = null }
  _attach(sim, name) { this._sim = sim; this._name = name }
  _set(key, value) {
    this[key] = value
    if (this._sim) this._sim._install(this._name)
    return this
  }
  _accessor(key, args) { return args.length ? this._set(key, args[0]) : this[key] }
}

export function forceManyBody() {
  const f = new Descriptor('manyBody')
  Object.assign(f, { _strength: -30, _theta: 0.9, _distanceMin: 1, _distanceMax: Infinity })
  f.strength = function (...a) { return this._accessor('_strength', a) }
  f.theta = function (...a) { return this._accessor('_theta', a) }
  f.distanceMin = function (...a) { return this._accessor('_distanceMin', a) }
  f.distanceMax = function (...a) { return this._accessor('_distanceMax', a) }
  return f
}

export function forceLink(links = []) {
  const f = new Descriptor('link')
  Object.assign(f, { _links: links, _id: (d) => d.index, _distance: 30, _strength: null, _iterations: 1 })
  f.links = function (...a) { return this._accessor('_links', a) }
  f.id = function (...a) { return this._accessor('_id', a) }
  f.distance = function (...a) { return this._accessor('_distance', a) }
  f.strength = function (...a) { return this._accessor('_strength', a) }
  f.iterations = function (...a) { return this._accessor('_iterations', a) }
  /** Resolve ids to node proxies in place, as d3's link.initialize does. */
  f._resolve = function (nodes) {
    const byId = new Map(nodes.map((d, i) => [this._id(d, i, nodes), d]))
    const find = (id) => {
      const n = byId.get(id)
      if (!n) throw new Error(`node not found: ${id}`)
      return n
    }
    this._links.forEach((l, i) => {
      l.index = i
      if (typeof l.source !== 'object') l.source = find(l.source)
      else if (l.source.index == null || nodes[l.source.index] !== l.source) l.source = find(this._id(l.source, l.source.index, nodes))
      if (typeof l.target !== 'object') l.target = find(l.target)
      else if (l.target.index == null || nodes[l.target.index] !== l.target) l.target = find(this._id(l.target, l.target.index, nodes))
    })
    return this._links
  }
  return f
}

export function forceCollide(radius) {
  const f = new Descriptor('collide')
  Object.assign(f, { _radius: radius == null ? 1 : radius, _strength: 1, _iterations: 1 })
  f.radius = function (...a) { return this._accessor('_radius', a) }
  f.strength = function (...a) { return this._accessor('_strength', a) }
  f.iterations = function (...a) { return this._accessor('_iterations', a) }
  return f
}

export function forceX(x) {
  const f = new Descriptor('x')
  Object.assign(f, { _x: x == null ? 0 : x, _strength: 0.1 })
  f.x = function (...a) { return this._accessor('_x', a) }
  f.strength = function (...a) { return this._accessor('_strength', a) }
  return f
}

export function forceY(y) {
  const f = new Descriptor('y')
  Object.assign(f, { _y: y == null ? 0 : y, _strength: 0.1 })
  f.y = function (...a) { return this._accessor('_y', a) }
  f.strength = function (...a) { return this._accessor('_strength', a) }
  return f
}

export function forceCenter(x = 0, y = 0) {
  const f = new Descriptor('center')
  Object.assign(f, { _x: x, _y: y, _strength: 1 })
  f.x = function (...a) { return this._accessor('_x', a) }
  f.y = function (...a) { return this._accessor('_y', a) }
  f.strength = function (...a) { return this._accessor('_strength', a) }
  return f
}

export function forceRadial(radius, x = 0, y = 0) {
  const f = new Descriptor('radial')
  Object.assign(f, { _radius: radius, _x: x, _y: y, _strength: 0.1 })
  f.radius = function (...a) { return this._accessor('_radius', a) }
  f.x = function (...a) { return this._accessor('_x', a) }
  f.y = function (...a) { return this._accessor('_y', a) }
  f.strength = function (...a) { return this._accessor('_strength', a) }
  return f
}
