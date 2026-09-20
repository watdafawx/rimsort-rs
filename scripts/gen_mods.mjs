// Generate a synthetic RimWorld install for benchmarks.
// Usage: node scripts/gen_mods.mjs <count> <outDir>   (default: 5000 mods -> debug/synth)
// Layout: <out>/game (Data/Core, Version.txt, Mods), <out>/workshop/<id>/About/About.xml, <out>/config/ModsConfig.xml,
//         <out>/settings.json (RimSort-format settings pointing at it).
import fs from 'node:fs'
import path from 'node:path'

const count = Number(process.argv[2] ?? 5000)
const out = path.resolve(process.argv[3] ?? 'debug/synth')
let seed = 12345
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32)
const pick = (n) => Math.floor(rnd() * n)

fs.rmSync(out, { recursive: true, force: true })
const game = path.join(out, 'game'), ws = path.join(out, 'workshop'), cfg = path.join(out, 'config')
for (const d of [path.join(game, 'Data/Core/About'), path.join(game, 'Mods'), ws, cfg]) fs.mkdirSync(d, { recursive: true })
fs.writeFileSync(path.join(game, 'Version.txt'), '1.6.4871 rev590')
fs.writeFileSync(path.join(game, 'Data/Core/About/About.xml'), '<ModMetaData><packageId>ludeon.rimworld</packageId></ModMetaData>')

const words = 'alpha beta gamma delta ember frost gale haze iris jade kilo lumen mist nova opal pine quartz rune sage tide umbra vale wisp xenon yarrow zephyr'.split(' ')
const ids = []
for (let i = 0; i < count; i++) {
  const pid = `synth.mod${i}`
  const name = `${words[pick(words.length)]} ${words[pick(words.length)]} ${i}`
  const after = [...new Set(Array.from({ length: pick(4) }, () => (i ? pick(i) : -1)).filter((x) => x >= 0))]
  const deps = [...new Set(Array.from({ length: pick(3) }, () => (i ? pick(i) : -1)).filter((x) => x >= 0))]
  const xml = `<?xml version="1.0" encoding="utf-8"?>
<ModMetaData>
  <name>${name}</name>
  <author>Author ${pick(400)}</author>
  <packageId>${pid}</packageId>
  <supportedVersions><li>1.5</li>${rnd() < 0.9 ? '<li>1.6</li>' : ''}</supportedVersions>
  <description>${'Lorem ipsum dolor sit amet, consectetur adipiscing elit. '.repeat(6)}</description>
  <modDependencies>${deps.map((d) => `<li><packageId>synth.mod${d}</packageId><displayName>Mod ${d}</displayName></li>`).join('')}</modDependencies>
  <loadAfter>${after.map((d) => `<li>synth.mod${d}</li>`).join('')}</loadAfter>
</ModMetaData>`
  const dir = path.join(ws, String(1000000 + i))
  fs.mkdirSync(path.join(dir, 'About'), { recursive: true })
  fs.writeFileSync(path.join(dir, 'About/About.xml'), xml)
  ids.push(pid)
}
// Shuffled active list (Fisher-Yates) so the sort has real work to do.
for (let i = ids.length - 1; i > 0; i--) { const j = pick(i + 1); [ids[i], ids[j]] = [ids[j], ids[i]] }
fs.writeFileSync(path.join(cfg, 'ModsConfig.xml'), `<?xml version="1.0"?>\n<ModsConfigData>\n  <version>1.6.4871 rev590</version>\n  <activeMods>\n    <li>ludeon.rimworld</li>\n${ids.map((p) => `    <li>${p}</li>`).join('\n')}\n  </activeMods>\n  <knownExpansions/>\n</ModsConfigData>`)
fs.writeFileSync(path.join(out, 'settings.json'), JSON.stringify({
  current_instance: 'Synth',
  instances: { Synth: { name: 'Synth', game_folder: game, config_folder: cfg, local_folder: path.join(game, 'Mods'), workshop_folder: ws } },
}, null, 2))
console.log(`generated ${count} mods in ${out}`)
