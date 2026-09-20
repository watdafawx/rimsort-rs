import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ModRow } from '../bindings'

const ok = <T>(data: T) => Promise.resolve({ status: 'ok' as const, data })
const setActive = vi.fn((_ids: string[]) => ok(null))

vi.mock('../bindings', () => ({
  commands: {
    setActive: (ids: string[]) => setActive(ids),
    getValidation: () => ok({ mods: [], errors: 0, warnings: 0, missing_dependencies: 0 }),
  },
  events: { taskUpdate: { listen: () => Promise.resolve(() => {}) } },
}))

const { app, disable, enable, moveActive, redo, setInactiveSort, undo } =
  await import('./store.svelte')

const row = (id: string, over: Partial<ModRow> = {}): ModRow => ({
  id,
  name: id,
  authors: '',
  package_id: `pkg.${id}`,
  mod_type: 'Local',
  valid: true,
  unsupported_version: false,
  published_file_id: null,
  modified: 0,
  color: null,
  tags: [],
  has_note: false,
  ...over,
})
const ids = (rows: ModRow[]) => rows.map((r) => r.id)

beforeEach(() => {
  app.active = [row('a'), row('b'), row('c')]
  app.inactive = [row('d'), row('e'), row('f', { valid: false })]
  app.dirty = false
  setActive.mockClear()
  // history is module-level: drain it
  while (app.undoDepth) undo()
  while (app.redoDepth) redo()
  app.active = [row('a'), row('b'), row('c')]
  app.inactive = [row('d'), row('e'), row('f', { valid: false })]
  setActive.mockClear()
})

describe('list edits', () => {
  it('enable inserts before the target and pushes the new order to the core', async () => {
    await enable(['d', 'e'], 'b')
    expect(ids(app.active)).toEqual(['a', 'd', 'e', 'b', 'c'])
    expect(ids(app.inactive)).toEqual(['f'])
    expect(setActive).toHaveBeenLastCalledWith(['a', 'd', 'e', 'b', 'c'])
    expect(app.dirty).toBe(true)
  })

  it('enable appends when no target and refuses invalid mods', async () => {
    await enable(['d', 'f'])
    expect(ids(app.active)).toEqual(['a', 'b', 'c', 'd'])
    expect(ids(app.inactive)).toEqual(['e', 'f']) // invalid stays inactive
  })

  it('disable returns rows to the inactive list, sorted by name', async () => {
    disable(['b'])
    expect(ids(app.active)).toEqual(['a', 'c'])
    expect(ids(app.inactive)).toEqual(['b', 'd', 'e', 'f'])
  })

  it('moveActive keeps the moved rows in their relative order', () => {
    moveActive(['a', 'b'], null)
    expect(ids(app.active)).toEqual(['c', 'a', 'b'])
    moveActive(['b'], 'c')
    expect(ids(app.active)).toEqual(['b', 'c', 'a'])
  })
})

describe('undo / redo', () => {
  it('restores both lists and re-syncs the core', async () => {
    await enable(['d'])
    disable(['a'])
    expect(ids(app.active)).toEqual(['b', 'c', 'd'])
    undo()
    expect(ids(app.active)).toEqual(['a', 'b', 'c', 'd'])
    undo()
    expect(ids(app.active)).toEqual(['a', 'b', 'c'])
    expect(ids(app.inactive)).toEqual(['d', 'e', 'f'])
    expect(setActive).toHaveBeenLastCalledWith(['a', 'b', 'c'])
    redo()
    expect(ids(app.active)).toEqual(['a', 'b', 'c', 'd'])
  })

  it('a new edit clears the redo stack', async () => {
    disable(['a'])
    undo()
    expect(app.redoDepth).toBe(1)
    disable(['b'])
    expect(app.redoDepth).toBe(0)
  })
})

describe('inactive sorting', () => {
  it('sorts by modified time, ascending and descending, with name as tiebreak', () => {
    app.inactive = [row('x', { modified: 5 }), row('y', { modified: 9 }), row('z', { modified: 5 })]
    setInactiveSort('modified', false)
    expect(ids(app.inactive)).toEqual(['x', 'z', 'y'])
    setInactiveSort('modified', true)
    expect(ids(app.inactive)).toEqual(['y', 'x', 'z'])
    setInactiveSort('name', false)
  })
})
