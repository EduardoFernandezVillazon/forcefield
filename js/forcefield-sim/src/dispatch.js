// Minimal d3-dispatch: named events with "type.name" namespaces, so
// consumers written against d3 (`sim.on('tick.adaptive', fn)`,
// `sim.on('tick.adaptive', null)`) keep working.

export function dispatch(...types) {
  const listeners = new Map(types.map((t) => [t, []]))

  function parse(typename) {
    const i = typename.indexOf('.')
    const type = i < 0 ? typename : typename.slice(0, i)
    const name = i < 0 ? '' : typename.slice(i + 1)
    if (!listeners.has(type)) throw new Error(`unknown type: ${type}`)
    return { type, name }
  }

  return {
    on(typename, callback) {
      const { type, name } = parse(typename)
      const list = listeners.get(type)
      if (callback === undefined) return list.find((l) => l.name === name)?.value
      const idx = list.findIndex((l) => l.name === name)
      if (idx >= 0) list.splice(idx, 1)
      if (callback != null) list.push({ name, value: callback })
      return this
    },
    call(type, that, ...args) {
      for (const l of [...listeners.get(type)]) l.value.apply(that, args)
    },
  }
}
