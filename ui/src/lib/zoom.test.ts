// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'

const setZoomMock = vi.hoisted(() => vi.fn().mockResolvedValue(undefined))
vi.mock('@tauri-apps/api/webview', () => ({ getCurrentWebview: () => ({ setZoom: setZoomMock }) }))

const { initZoom, setZoom, stepZoom, zoom, ZOOM_STEPS } = await import('./zoom.svelte')

beforeEach(async () => {
  localStorage.clear()
  setZoomMock.mockClear()
  await setZoom(1)
  setZoomMock.mockClear()
})

describe('zoom', () => {
  it('steps up and down through the levels and stops at the ends', async () => {
    await stepZoom(1)
    expect(zoom.level).toBe(1.1)
    await stepZoom(-1)
    await stepZoom(-1)
    expect(zoom.level).toBe(0.9)
    for (let i = 0; i < 20; i++) await stepZoom(-1)
    expect(zoom.level).toBe(ZOOM_STEPS[0])
    for (let i = 0; i < 20; i++) await stepZoom(1)
    expect(zoom.level).toBe(ZOOM_STEPS.at(-1))
  })

  it('resets to 100% and applies it to the webview', async () => {
    await setZoom(1.5)
    expect(setZoomMock).toHaveBeenLastCalledWith(1.5)
    await stepZoom(0)
    expect(zoom.level).toBe(1)
    expect(setZoomMock).toHaveBeenLastCalledWith(1)
  })

  it('persists the level and restores it, snapping odd stored values', async () => {
    await setZoom(1.25)
    expect(localStorage.getItem('rimsort-rs.zoom')).toBe('1.25')
    localStorage.setItem('rimsort-rs.zoom', '9')
    await initZoom()
    expect(zoom.level).toBe(2)
    localStorage.setItem('rimsort-rs.zoom', 'garbage')
    await initZoom()
    expect(zoom.level).toBe(1)
  })
})
