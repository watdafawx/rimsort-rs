// Convert RimSort's Qt Linguist locales (reference-python/locales/*.ts) into flat JSON dictionaries keyed by the
// English source string: ui/src/locales/<lang>.json. Usage: node scripts/ts_to_json.mjs
import fs from 'node:fs'
import path from 'node:path'

const src = path.resolve('reference-python/locales')
const out = path.resolve('debug/locales-full')
fs.mkdirSync(out, { recursive: true })
const unescape = (s) =>
  s.replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&amp;/g, '&')
// Qt mnemonics ("&Save") and {placeholders} handling: strip the mnemonic marker only.
const clean = (s) => unescape(s).replace(/&(?=\w)/g, '').trim()

for (const f of fs.readdirSync(src).filter((f) => f.endsWith('.ts') && !f.startsWith('en_'))) {
  const xml = fs.readFileSync(path.join(src, f), 'utf8')
  const dict = {}
  for (const m of xml.matchAll(/<message>[\s\S]*?<source>([\s\S]*?)<\/source>[\s\S]*?<translation([^>]*)>([\s\S]*?)<\/translation>/g)) {
    const [, s, attrs, t] = m
    if (/type="(unfinished|obsolete|vanished)"/.test(attrs)) continue
    const key = clean(s), val = clean(t)
    if (key && val && key !== val && !(key in dict)) dict[key] = val
  }
  const lang = f.replace('.ts', '')
  fs.writeFileSync(path.join(out, `${lang}.json`), JSON.stringify(dict, null, 1))
  console.log(lang, Object.keys(dict).length)
}
