import {
  commands,
  type CycleDto,
  type ModRow,
  type SaveInfo,
  type SettingsView,
  type ToddsOptions,
  type Warning,
} from '../bindings'
import { call, external, toast, waitTask } from './ipc.svelte'
import { clearDetails } from './hovercache'
import { t } from './i18n.svelte'

export const app = $state({
  /** Background fetch of Steam Workshop details (0 = idle). */
  syncTask: 0,
  /** Newest save game (its mod list drives the "new" markers), if any. */
  save: null as SaveInfo | null,
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
  /** Background job (e.g. SteamCMD download) shown in the status bar; 0 = none. */
  jobTask: 0,
  cycles: [] as CycleDto[],
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
    case 'WorkshopUpdate':
      return 'A newer version is on the Workshop (Steam has not downloaded it yet)'
    case 'UseThisInstead':
      return `A maintained replacement exists: ${w.other_name}`
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

// ── inactive list ordering (the active list's order is the load order) ──
export type SortKey = 'name' | 'author' | 'modified' | 'type'
const SORT_KEY = 'rimsort-rs.inactiveSort'
export const inactiveSort = $state({ key: 'name' as SortKey, desc: false })
try {
  Object.assign(inactiveSort, JSON.parse(localStorage.getItem(SORT_KEY) ?? '{}'))
} catch {
  /* storage unavailable: defaults */
}

function inactiveCmp(a: ModRow, b: ModRow): number {
  let c = 0
  switch (inactiveSort.key) {
    case 'author':
      c = a.authors.toLowerCase().localeCompare(b.authors.toLowerCase())
      break
    case 'modified':
      c = a.modified - b.modified
      break
    case 'type':
      c = a.mod_type.localeCompare(b.mod_type)
      break
  }
  return (inactiveSort.desc ? -c : c) || byName(a, b)
}

export function setInactiveSort(key: SortKey, desc: boolean) {
  inactiveSort.key = key
  inactiveSort.desc = desc
  app.inactive = app.inactive.slice().sort(inactiveCmp)
  try {
    localStorage.setItem(SORT_KEY, JSON.stringify(inactiveSort))
  } catch {
    /* ignore */
  }
}

export async function loadSettings() {
  app.settings = await call(commands.getSettings())
}

/**
 * Rescan disk and refresh both lists. By default the active list is re-read from ModsConfig.xml
 * (discarding unsaved edits); with `keep` the current active list survives the rescan.
 */
export async function refresh(keep = false) {
  if (app.scanning) return
  app.scanning = true
  external.changed = null
  try {
    app.scanTask = await call(commands.startScan(keep))
    const t = await waitTask(app.scanTask)
    if (t.status !== 'finished') return
    const l = await call(commands.getLists())
    app.active = l.active
    app.inactive = l.inactive.slice().sort(inactiveCmp)
    app.missing = l.missing
    app.gameVersion = l.game_version
    app.scanMs = l.scan_ms
    app.duplicates = l.duplicate_package_ids
    app.communityRules = l.community_rules
    try {
      app.save = await call(commands.latestSave())
    } catch {
      app.save = null // markers are a nicety; never block a scan on them
    }
    if (!keep) app.dirty = false
    undoStack.length = redoStack.length = 0 // snapshots may reference mods that just changed
    syncDepth()
    app.loaded = true
    clearDetails()
    revalidate(0)
    void startWorkshopSync()
  } finally {
    app.scanning = false
  }
}

async function pushActive() {
  app.dirty = true
  await call(commands.setActive(app.active.map((r) => r.id)))
  revalidate()
}

/** After enabling mods: offer to also enable installed-but-inactive dependencies they need. */
async function offerDependencies(enabled: ModRow[]) {
  const names = new Set(enabled.map((r) => r.name))
  const missing = await call(commands.getMissingDependencies())
  const wanted = missing.filter((d) => d.installed && d.required_by.some((n) => names.has(n)))
  if (!wanted.length) return
  toast(
    wanted.length === 1
      ? t('{n} required mod is installed but inactive', { n: 1 })
      : t('{n} required mods are installed but inactive', { n: wanted.length }),
    12000,
    { label: t('Enable'), run: () => void enable(wanted.map((d) => d.installed!)) },
  )
}

/** Move rows into the active list before `beforeId` (end when null). Accepts ids from either list. */
export async function enable(ids: string[], beforeId: string | null = null, offerDeps = true) {
  const set = new Set(ids)
  const moving = app.inactive.filter((r) => set.has(r.id))
  const bad = moving.filter((r) => !r.valid)
  if (bad.length) toast(t("Can't enable invalid mod: {name}", { name: bad[0].name }))
  const ok = moving.filter((r) => r.valid)
  if (!ok.length) return
  remember()
  app.inactive = app.inactive.filter((r) => !set.has(r.id) || !r.valid)
  const at = beforeId ? app.active.findIndex((r) => r.id === beforeId) : -1
  const next = app.active.slice()
  next.splice(at < 0 ? next.length : at, 0, ...ok)
  app.active = next
  await pushActive()
  if (offerDeps) await offerDependencies(ok)
}

export function disable(ids: string[]) {
  const set = new Set(ids)
  const moving = app.active.filter((r) => set.has(r.id))
  if (!moving.length) return
  remember()
  app.active = app.active.filter((r) => !set.has(r.id))
  app.inactive = [...app.inactive, ...moving].sort(inactiveCmp)
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

/** Make the given installed copy the active one for its package id (swaps in place). */
export async function useCopy(id: string) {
  const row = app.inactive.find((r) => r.id === id)
  if (!row) return
  const at = app.active.findIndex((r) => r.package_id === row.package_id)
  if (at < 0) return enable([id])
  remember()
  const old = app.active[at]
  const next = app.active.slice()
  next[at] = row
  app.active = next
  app.inactive = [...app.inactive.filter((r) => r.id !== id), old].sort(inactiveCmp)
  await pushActive()
}

/** Update one row in place (e.g. after editing its color/tags), in whichever list holds it. */
export function patchRow(id: string, patch: Partial<ModRow>) {
  for (const key of ['active', 'inactive'] as const) {
    const i = app[key].findIndex((r) => r.id === id)
    if (i >= 0) {
      const next = app[key].slice()
      next[i] = { ...next[i], ...patch }
      app[key] = next
    }
  }
}

/** Run a background job shown in the status bar; rescan afterwards unless there are unsaved edits. */
async function runJob(start: Promise<number>, doneText: string): Promise<boolean> {
  app.jobTask = await start
  const t = await waitTask(app.jobTask)
  app.jobTask = 0
  if (t.status !== 'finished') return false
  toast(doneText, 4000)
  await refresh(app.dirty) // keep unsaved edits
  return true
}

/** Download Workshop items via SteamCMD. */
export async function downloadMods(ids: string[]) {
  if (app.jobTask) return toast(t('A download is already running'))
  await runJob(
    call(commands.downloadMods(ids)),
    `Downloaded ${ids.length} mod${ids.length === 1 ? '' : 's'}`,
  )
}

export type BulkMeta = { color?: string | null; addTag?: string }

/** Apply a colour and/or an extra tag to many mods at once, keeping each mod's own notes and other tags. */
export async function bulkMeta(ids: string[], patch: BulkMeta) {
  let done = 0
  for (const id of ids) {
    const d = await call(commands.getMod(id))
    if (!d) continue
    const tags = patch.addTag && !d.tags.includes(patch.addTag) ? [...d.tags, patch.addTag] : d.tags
    const color = 'color' in patch ? (patch.color ?? null) : d.color
    await call(commands.setModMeta(id, { color, tags, note: d.note }))
    patchRow(id, { color, tags })
    done++
  }
  toast(t('Updated {n} mods', { n: done }), 2500)
}

/** Subscribe to / unsubscribe from Workshop items in the Steam client (Steam downloads or removes them itself). */
export async function steamSubscribe(ids: string[], subscribe: boolean) {
  const results = await call(commands.steamSetSubscribed(ids, subscribe))
  const failed = results.filter((r) => r.error)
  if (failed.length) {
    toast(`Steam: ${failed[0].error}`, 8000)
  } else {
    toast(
      subscribe
        ? t('Subscribed — Steam will download it shortly')
        : t('Unsubscribed — Steam will remove it shortly'),
      5000,
    )
  }
}

/** Fetch Steam details for Workshop mods in the background (slow on purpose; results are cached on disk). */
export async function startWorkshopSync() {
  if (app.syncTask) return
  try {
    const id = await call(commands.startWorkshopSync())
    if (!id) return
    app.syncTask = id
    await waitTask(id)
  } catch {
    /* offline or Steam unreachable: the cards just show less */
  } finally {
    app.syncTask = 0
  }
}

/** Run todds (texture optimizer / clean-up) as a background job. */
export async function runTodds(options: ToddsOptions, doneText: string): Promise<boolean> {
  if (app.jobTask) {
    toast(t('A background job is already running'))
    return false
  }
  return runJob(call(commands.runTodds(options)), doneText)
}

/** Copy a (Workshop) mod into the local mods folder. */
export async function createLocalCopy(id: string) {
  if (app.jobTask) return toast(t('A background job is already running'))
  await runJob(
    call(commands.createLocalCopy(id)),
    'Local copy created — pick which copy to use in Duplicates',
  )
}

/** Clone GitHub repositories into the local mods folder. */
export async function cloneGitMods(urls: string[]) {
  if (app.jobTask) return toast('A download is already running')
  await runJob(
    call(commands.cloneGitMods(urls)),
    `Cloned ${urls.length} repositor${urls.length === 1 ? 'y' : 'ies'}`,
  )
}

/** Move a mod's folder to the recycle bin (after the caller confirmed) and refresh the lists. */
export async function deleteMod(id: string) {
  const wasActive = app.active.some((r) => r.id === id)
  await call(commands.deleteMod(id))
  const l = await call(commands.getLists())
  app.active = l.active
  app.inactive = l.inactive.slice().sort(inactiveCmp)
  app.missing = l.missing
  // The removed mod can't be undone back into the lists, so history would point at a dead id.
  undoStack.length = redoStack.length = 0
  syncDepth()
  if (wasActive) app.dirty = true
  revalidate(0)
  toast(t('Moved to the Recycle Bin'), 3000)
}

/** Disable everything except the base game and official expansions. */
export function clearActive() {
  disable(app.active.filter((r) => r.mod_type !== 'Ludeon').map((r) => r.id))
}

export async function sort() {
  const before = snap()
  const r = await call(commands.sortActive())
  app.cycles = r.cycles
  if (!r.ok) return toast(t('Sort failed: circular dependencies found'))
  const l = await call(commands.getLists())
  if (r.changed) {
    undoStack.push(before)
    redoStack.length = 0
    syncDepth()
    app.dirty = true
  }
  app.active = l.active
  revalidate(0)
  toast(r.changed ? t('Sorted') : t('Already sorted'), 2500)
}

/** Replace the active list from a file (JSON, ModsConfig/.rml/.rws XML, text, clipboard report). */
export async function importList(path: string) {
  await replaceActive(() => commands.importModlist(path))
}

/** Load a named snapshot as the active list (undoable). */
export async function loadSnapshot(name: string) {
  await replaceActive(() => commands.loadSnapshot(name))
}

async function replaceActive(load: () => ReturnType<typeof commands.importModlist>) {
  const before = snap()
  const r = await call(load())
  const l = await call(commands.getLists())
  undoStack.push(before)
  redoStack.length = 0
  syncDepth()
  app.active = l.active
  app.inactive = l.inactive.slice().sort(inactiveCmp)
  app.missing = l.missing
  app.dirty = true
  revalidate(0)
  toast(
    r.missing.length
      ? t('Imported {n} mods, {m} not installed', { n: r.imported, m: r.missing.length })
      : t('Imported {n} mods', { n: r.imported }),
    4000,
  )
}

export async function save() {
  const r = await call(commands.saveModsConfig())
  app.dirty = false
  toast(
    r.backup
      ? t('Saved {n} active mods (previous file backed up)', { n: r.count })
      : t('Saved {n} active mods', { n: r.count }),
    4000,
  )
}
