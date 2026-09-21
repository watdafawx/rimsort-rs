import type { ModDetail, ModRow, WorkshopMeta } from '../bindings'
import { call, commands } from './ipc.svelte'

/**
 * In-memory caches behind the hover card, filled on mouse-enter (before the card shows) so the card
 * appears complete instead of popping in. Local mod details never change without a rescan; Steam
 * details only ever gain entries, and "not fetched yet" answers are deliberately not cached.
 * (The Steam data itself is persisted on disk by the backend and survives restarts.)
 */
const details = new Map<string, ModDetail>()
const steam = new Map<string, WorkshopMeta>()

export const cachedDetail = (id: string) => details.get(id) ?? null
export const cachedSteam = (pfid: string | null) => (pfid ? (steam.get(pfid) ?? null) : null)

/** A rescan can change any mod's details. */
export function clearDetails() {
  details.clear()
}

export async function prefetch(row: ModRow): Promise<void> {
  const jobs: Promise<unknown>[] = []
  if (!details.has(row.id)) {
    jobs.push(
      call(commands.getMod(row.id)).then((d) => {
        if (!d) return
        if (details.size > 400) details.delete(details.keys().next().value!)
        details.set(row.id, d)
      }),
    )
  }
  const pfid = row.published_file_id
  if (pfid && !steam.has(pfid)) {
    jobs.push(
      call(commands.getWorkshopMeta(pfid)).then((m) => {
        if (m) steam.set(pfid, m)
      }),
    )
  }
  await Promise.allSettled(jobs)
}
