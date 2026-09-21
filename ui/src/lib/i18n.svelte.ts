// Tiny i18n: English source strings are the keys; a language file maps them to translations.
// Missing keys fall back to English. Dictionaries are trimmed to the keys the UI uses
// (`node scripts/i18n.mjs`) and loaded lazily per language.

export const LANGS: { code: string; label: string }[] = [
  { code: 'en', label: 'English' },
  { code: 'de_DE', label: 'Deutsch' },
  { code: 'es_ES', label: 'Español' },
  { code: 'fr_FR', label: 'Français' },
  { code: 'ja_JP', label: '日本語' },
  { code: 'ko_KR', label: '한국어' },
  { code: 'pt_BR', label: 'Português (Brasil)' },
  { code: 'ru_RU', label: 'Русский' },
  { code: 'tr_TR', label: 'Türkçe' },
  { code: 'zh_CN', label: '简体中文' },
  { code: 'zh_TW', label: '繁體中文' },
]

const loaders = import.meta.glob<{ default: Record<string, string> }>('../locales/*.json')
const KEY = 'rimsort-rs.lang'

export const i18n = $state({ lang: 'en', dict: {} as Record<string, string> })

/** Translate an English source string; `{name}` placeholders are filled from `params`. */
export function t(text: string, params?: Record<string, string | number>): string {
  const s = i18n.dict[text] ?? text
  return params ? s.replace(/\{(\w+)\}/g, (_, k) => String(params[k] ?? `{${k}}`)) : s
}

export async function setLanguage(code: string) {
  const load: (() => Promise<{ default: Record<string, string> }>) | undefined =
    loaders[`../locales/${code}.json`]
  i18n.dict = code !== 'en' && load ? (await load()).default : {}
  i18n.lang = load !== undefined || code === 'en' ? code : 'en'
  try {
    localStorage.setItem(KEY, i18n.lang)
  } catch {
    /* storage unavailable */
  }
}

/** Saved choice, else the system language (matched by prefix), else English. */
export function initLanguage() {
  let saved: string | null = null
  try {
    saved = localStorage.getItem(KEY)
  } catch {
    /* ignore */
  }
  const sys = (typeof navigator !== 'undefined' ? navigator.language : 'en').replace('-', '_')
  const match =
    saved ??
    LANGS.find((l) => l.code === sys)?.code ??
    LANGS.find((l) => l.code.startsWith(sys.split('_')[0] + '_'))?.code ??
    'en'
  return setLanguage(match)
}
