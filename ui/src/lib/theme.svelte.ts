export type ThemeMode = 'auto' | 'dark' | 'light'
const KEY = 'rimsort-rs.theme'

export const theme = $state<{ mode: ThemeMode }>({ mode: 'auto' })

function apply() {
  const root = document.documentElement
  if (theme.mode === 'auto') root.removeAttribute('data-theme')
  else root.dataset.theme = theme.mode
}

export function setTheme(mode: ThemeMode) {
  theme.mode = mode
  apply()
  try {
    localStorage.setItem(KEY, mode)
  } catch {
    /* storage unavailable */
  }
}

export function initTheme() {
  let saved: string | null = null
  try {
    saved = localStorage.getItem(KEY)
  } catch {
    /* ignore */
  }
  setTheme(saved === 'dark' || saved === 'light' ? saved : 'auto')
}
