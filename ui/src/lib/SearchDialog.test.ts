// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { SearchHit } from '../bindings'
import { toasts } from './ipc.svelte'

const searchMods = vi.hoisted(() => vi.fn())
vi.mock('../bindings', () => ({
  commands: { searchMods },
  events: { taskUpdate: { listen: () => Promise.resolve(() => {}) } },
}))
vi.mock('@tauri-apps/plugin-opener', () => ({ revealItemInDir: vi.fn() }))

const { default: SearchDialog } = await import('./SearchDialog.svelte')

const hit = (mod: string, rel: string, line: number, text: string): SearchHit => ({
  mod_id: mod,
  mod_name: `Mod ${mod}`,
  path: `C:/mods/${mod}/${rel}`,
  rel,
  line,
  text,
})

afterEach(() => {
  cleanup()
  searchMods.mockReset()
})

async function search(text: string) {
  render(SearchDialog, { onclose: () => {} })
  await fireEvent.input(screen.getByLabelText('Search text'), { target: { value: text } })
  await fireEvent.click(screen.getByRole('button', { name: /^Search$/ }))
}

describe('SearchDialog', () => {
  it('sends the query with parsed extensions and groups hits by mod', async () => {
    searchMods.mockResolvedValue({
      status: 'ok',
      data: {
        hits: [
          hit('a', 'About/About.xml', 3, '<name>Harmony</name>'),
          hit('a', 'Defs/x.xml', 9, 'Harmony patch'),
          hit('b', 'Source/Main.cs', 1, 'using HarmonyLib;'),
        ],
        truncated: false,
        files_searched: 42,
        ms: 7,
      },
    })
    await search('harmony')

    await waitFor(() => expect(screen.getAllByRole('heading', { level: 3 })).toHaveLength(2))
    expect(searchMods).toHaveBeenCalledWith({
      text: 'harmony',
      regex: false,
      case_sensitive: false,
      extensions: ['xml', 'txt', 'json', 'cs'],
      file_names: false,
    })
    expect(screen.getByText('About/About.xml:3')).toBeTruthy()
    expect(screen.getByText(/3 matches in 2 mods · 42 files/)).toBeTruthy()
  })

  it('says so when nothing matches and flags truncated results', async () => {
    searchMods.mockResolvedValue({
      status: 'ok',
      data: { hits: [], truncated: false, files_searched: 5, ms: 1 },
    })
    await search('zzz')
    await waitFor(() => expect(screen.getByText('No matches.')).toBeTruthy())
  })

  it('shows the backend error for a bad pattern without crashing', async () => {
    searchMods.mockResolvedValue({
      status: 'error',
      error: { kind: 'error', message: 'Invalid pattern: unclosed group' },
    })
    await search('(')
    await waitFor(() => expect(toasts.some((t) => t.text.includes('Invalid pattern'))).toBe(true))
    // still usable afterwards
    await waitFor(() => expect(screen.getByRole('button', { name: /^Search$/ })).toBeTruthy())
  })
})
