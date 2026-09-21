/**
 * Mod descriptions mix plain text with Steam BBCode (`[b]`, `[url=…]`, `[list]`) and Unity rich text
 * (`<b>`, `<color=#f00>`). Everything is HTML-escaped first; only the tags below are turned back into
 * markup, so the result is safe for `{@html}`.
 */

const escapeHtml = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

const COLOR = String.raw`(#[0-9a-fA-F]{3,8}|[a-zA-Z]{3,20})`
const URL = String.raw`(https?:\/\/[^\]\s"'<>]+)`

/** [pattern, replacement] pairs applied in order to the escaped text (which contains `&lt;`, not `<`). */
const RULES: [RegExp, string][] = [
  // Unity rich text (escaped form)
  [/&lt;(\/?)(b|i|u)&gt;/gi, '<$1$2>'],
  [new RegExp(String.raw`&lt;color=${COLOR}&gt;`, 'gi'), '<span style="color:$1">'],
  [/&lt;\/color&gt;/gi, '</span>'],
  [/&lt;\/?size(=[^&]{0,8})?&gt;/gi, ''],
  [/&lt;br\s*\/?&gt;/gi, '\n'],
  // Steam BBCode
  [/\[(\/?)b\]/gi, '<$1b>'],
  [/\[(\/?)i\]/gi, '<$1i>'],
  [/\[(\/?)u\]/gi, '<$1u>'],
  [/\[(\/?)strike\]/gi, '<$1s>'],
  [/\[h([1-3])\]\s*/gi, '<strong class="h h$1">'],
  [/\s*\[\/h[1-3]\]\s*/gi, '</strong>\n'],
  [/\[hr\]\s*/gi, '<hr>'],
  [/\[quote(=[^\]]*)?\]\s*/gi, '<blockquote>'],
  [/\s*\[\/quote\]\s*/gi, '</blockquote>'],
  [/\[code\]/gi, '<code>'],
  [/\[\/code\]/gi, '</code>'],
  [/\[list\]\s*/gi, '<ul>'],
  [/\[olist\]\s*/gi, '<ol>'],
  [/\s*\[\/list\]\s*/gi, '</ul>'],
  [/\s*\[\/olist\]\s*/gi, '</ol>'],
  [/\s*\[\*\]\s*/g, '<li>'],
  [new RegExp(String.raw`\[color=${COLOR}\]`, 'gi'), '<span style="color:$1">'],
  [/\[\/color\]/gi, '</span>'],
  [new RegExp(String.raw`\[url=${URL}\]`, 'gi'), '<a href="$1">'],
  [/\[\/url\]/gi, '</a>'],
  [/\[img\]([^[]*)\[\/img\]/gi, '<span class="img">[image]</span>'],
  [/\s*\[\/?(table|tr|td|th)\]\s*/gi, ' '],
]

const BARE_LINK = /(?<=^|[\s(])(https?:\/\/(?:[^\s<>"')&]|&amp;)+)/g

/** Description text → sanitized HTML (newlines are kept; render with `white-space: pre-wrap`). */
export function richText(text: string): string {
  let out = escapeHtml(text.replace(/\r\n?/g, '\n'))
  // `[url]link[/url]` must be handled before `[/url]` closes a non-existent anchor.
  out = out.replace(new RegExp(String.raw`\[url\]${URL}\[\/url\]`, 'gi'), '<a href="$1">$1</a>')
  for (const [re, to] of RULES) out = out.replace(re, to)
  // Bare links, only after whitespace or `(` so attribute values and anchor text are left alone.
  out = out.replace(BARE_LINK, (m) => {
    const tail = /[.,;:!?]+$/.exec(m)?.[0] ?? '' // sentence punctuation isn't part of the link
    const url = m.slice(0, m.length - tail.length)
    return `<a href="${url}">${url}</a>${tail}`
  })
  return out.replace(/\n{3,}/g, '\n\n').trim()
}
