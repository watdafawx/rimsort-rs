// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { ModRow, Warning } from '../bindings'

vi.mock('../bindings', () => ({
  commands: {},
  events: { taskUpdate: { listen: () => Promise.resolve(() => {}) } },
}))
// jsdom has no layout engine: stub what `bind:clientHeight` needs.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
}

const { default: ModList } = await import('./ModList.svelte')

const row = (id: string, over: Partial<ModRow> = {}): ModRow => ({
  id,
  name: `Mod ${id}`,
  authors: 'someone',
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

const ROWS = ['a', 'b', 'c', 'd'].map((i) => row(i))

function setup(over: Record<string, unknown> = {}) {
  const props = {
    title: 'Active',
    listId: 'active' as const,
    rows: ROWS,
    onmove: vi.fn(),
    onactivate: vi.fn(),
    onselect: vi.fn(),
    ...over,
  }
  render(ModList, props)
  return props
}
const rows = () => screen.getByRole('listbox').querySelectorAll<HTMLElement>('.row')
const names = () => [...rows()].map((o) => o.querySelector('.name')!.textContent)
const selected = () =>
  [...rows()]
    .filter((o) => o.classList.contains('selected'))
    .map((o) => o.querySelector('.name')!.textContent)
const opt = (name: string) => screen.getByText(name).closest('[role=option]')!

afterEach(cleanup)

describe('ModList selection', () => {
  it('click selects one row and reports it', async () => {
    const p = setup()
    await fireEvent.click(opt('Mod b'))
    expect(selected()).toEqual(['Mod b'])
    expect(p.onselect).toHaveBeenCalledWith(expect.objectContaining({ id: 'b' }))
    await fireEvent.click(opt('Mod c'))
    expect(selected()).toEqual(['Mod c'])
  })

  it('ctrl-click toggles, shift-click selects a range', async () => {
    setup()
    await fireEvent.click(opt('Mod a'))
    await fireEvent.click(opt('Mod c'), { ctrlKey: true })
    expect(selected()).toEqual(['Mod a', 'Mod c'])
    await fireEvent.click(opt('Mod c'), { ctrlKey: true })
    expect(selected()).toEqual(['Mod a'])
    await fireEvent.click(opt('Mod a'))
    await fireEvent.click(opt('Mod d'), { shiftKey: true })
    expect(selected()).toEqual(['Mod a', 'Mod b', 'Mod c', 'Mod d'])
    // the anchor stays put for the next shift-click
    await fireEvent.click(opt('Mod b'), { shiftKey: true })
    expect(selected()).toEqual(['Mod a', 'Mod b'])
  })

  it('Enter / Delete / double-click activate the selection in list order', async () => {
    const p = setup()
    await fireEvent.click(opt('Mod b'))
    await fireEvent.click(opt('Mod d'), { ctrlKey: true })
    const box = screen.getByRole('listbox')
    await fireEvent.keyDown(box, { key: 'Enter' })
    expect(p.onactivate).toHaveBeenLastCalledWith(['b', 'd'])
    await fireEvent.keyDown(box, { key: 'Delete' })
    expect(p.onactivate).toHaveBeenCalledTimes(2)
    await fireEvent.dblClick(opt('Mod b'))
    expect(p.onactivate).toHaveBeenCalledTimes(3)
  })

  it('Ctrl+A selects everything shown; arrows move the cursor', async () => {
    setup()
    const box = screen.getByRole('listbox')
    await fireEvent.keyDown(box, { key: 'a', ctrlKey: true })
    expect(selected()).toHaveLength(4)
    await fireEvent.keyDown(box, { key: 'ArrowDown' })
    expect(selected()).toEqual(['Mod a'])
    await fireEvent.keyDown(box, { key: 'ArrowDown' })
    expect(selected()).toEqual(['Mod b'])
    await fireEvent.keyDown(box, { key: 'ArrowUp' })
    expect(selected()).toEqual(['Mod a'])
  })
})

describe('ModList filtering', () => {
  it('search matches name, author, package id and tags', async () => {
    setup({
      rows: [
        row('a', { name: 'Alpha' }),
        row('b', { name: 'Beta', authors: 'Zed' }),
        row('c', { name: 'Gamma', tags: ['combat'] }),
      ],
    })
    const search = screen.getByPlaceholderText(/Search/)
    await fireEvent.input(search, { target: { value: 'zed' } })
    expect(names()).toEqual(['Beta'])
    await fireEvent.input(search, { target: { value: 'combat' } })
    expect(names()).toEqual(['Gamma'])
    await fireEvent.input(search, { target: { value: 'pkg.a' } })
    expect(names()).toEqual(['Alpha'])
    await fireEvent.input(search, { target: { value: '' } })
    expect(names()).toHaveLength(3)
  })

  it('the warnings toggle shows only mods with warnings or version issues', async () => {
    const w: Warning = { kind: 'LoadAfter', other: 'x', other_name: 'X' }
    setup({
      rows: [row('a'), row('b', { unsupported_version: true }), row('c')],
      warnings: { c: [w] },
    })
    await fireEvent.click(screen.getByTitle('Show only mods with warnings').querySelector('input')!)
    expect(names().sort()).toEqual(['Mod b', 'Mod c'])
  })

  it('shows a colored stripe and dims invalid mods', () => {
    setup({ rows: [row('a', { color: '#ff0000' }), row('b', { valid: false })] })
    expect((opt('Mod a') as HTMLElement).style.boxShadow).toContain('#ff0000')
    expect(opt('Mod b').classList.contains('invalid')).toBe(true)
  })
})
