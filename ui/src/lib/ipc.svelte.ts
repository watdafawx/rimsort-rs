import { commands, events, type ErrorDto, type TaskEvent } from '../bindings'

export type { ErrorDto }

// ── toasts ──────────────────────────────────────────────────────────────
export const toasts = $state<{ id: number; text: string }[]>([])
let toastSeq = 0
export function toast(text: string, ms = 5000) {
  const id = ++toastSeq
  toasts.push({ id, text })
  setTimeout(() => {
    const i = toasts.findIndex((t) => t.id === id)
    if (i >= 0) toasts.splice(i, 1)
  }, ms)
}

// ── typed invoke ────────────────────────────────────────────────────────
type Res<T> = { status: 'ok'; data: T } | { status: 'error'; error: ErrorDto }

/** Unwrap a specta command result; on error toast it and rethrow. */
export async function call<T>(p: Promise<Res<T>>): Promise<T> {
  const r = await p
  if (r.status === 'ok') return r.data
  toast(`${r.error.kind}: ${r.error.message}`)
  throw r.error
}

export { commands }

// ── tasks store ─────────────────────────────────────────────────────────
export type Task = {
  id: number
  status: 'running' | 'finished' | 'cancelled' | 'failed'
  done: number
  total: number
  msg: string
}
export const tasks = $state<Record<number, Task>>({})

const waiters = new Map<number, ((t: Task) => void)[]>()

/** Resolves when the task leaves `running` (immediately if it already has). */
export function waitTask(id: number): Promise<Task> {
  const t = tasks[id]
  if (t && t.status !== 'running') return Promise.resolve(t)
  return new Promise((res) => waiters.set(id, [...(waiters.get(id) ?? []), res]))
}

/** Set when files changed outside the app (watched mods folders / ModsConfig.xml). */
export const external = $state({ changed: null as 'mods' | 'config' | null })

function apply(e: TaskEvent) {
  if (e.kind === 'fs_changed') {
    // `config` outranks `mods` (it is the more disruptive change).
    if (e.what === 'config' || external.changed === null)
      external.changed = e.what as 'mods' | 'config'
    return
  }
  tasks[e.id] ??= { id: e.id, status: 'running', done: 0, total: 0, msg: '' }
  const t = tasks[e.id]
  if (e.kind === 'progress') Object.assign(t, { done: e.done, total: e.total, msg: e.msg })
  else if (e.kind === 'failed') {
    t.status = 'failed'
    toast(`task ${e.id} failed: ${e.error.message}`)
  } else t.status = e.kind
  if (t.status !== 'running') {
    waiters.get(e.id)?.forEach((r) => r(t))
    waiters.delete(e.id)
  }
}

/** Call once at startup. */
export function listenTasks() {
  return events.taskUpdate.listen((ev) => apply(ev.payload))
}
