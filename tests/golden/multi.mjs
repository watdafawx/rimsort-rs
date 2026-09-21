// Generate varied ModsConfig files (shuffles and random subsets of the real active list), then sort each
// with RimSort's Python and with ours and compare. Usage: node tests/golden/multi.mjs [count]
// Needs `just golden-setup` and a RimSort install; never writes any ModsConfig.xml except under debug/golden.
import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

const count = Number(process.argv[2] ?? 12)
const settings = JSON.parse(
  fs.readFileSync(path.join(process.env.LOCALAPPDATA, 'RimSort', 'settings.json'), 'utf8'),
)
const inst = settings.instances[settings.current_instance]
const src = fs.readFileSync(path.join(inst.config_folder, 'ModsConfig.xml'), 'utf8')
const active = [...src.matchAll(/<li>([^<]+)<\/li>/g)].map((m) => m[1])

// mulberry32: seeded so a failure is reproducible
let seed = 20260921
const rand = () => {
  seed = (seed + 0x6d2b79f5) | 0
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed)
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296
}
const shuffle = (a) => {
  const b = [...a]
  for (let i = b.length - 1; i > 0; i--) {
    const j = Math.floor(rand() * (i + 1))
    ;[b[i], b[j]] = [b[j], b[i]]
  }
  return b
}

const dir = path.resolve('debug/golden')
fs.rmSync(dir, { recursive: true, force: true })
fs.mkdirSync(dir, { recursive: true })
const write = (name, ids) =>
  fs.writeFileSync(
    path.join(dir, `${name}.xml`),
    `<?xml version="1.0" encoding="utf-8"?>\n<ModsConfigData><version>1.6.4871 rev590</version><activeMods>${ids.map((i) => `<li>${i}</li>`).join('')}</activeMods><knownExpansions><li>ludeon.rimworld.royalty</li><li>ludeon.rimworld.ideology</li></knownExpansions></ModsConfigData>`,
  )
write('c00_saved_order', active)
for (let i = 1; i < count; i++) {
  const keep = i % 2 ? 1 : 0.5 + rand() * 0.4
  write(`c${String(i).padStart(2, '0')}_${keep === 1 ? 'shuffle' : 'subset'}`, shuffle(active).filter(() => rand() < keep))
}

const env = { ...process.env, GOLDEN_DIR: dir, RIMSORT_RS_DATA_DIR: path.resolve('debug/testdata') }
console.log('python...')
execFileSync(
  'debug/pyvenv/Scripts/python.exe',
  ['tests/golden/py_sort.py', path.join(process.env.LOCALAPPDATA, 'RimSort', 'settings.json'), 'unused'],
  { env, stdio: 'inherit' },
)
console.log('rust...')
execFileSync('cargo', ['test', '-p', 'rimsort-core', '--test', 'real_machine', 'sort_golden_configs', '--', '--ignored'], {
  env,
  stdio: 'inherit',
})

let bad = 0
for (const f of fs.readdirSync(dir).filter((f) => f.endsWith('.py.json')).sort()) {
  const py = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8'))
  const rs = JSON.parse(fs.readFileSync(path.join(dir, f.replace('.py.json', '.rs.json')), 'utf8'))
  const same = py.sorted.filter((p, i) => p === rs.sorted[i]).length
  const ok = py.ok === rs.ok && same === py.sorted.length && py.sorted.length === rs.sorted.length
  if (!ok) bad++
  console.log(`${ok ? 'OK  ' : 'DIFF'} ${f.replace('.py.json', '')}: ${same}/${py.sorted.length} (python ok=${py.ok}, rust ok=${rs.ok})`)
}
process.exit(bad ? 1 : 0)
