// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import { ago } from './prefs.svelte'

describe('ago', () => {
  const now = 1_000_000_000
  it('formats hours and days with correct plurals', () => {
    expect(ago(now - 600, now)).toBe('less than an hour ago')
    expect(ago(now - 3600, now)).toBe('1 hour ago')
    expect(ago(now - 5 * 3600, now)).toBe('5 hours ago')
    expect(ago(now - 86400, now)).toBe('1 day ago')
    expect(ago(now - 3 * 86400 - 100, now)).toBe('3 days ago')
  })
  it('never goes negative for clock skew', () => {
    expect(ago(now + 5000, now)).toBe('less than an hour ago')
  })
})
