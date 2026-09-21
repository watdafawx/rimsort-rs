// Build trimmed translation dictionaries and report gaps.
//   node scripts/i18n.mjs            regenerate ui/src/locales/*.json (from RimSort's .ts files) with only the keys
//                                    the UI uses via t('…'), and print per-language coverage
//   node scripts/i18n.mjs --check    fail if a used key is missing from the key list (keys.json is committed)
import fs from 'node:fs'
import path from 'node:path'

const uiSrc = path.resolve('ui/src')
const walk = (d) => fs.readdirSync(d, { withFileTypes: true }).flatMap((e) => e.isDirectory() ? (e.name === 'locales' ? [] : walk(path.join(d, e.name))) : [path.join(d, e.name)])
const files = walk(uiSrc).filter((f) => /\.(svelte|ts)$/.test(f) && !/\.test\.ts$/.test(f) && !f.endsWith('bindings.ts'))
const used = new Set()
for (const f of files) {
  for (const m of fs.readFileSync(f, 'utf8').matchAll(/\bt\(\s*(['"`])((?:\\.|(?!\1)[^\\])*)\1/g)) {
    used.add(m[2].replace(/\\(['"`])/g, '$1'))
  }
}
const keysFile = path.join(uiSrc, 'locales', 'keys.json')
const sorted = [...used].sort()
if (process.argv.includes('--check')) {
  const known = new Set(JSON.parse(fs.readFileSync(keysFile, 'utf8')))
  const missing = sorted.filter((k) => !known.has(k)), stale = [...known].filter((k) => !used.has(k))
  if (missing.length || stale.length) {
    console.error('keys.json out of date; run `node scripts/i18n.mjs`\n missing:', missing, '\n unused:', stale)
    process.exit(1)
  }
  console.log(`i18n keys ok (${sorted.length})`)
  process.exit(0)
}

// Full dictionaries come from RimSort's locales (kept out of the repo): run scripts/ts_to_json.mjs first.
const rawDir = path.resolve('debug/locales-full')
if (!fs.existsSync(rawDir)) { console.error('run: node scripts/ts_to_json.mjs (writes debug/locales-full)'); process.exit(1) }
fs.mkdirSync(path.dirname(keysFile), { recursive: true })
fs.writeFileSync(keysFile, JSON.stringify(sorted, null, 1))
// Hand-written strings for keys RimSort has no translation for (they win over RimSort's, some of which are wrong, e.g. es Save = "Ahorrar").
const overrides = JSON.parse(fs.readFileSync(path.resolve('scripts/i18n_overrides.json'), 'utf8'))
for (const f of fs.readdirSync(rawDir)) {
  const lang = f.replace('.json', '')
  const full = { ...JSON.parse(fs.readFileSync(path.join(rawDir, f), 'utf8')), ...(overrides[lang] ?? {}) }
  const trimmed = Object.fromEntries(sorted.filter((k) => full[k]).map((k) => [k, full[k]]))
  fs.writeFileSync(path.join(uiSrc, 'locales', f), JSON.stringify(trimmed, null, 1))
  console.log(f.replace('.json', '').padEnd(6), `${Object.keys(trimmed).length}/${sorted.length} keys translated`)
}
