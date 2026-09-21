import { getCurrentWebview } from '@tauri-apps/api/webview'

const KEY = 'rimsort-rs.zoom'
export const ZOOM_STEPS = [0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2]

export const zoom = $state({ level: 1 })

/** Snap to the nearest step so odd stored values can't wedge the UI at an unusable size. */
const clamp = (z: number) =>
  ZOOM_STEPS.reduce((best, s) => (Math.abs(s - z) < Math.abs(best - z) ? s : best), 1)

export async function setZoom(level: number) {
  zoom.level = clamp(level)
  try {
    localStorage.setItem(KEY, String(zoom.level))
  } catch {
    /* storage unavailable */
  }
  try {
    await getCurrentWebview().setZoom(zoom.level)
  } catch {
    /* not running inside Tauri (tests, plain browser) */
  }
}

/** Step to the next larger/smaller zoom level; `0` resets. */
export function stepZoom(dir: -1 | 0 | 1) {
  if (dir === 0) return setZoom(1)
  const i = ZOOM_STEPS.indexOf(zoom.level)
  return setZoom(ZOOM_STEPS[Math.min(ZOOM_STEPS.length - 1, Math.max(0, i + dir))])
}

export function initZoom() {
  let saved = 1
  try {
    saved = Number(localStorage.getItem(KEY)) || 1
  } catch {
    /* ignore */
  }
  return setZoom(saved)
}
