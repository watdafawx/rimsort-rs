/** Per-viewer display preferences (localStorage; the app works fine if storage is unavailable). */
const KEY = 'rimsort-rs.prefs'

export const RECENT_CHOICES = [0, 1, 3, 7, 14, 30] as const

export const prefs = $state({
  /** Mark mods whose folder changed within this many days (0 = off). */
  recentDays: 7 as number,
  /** Steam / Ludeon / folder icons instead of text badges for the mod source. */
  sourceIcons: true,
  /** C# vs XML/content icon on each mod. */
  typeIcons: true,
  /** "New" marker on mods missing from the latest save (and "in save" on inactive ones). */
  saveMarks: true,
  /** Rescan by itself when mods or ModsConfig.xml change on disk. */
  autoRefresh: true,
  /** Show what Sort would change before applying it. */
  previewSort: false,
  /** Row height preset for the mod lists. */
  density: 'normal' as 'compact' | 'normal' | 'comfortable',
})

try {
  const saved = JSON.parse(localStorage.getItem(KEY) ?? '{}')
  if (RECENT_CHOICES.includes(saved.recentDays)) prefs.recentDays = saved.recentDays
  if (['compact', 'normal', 'comfortable'].includes(saved.density)) prefs.density = saved.density
  for (const k of [
    'sourceIcons',
    'typeIcons',
    'saveMarks',
    'autoRefresh',
    'previewSort',
  ] as const) {
    if (typeof saved[k] === 'boolean') prefs[k] = saved[k]
  }
} catch {
  /* corrupt or unavailable: keep defaults */
}

function persist() {
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs))
  } catch {
    /* ignore */
  }
}

export function setRecentDays(days: number) {
  prefs.recentDays = days
  persist()
}

export const ROW_HEIGHT = { compact: 24, normal: 28, comfortable: 34 } as const

export function setDensity(d: 'compact' | 'normal' | 'comfortable') {
  prefs.density = d
  persist()
}

export function setPref(
  key: 'sourceIcons' | 'typeIcons' | 'saveMarks' | 'autoRefresh' | 'previewSort',
  on: boolean,
) {
  prefs[key] = on
  persist()
}

/** "3 days ago"-style text for a unix-seconds timestamp. */
export function ago(unix: number, now = Date.now() / 1000): string {
  const h = Math.max(0, now - unix) / 3600
  if (h < 1) return 'less than an hour ago'
  if (h < 24) return `${Math.floor(h)} hour${Math.floor(h) === 1 ? '' : 's'} ago`
  const d = Math.floor(h / 24)
  return `${d} day${d === 1 ? '' : 's'} ago`
}
