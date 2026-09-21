/** Per-viewer display preferences (localStorage; the app works fine if storage is unavailable). */
const KEY = 'rimsort-rs.prefs'

export const RECENT_CHOICES = [0, 1, 3, 7, 14, 30] as const

export const prefs = $state({
  /** Mark mods whose folder changed within this many days (0 = off). */
  recentDays: 7 as number,
})

try {
  const saved = JSON.parse(localStorage.getItem(KEY) ?? '{}')
  if (RECENT_CHOICES.includes(saved.recentDays)) prefs.recentDays = saved.recentDays
} catch {
  /* corrupt or unavailable: keep defaults */
}

export function setRecentDays(days: number) {
  prefs.recentDays = days
  try {
    localStorage.setItem(KEY, JSON.stringify({ recentDays: days }))
  } catch {
    /* ignore */
  }
}

/** "3 days ago"-style text for a unix-seconds timestamp. */
export function ago(unix: number, now = Date.now() / 1000): string {
  const h = Math.max(0, now - unix) / 3600
  if (h < 1) return 'less than an hour ago'
  if (h < 24) return `${Math.floor(h)} hour${Math.floor(h) === 1 ? '' : 's'} ago`
  const d = Math.floor(h / 24)
  return `${d} day${d === 1 ? '' : 's'} ago`
}
