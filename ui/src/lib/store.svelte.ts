import { commands, type ModRow, type SettingsView, type Warning } from '../bindings'
import { call, external, toast, waitTask } from './ipc.svelte'

export const app = $state({
  settings: null as SettingsView | null,
  active: [] as ModRow[],
  inactive: [] as ModRow[],
  missing: [] as string[],
  gameVersion: '',
  scanMs: 0,
  duplicates: 0,
  dirty: false,
  scanning: false,
  scanTask: 0,
  cycles: [] as string[][],
  loaded: false,
  /** Active-list warnings keyed by mod id. */
  warnings: {} as Record<string, Warning[]>,
  errorCount: 0,
  warningCount: 0,
  missingDeps: 0,
  communityRules: 0,
  /** Undo/redo depth, for enabling the buttons. */
  undoDepth: 0,
  redoDepth: 0,
})

type Snap = { active: ModRow[]; inactive: ModRow[] }
const undoStack: Snap[] = []
const redoStack: Snap[] = []
const HISTORY_MAX = 100

const snap = (): Snap => ({ active: app.active, inactive: app.inactive })
function syncDepth() {
  app.undoDepth = undoStack.length
  app.redoDepth = redoStack.length
}
/** Call before mutating the lists. */
function remember() {
  undoStack.push(snap())
  if (undoStack.length > HISTORY_MAX) undoStack.shift()
  redoStack.length = 0
  syncDepth()
}
function restore(s: Snap) {
  app.active = s.active
  app.inactive = s.inactive
  pushActive()
  syncDepth()
}
export function undo() {
  const s = undoStack.pop()
  if (!s) return
  redoStack.push(snap())
  restore(s)
}
export function redo() {
  const s = redoStack.pop()
  if (!s) return
  undoStack.push(snap())
  restore(s)
}

export const isError = (w: Warning) => w.kind === 'MissingDependency' || w.kind === 'Incompatible'

export function describe(w: Warning): string {
  switch (w.kind) {
    case 'MissingDependency':
      return `Missing dependency: ${w.other_name}`
    case 'Incompatible':
      return `Incompatible with: ${w.other_name}`
    case 'LoadBefore':
      return `Should load before: ${w.other_name}`
    case 'LoadAfter':
      return `Should load after: ${w.other_name}`
    case 'VersionMismatch':
      return 'Does not list support for this game version'
  }
}

let validateTimer: ReturnType<typeof setTimeout> | undefined
/** Recompute warnings for the active list (debounced; runs in core, off the UI thread). */
export function revalidate(delay = 60) {
  clearTimeout(validateTimer)
  validateTimer = setTimeout(async () => {
    const v = await call(commands.getValidation())
    const map: Record<string, Warning[]> = {}
    for (const m of v.mods) map[m.id] = m.warnings
    app.warnings = map
    app.errorCount = v.errors
    app.warningCount = v.warnings
    app.missingDeps = v.missing_dependencies
  }, delay)
}

const byName = (a: ModRow, b: ModRow) => a.name.toLowerCase().localeCompare(b.name.toLowerCase())

export async function loadSettings() {
  app.settings = await call(commands.getSettings())
}

/** Rescan disk + reload ModsConfig.xml, then refresh both lists. */
export async function refresh() {
  if (app.scanning) return
  app.scanning = true
  external.changed = null
  try {
    app.scanTask = await call(commands.startScan())
    const t = await waitTask(app.scanTask)
    if (t.status !== 'finished') return
    const l = await call(commands.getLists())
    app.active = l.active
    app.inactive = l.inactive
    app.missing = l.missing
    app.gameVersion = l.game_version
    app.scanMs = l.scan_ms
    app.duplicates = l.duplicate_package_ids
    app.communityRules = l.community_rules
    app.dirty = false
    undoStack.length = redoStack.length = 0
    syncDepth()
    app.loaded = true
    revalidate(0)
  } finally {
    app.scanning = false
  }
}

async function pushActive() {
  app.dirty = true
  await call(commands.setActive(app.active.map((r) => r.id)))
  revalidate()
}

/** Move rows into the active list before `beforeId` (end when null). Accepts ids from either list. */
export async function enable(ids: string[], beforeId: string | null = null) {
  const set = new Set(ids)
  const moving = app.inactive.filter((r) => set.has(r.id))
  const bad = moving.filter((r) => !r.valid)
  if (bad.length) toast(`Can't enable invalid mod: ${bad[0].name}`)
  const ok = moving.filter((r) => r.valid)
  if (!ok.length) return
  remember()
  app.inactive = app.inactive.filter((r) => !set.has(r.id) || !r.valid)
  const at = beforeId ? app.active.findIndex((r) => r.id === beforeId) : -1
  const next = app.active.slice()
  next.splice(at < 0 ? next.length : at, 0, ...ok)
  app.active = next
  await pushActive()
}

export function disable(ids: string[]) {
  const set = new Set(ids)
  const moving = app.active.filter((r) => set.has(r.id))
  if (!moving.length) return
  remember()
  app.active = app.active.filter((r) => !set.has(r.id))
  app.inactive = [...app.inactive, ...moving].sort(byName)
  pushActive()
}

/** Reorder within the active list: put `ids` (in their current relative order) before `beforeId`. */
export function moveActive(ids: string[], beforeId: string | null) {
  remember()
  const set = new Set(ids)
  const moving = app.active.filter((r) => set.has(r.id))
  const rest = app.active.filter((r) => !set.has(r.id))
  const at = beforeId ? rest.findIndex((r) => r.id === beforeId) : -1
  rest.splice(at < 0 ? rest.length : at, 0, ...moving)
  app.active = rest
  pushActive()
}

/** Disable everything except the base game and official expansions. */
export function clearActive() {
  disable(app.active.filter((r) => r.mod_type !== 'Ludeon').map((r) => r.id))
}

export async function sort() {
  const before = snap()
  const r = await call(commands.sortActive())
  app.cycles = r.cycles
  if (!r.ok) return toast('Sort failed: circular dependencies found')
  const l = await call(commands.getLists())
  if (r.changed) {
    undoStack.push(before)
    redoStack.length = 0
    syncDepth()
    app.dirty = true
  }
  app.active = l.active
  revalidate(0)
  toast(r.changed ? 'Sorted' : 'Already sorted', 2500)
}

/** Replace the active list from a file (JSON, ModsConfig/.rml/.rws XML, text, clipboard report). */
export async function importList(path: string) {
  const before = snap()
  const r = await call(commands.importModlist(path))
  const l = await call(commands.getLists())
  undoStack.push(before)
  redoStack.length = 0
  syncDepth()
  app.active = l.active
  app.inactive = l.inactive
  app.missing = l.missing
  app.dirty = true
  revalidate(0)
  toast(
    `Imported ${r.imported} mods${r.missing.length ? `, ${r.missing.length} not installed` : ''}`,
    4000,
  )
}

export async function save() {
  const r = await call(commands.saveModsConfig())
  app.dirty = false
  toast(`Saved ${r.count} active mods${r.backup ? ' (previous file backed up)' : ''}`, 4000)
}
