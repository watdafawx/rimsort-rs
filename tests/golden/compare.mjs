// Compare Python golden order with Rust order: node tests/golden/compare.mjs debug/py_sorted.json debug/rust_sorted.json
import fs from 'node:fs'
const [py, rs] = process.argv.slice(2).map((p) => JSON.parse(fs.readFileSync(p, 'utf8')))
const same = py.sorted.filter((p, i) => p === rs.sorted[i]).length
console.log(`python ok=${py.ok} rust ok=${rs.ok}; identical positions ${same}/${py.sorted.length}`)
const i = py.sorted.findIndex((p, i) => p !== rs.sorted[i])
if (i >= 0) console.log('first diff at', i, '\n python:', py.sorted.slice(i, i + 5), '\n rust:  ', rs.sorted.slice(i, i + 5))
process.exit(i >= 0 || py.ok !== rs.ok ? 1 : 0)
