// ContinuousLayout, forked from cytoscape-d3-force 1.1.4 src/d3-force.js
// (MIT, Copyright 2019-2020 yangdf). Behaviour is kept line for line except:
//
//   * `d3.forceSimulation(nodes)` → `new Simulation(wasm, nodes)`; the
//     force factories come from ./simulation.js and have the same API.
//   * The simulation hands back node proxies whose x/y/vx/vy/fx/fy live in
//     wasm memory; they replace the plain scratch objects, so the rest of
//     the adapter (and consumers reading `layout.simulation.nodes()`) sees
//     live coordinates without any per-tick copy.
//   * `Object.assign` is used directly instead of the polyfill.

import defaults from './defaults.js'
import { Simulation, forceCollide, forceLink, forceManyBody, forceX, forceY, forceRadial, forceCenter } from 'forcefield-sim'

const assign = Object.assign

export function createLayout(wasm) {
  class ContinuousLayout {
    constructor(options) {
      const o = (this.options = assign({}, defaults, options))
      this.state = assign({}, o, {
        layout: this,
        nodes: o.eles.nodes(),
        edges: o.eles.edges(),
        progress: 0,
        iterations: 0,
        startTime: 0,
      })
      this.simulation = null
      this.removeCytoscapeEvents = null
      this.destroyedEvent = null
    }

    makeBoundingBox(bb, cy) {
      if (bb == null) {
        bb = { x1: 0, y1: 0, w: cy.width(), h: cy.height() }
      } else {
        bb = { x1: bb.x1, x2: bb.x2, y1: bb.y1, y2: bb.y2, w: bb.w, h: bb.h }
      }
      if (bb.x2 == null) bb.x2 = bb.x1 + bb.w
      if (bb.w == null) bb.w = bb.x2 - bb.x1
      if (bb.y2 == null) bb.y2 = bb.y1 + bb.h
      if (bb.h == null) bb.h = bb.y2 - bb.y1
      return bb
    }

    setInitialPositionState(node, state) {
      const p = node.position()
      const bb = state.currentBoundingBox
      let scratch = node.scratch(state.name)
      if (scratch == null) {
        scratch = {}
        node.scratch(state.name, scratch)
      }
      assign(scratch, state.randomize
        ? { x: bb.x1 + Math.round(Math.random() * bb.w), y: bb.y1 + Math.round(Math.random() * bb.h) }
        : { x: p.x, y: p.y })
      if (node.locked()) {
        assign(scratch, { fx: p.x, fy: p.y })
      }
    }

    refreshPositions(nodes, state, fit) {
      nodes.positions((node) => {
        const scratch = node.scratch(state.name)
        return { x: scratch.x, y: scratch.y }
      })
      fit && state.cy.fit(state.padding)
    }

    getScratch(el) {
      const name = this.state.name
      let scratch = el.scratch(name)
      if (!scratch) {
        scratch = {}
        el.scratch(name, scratch)
      }
      return scratch
    }

    ungrabify(nodes) {
      if (!this.state.ungrabifyWhileSimulating) return
      nodes.filter((node) => {
        const nodeGrabbable = (this.getScratch(node).grabbable = node.grabbable())
        return nodeGrabbable
      })
      nodes.ungrabify()
    }

    regrabify(nodes) {
      if (!this.state.ungrabifyWhileSimulating) return
      nodes.filter((node) => this.getScratch(node).grabbable)
      nodes.grabify()
    }

    tick() {
      const s = this.state
      s.progress += 1 / Math.ceil(Math.log(this.simulation.alphaMin()) / Math.log(1 - this.simulation.alphaDecay()))
      s.iterations++
      const _iterations = s.maxIterations && !s.infinite ? s.iterations / s.maxIterations : 0
      const _timeRunning = Date.now() - s.startTime
      const _timeIterations = s.maxSimulationTime && !s.infinite ? _timeRunning / s.maxSimulationTime : 0
      let _progress = Math.max(_iterations, _timeIterations, s.progress)
      _progress = _progress > 1 ? 1 : _progress
      if (_progress >= 1) {
        this.end(!s.infinite)
        return
      }
      s.tick && s.tick(_progress)
      if (s.animate) {
        this.refreshPositions(s.nodes, s, s.fit)
      }
    }

    end(destroyed) {
      const s = this.state
      this.refreshPositions(s.nodes, s, s.fit)
      this.emit('layoutstop', s.cy)
      this.reset(destroyed)
    }

    reset(destroyed) {
      this.simulation && this.simulation.stop()
      const s = this.state
      this.destroyedEvent && this.destroyedEvent()
      ;(destroyed || !s.infinite) && this.removeCytoscapeEvents && this.removeCytoscapeEvents()
      s.animate && this.regrabify(s.nodes)
      return this
    }

    run() {
      this.reset()
      const l = this
      const s = this.state
      let ready = false
      s.currentBoundingBox = this.makeBoundingBox(s.boundingBox, s.cy)
      if (s.ready) l.one('layoutready', s.ready)
      if (s.stop) l.one('layoutstop', s.stop)
      s.nodes.forEach((n) => this.setInitialPositionState(n, s))
      if (!ready) {
        ready = true
        l.emit('layoutready')
      }

      if (!l.simulation) {
        const _forcenodes = s.nodes.map((n) => assign(l.getScratch(n), n.data()))
        const _forceedges = s.edges.map((e) => assign({}, e.data()))
        l.simulation = new Simulation(wasm, _forcenodes)
        // The simulation's proxies become the scratch objects: their x/y/fx/fy
        // read and write wasm memory directly.
        l.simulation.nodes().forEach((proxy, i) => s.nodes[i].scratch(s.name, proxy))
        s.alpha && l.simulation.alpha(s.alpha)
        s.alphaMin && l.simulation.alphaMin(s.alphaMin)
        s.alphaDecay && l.simulation.alphaDecay(s.alphaDecay)
        s.alphaTarget && l.simulation.alphaTarget(s.alphaTarget)
        s.velocityDecay && l.simulation.velocityDecay(s.velocityDecay)
        const _collide = forceCollide()
        s.collideRadius && _collide.radius(s.collideRadius)
        s.collideStrength && _collide.strength(s.collideStrength)
        s.collideIterations && _collide.iterations(s.collideIterations)
        const _link = forceLink(_forceedges)
        s.linkId && _link.id(s.linkId)
        s.linkDistance && _link.distance(s.linkDistance)
        s.linkStrength && _link.strength(s.linkStrength)
        s.linkIterations && _link.iterations(s.linkIterations)
        const _manyBody = forceManyBody()
        s.manyBodyStrength && _manyBody.strength(s.manyBodyStrength)
        s.manyBodyTheta && _manyBody.theta(s.manyBodyTheta)
        s.manyBodyDistanceMin && _manyBody.distanceMin(s.manyBodyDistanceMin)
        s.manyBodyDistanceMax && _manyBody.distanceMax(s.manyBodyDistanceMax)
        const _x = forceX()
        s.xX && _x.x(s.xX)
        s.xStrength && _x.strength(s.xStrength)
        const _y = forceY()
        s.yY && _y.y(s.yY)
        s.yStrength && _y.strength(s.yStrength)
        let _radius = null
        if (s.radialRadius || s.radialStrength || s.radialX || s.radialY) {
          _radius = forceRadial()
          s.radialRadius && _radius.radius(s.radialRadius)
          s.radialStrength && _radius.strength(s.radialStrength)
          s.radialX && _radius.x(s.radialX)
          s.radialY && _radius.y(s.radialY)
        }
        const _center = forceCenter(s.currentBoundingBox.w / 2, s.currentBoundingBox.h / 2)
        l.simulation
          .force('collide', _collide)
          .force('link', _link)
          .force('many-body', _manyBody)
          .force('x', _x)
          .force('y', _y)
          .force('center', _center)
        _radius && l.simulation.force('radius', _radius)
        l.simulation
          .on('tick', () => { l.tick() })
          .on('end', () => { l.end() })
      }
      if (!l.destroyedEvent) {
        const destroyHandler = function () { l.stop() }
        s.cy.one('destroy', destroyHandler)
        l.destroyedEvent = function () {
          s.cy.off('destroy', destroyHandler)
          l.destroyedEvent = null
        }
      }
      l.prerun(s)
      l.emit('layoutstart')
      s.progress = 0
      s.iterations = 0
      s.startTime = Date.now()

      if (s.animate) {
        const restartAlphaTarget = Math.abs((s.alpha || 1) - (s.alphaTarget || 0)) / 3
        if (!l.removeCytoscapeEvents) {
          const _cytoscapeEvent = function (e) {
            const node = this
            const pos = node.position()
            const nodeIsTarget = e.cyTarget === node || e.target === node
            if (!nodeIsTarget) return
            const _scratch = l.getScratch(node)
            s.progress = 0
            s.iterations = 0
            s.startTime = Date.now()
            _scratch.x = pos.x
            _scratch.y = pos.y
            if (e.type === 'grab') {
              l.simulation.alphaTarget(restartAlphaTarget).restart()
            } else if (e.type === 'unlock' || e.type === 'free') {
              if (!s.fixedAfterDragging) {
                delete _scratch.fx
                delete _scratch.fy
              } else {
                _scratch.fx = pos.x
                _scratch.fy = pos.y
              }
              l.simulation.alphaTarget(restartAlphaTarget).restart()
            } else {
              _scratch.fx = pos.x
              _scratch.fy = pos.y
            }
          }
          l.removeCytoscapeEvents = function () {
            s.nodes.off('grab free drag lock unlock', _cytoscapeEvent)
            l.removeCytoscapeEvents = null
          }
          s.nodes.on('grab free drag lock unlock', _cytoscapeEvent)
        }
        l.ungrabify(s.nodes)
      }
      l.postrun(s)
      // The original relied on d3's timer, which starts on forceSimulation();
      // here the loop is ours, so start it explicitly.
      l.simulation.restart()
      return this
    }

    prerun() {}
    postrun() {}

    stop() {
      return this.reset(true)
    }
  }

  // cytoscape instantiates layouts with `registrant.call(this, options)`, so
  // hand it a plain constructor function that builds the class instance
  // and adopts its state; methods come from the shared prototype chain.
  function Layout(options) {
    Object.assign(this, new ContinuousLayout(options))
    this.state.layout = this
  }
  Layout.prototype = ContinuousLayout.prototype
  return Layout
}
