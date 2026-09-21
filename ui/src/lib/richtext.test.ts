// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import { richText } from './richtext'

/** Parse the result like the browser would, to assert on real elements rather than strings. */
function dom(text: string) {
  const d = document.createElement('div')
  d.innerHTML = richText(text)
  return d
}

describe('richText', () => {
  it('escapes HTML instead of executing it', () => {
    const d = dom('<script>alert(1)</script><img src=x onerror="alert(2)"> & more')
    expect(d.querySelector('script')).toBeNull()
    expect(d.querySelector('img')).toBeNull()
    expect(d.textContent).toContain('<script>alert(1)</script>')
    expect(d.textContent).toContain('& more')
  })

  it('renders BBCode formatting', () => {
    const d = dom('[b]Bold[/b] and [i]italic[/i]\n[h1]Title[/h1]\n[hr]')
    expect(d.querySelector('b')?.textContent).toBe('Bold')
    expect(d.querySelector('i')?.textContent).toBe('italic')
    expect(d.querySelector('strong.h1')?.textContent).toBe('Title')
    expect(d.querySelector('hr')).not.toBeNull()
  })

  it('renders lists without stray blank lines', () => {
    const d = dom('Intro\n[list]\n[*] one\n[*] two\n[/list]\nOutro')
    expect([...d.querySelectorAll('li')].map((l) => l.textContent?.trim())).toEqual(['one', 'two'])
    expect(richText('[list]\n[*] a\n[/list]')).toBe('<ul><li>a</ul>')
  })

  it('renders http(s) links and refuses other schemes', () => {
    const d = dom(
      '[url=https://example.com/a?x=1&y=2]site[/url] [url]https://b.example[/url] [url=javascript:alert(1)]bad[/url] [url=data:text/html,x]bad2[/url]',
    )
    const hrefs = [...d.querySelectorAll('a')].map((a) => a.getAttribute('href'))
    expect(hrefs).toEqual(['https://example.com/a?x=1&y=2', 'https://b.example'])
    expect(d.textContent).toContain('[url=javascript:alert(1)]bad')
  })

  it('cannot break out of an attribute', () => {
    const d = dom('[url=https://x.com/"onmouseover="alert(1)]hi[/url]')
    for (const a of d.querySelectorAll('a')) {
      expect(a.getAttributeNames()).toEqual(['href'])
    }
  })

  it('handles Unity rich text and only allows safe colors', () => {
    const d = dom(
      '<b>bold</b> <color=#ff0000>red</color> <color=expression(alert(1))>x</color> <size=20>big</size>',
    )
    expect(d.querySelector('b')?.textContent).toBe('bold')
    const spans = [...d.querySelectorAll('span')].map((s) => s.getAttribute('style'))
    expect(spans).toEqual(['color:#ff0000'])
    expect(d.textContent).toContain('big')
    expect(d.textContent).not.toContain('<size')
  })

  it('leaves plain text and unknown brackets alone', () => {
    expect(richText('Needs [Harmony] 1.5\r\nline two')).toBe('Needs [Harmony] 1.5\nline two')
  })

  it('shows images as a placeholder, never loads them, and does not nest links', () => {
    const d = dom('[url=https://discord.gg/x][img]https://example.com/a.png[/img][/url]')
    expect(d.querySelector('img')).toBeNull()
    expect(d.querySelectorAll('a')).toHaveLength(1)
    expect(d.querySelector('a .img')?.textContent).toBe('[image]')
  })

  it('links bare URLs once, keeping query strings', () => {
    const d = dom(
      'see https://a.example/p?x=1&y=2, or (https://b.example) and [url=https://c.example]c[/url]',
    )
    expect([...d.querySelectorAll('a')].map((a) => a.getAttribute('href'))).toEqual([
      'https://a.example/p?x=1&y=2',
      'https://b.example',
      'https://c.example',
    ])
  })

  it('flattens Steam tables', () => {
    const d = dom('[table][tr][td]one[/td] [td]two[/td][/tr][/table]')
    expect(d.textContent?.replace(/\s+/g, ' ')).toBe('one two')
  })
})
