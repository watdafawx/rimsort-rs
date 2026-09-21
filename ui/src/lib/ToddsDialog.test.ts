// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { ToddsOptions } from '../bindings'

const h = vi.hoisted(() => ({
  getToddsOptions: vi.fn(),
  setToddsOptions: vi.fn(),
  runTodds: vi.fn(),
}))
vi.mock('../bindings', () => ({
  commands: { getToddsOptions: h.getToddsOptions, setToddsOptions: h.setToddsOptions },
  events: { taskUpdate: { listen: () => Promise.resolve(() => {}) } },
}))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))
vi.mock('./store.svelte', () => ({
  app: { active: [1, 2, 3], jobTask: 0 },
  runTodds: h.runTodds,
}))

const { default: ToddsDialog } = await import('./ToddsDialog.svelte')

const OPTS: ToddsOptions = {
  preset: 'Optimized',
  dry_run: false,
  overwrite: false,
  custom_command: '',
  active_mods_target: true,
  auto_before_launch: false,
}

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

async function open(onclose = () => {}) {
  h.getToddsOptions.mockResolvedValue({ status: 'ok', data: { ...OPTS } })
  h.setToddsOptions.mockResolvedValue({ status: 'ok', data: null })
  render(ToddsDialog, { onclose })
  await screen.findByText(/Active mods \(3\)/)
}

describe('ToddsDialog', () => {
  it('saves the chosen options, closes, then runs the optimizer', async () => {
    const onclose = vi.fn()
    await open(onclose)
    await fireEvent.click(screen.getByLabelText(/Re-encode textures/))
    await fireEvent.click(screen.getByLabelText(/Every mod in the local/))
    await fireEvent.click(screen.getByRole('button', { name: 'Optimize' }))

    await waitFor(() => expect(h.runTodds).toHaveBeenCalled())
    const sent = { ...OPTS, overwrite: true, active_mods_target: false }
    expect(h.setToddsOptions).toHaveBeenCalledWith(sent)
    expect(h.runTodds).toHaveBeenCalledWith(sent, 'Textures optimized')
    expect(onclose).toHaveBeenCalled()
  })

  it('a dry run relabels the button and the finish message', async () => {
    await open()
    await fireEvent.click(screen.getByLabelText(/Dry run/))
    await fireEvent.click(screen.getByRole('button', { name: 'Dry run' }))
    await waitFor(() => expect(h.runTodds).toHaveBeenCalled())
    expect(h.runTodds.mock.calls[0][1]).toBe('todds dry run finished')
  })

  it('clean asks first and does not persist the Clean preset', async () => {
    await open()
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false)
    await fireEvent.click(screen.getByRole('button', { name: /Delete generated/ }))
    expect(confirmSpy).toHaveBeenCalled()
    expect(h.runTodds).not.toHaveBeenCalled()

    confirmSpy.mockReturnValue(true)
    await fireEvent.click(screen.getByRole('button', { name: /Delete generated/ }))
    await waitFor(() => expect(h.runTodds).toHaveBeenCalled())
    expect(h.runTodds.mock.calls[0][0].preset).toBe('Clean')
    expect(h.setToddsOptions).not.toHaveBeenCalled()
  })
})
