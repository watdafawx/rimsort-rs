import { beforeEach, describe, expect, it } from 'vitest'
import { i18n, LANGS, setLanguage, t } from './i18n.svelte'

beforeEach(() => setLanguage('en'))

describe('i18n', () => {
  it('returns the English source when nothing is translated', () => {
    expect(t('Refresh')).toBe('Refresh')
    expect(t('never seen before')).toBe('never seen before')
  })

  it('fills {placeholders} and leaves unknown ones visible', () => {
    expect(t('{n} of {m}', { n: 2, m: 5 })).toBe('2 of 5')
    expect(t('{n} of {m}', { n: 2 })).toBe('2 of {m}')
  })

  it('switches language live and falls back to English for untranslated keys', async () => {
    await setLanguage('es_ES')
    expect(i18n.lang).toBe('es_ES')
    expect(t('Save')).toBe('Guardar')
    expect(t('never seen before')).toBe('never seen before')
    await setLanguage('en')
    expect(t('Save')).toBe('Save')
  })

  it('unknown language codes fall back to English', async () => {
    await setLanguage('xx_XX')
    expect(i18n.lang).toBe('en')
  })

  it('every listed language has a dictionary (or is English)', async () => {
    for (const l of LANGS) {
      await setLanguage(l.code)
      expect(i18n.lang).toBe(l.code)
    }
  })
})
