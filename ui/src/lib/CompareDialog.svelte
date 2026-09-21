<script lang="ts">
  import { dialogFocus } from './actions'
  import { onMount } from 'svelte'
  import type { BackupInfo, ModRow, SnapshotInfo } from '../bindings'
  import { call, commands } from './ipc.svelte'
  import { t } from './i18n.svelte'
  import { movedItems } from './listdiff'
  import { app, enable } from './store.svelte'

  /** `kind:key` — save, backup:<path>, snapshot:<name>. */
  let { initial = 'save', onclose }: { initial?: string; onclose: () => void } = $props()

  let sources = $state<{ key: string; label: string }[]>([])
  let choice = $state('')
  let theirs = $state<string[] | null>(null)
  let backups = $state<BackupInfo[]>([])
  let snaps = $state<SnapshotInfo[]>([])

  const norm = (p: string) => {
    const l = p.toLowerCase()
    return l.endsWith('_steam') ? l.slice(0, -6) : l
  }

  onMount(async () => {
    ;[backups, snaps] = await Promise.all([
      call(commands.listBackups()).catch(() => []),
      call(commands.listSnapshots()).catch(() => []),
    ])
    sources = [
      ...(app.save
        ? [{ key: 'save', label: t('Latest save: {name}', { name: app.save.name }) }]
        : []),
      ...snaps.map((s) => ({
        key: `snapshot:${s.name}`,
        label: t('Snapshot: {name}', { name: s.name }),
      })),
      ...backups.map((b) => ({
        key: `backup:${b.path}`,
        label: t('Backup: {when}', { when: new Date(b.unix * 1000).toLocaleString() }),
      })),
    ]
    choice = sources.some((s) => s.key === initial) ? initial : (sources[0]?.key ?? '')
  })

  $effect(() => {
    const key = choice
    theirs = null
    if (!key) return
    const load = async (): Promise<string[]> => {
      if (key === 'save') return app.save?.package_ids ?? []
      if (key.startsWith('snapshot:')) return call(commands.snapshotIds(key.slice(9)))
      return call(commands.listFileIds(key.slice(7)))
    }
    load().then(
      (ids) => {
        if (choice === key) theirs = ids.map(norm)
      },
      () => {
        if (choice === key) theirs = []
      },
    )
  })

  const rows = $derived(new Map([...app.active, ...app.inactive].map((r) => [r.package_id, r])))
  const activeIds = $derived(app.active.map((r) => r.package_id))
  const diff = $derived.by(() => {
    if (!theirs) return null
    const mine = new Set(activeIds)
    const other = new Set(theirs)
    const added = app.active.filter((r) => !other.has(r.package_id))
    const removed = theirs
      .filter((p) => !mine.has(p))
      .map((p) => ({ pid: p, row: rows.get(p) as ModRow | undefined }))
    const moved = movedItems(theirs, activeIds).length
    return { added, removed, moved }
  })
  const installedRemoved = $derived((diff?.removed ?? []).filter((r) => r.row))

  async function reenable() {
    await enable(installedRemoved.map((r) => r.row!.id))
    onclose()
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={t('Compare lists')}
    tabindex="-1"
    use:dialogFocus
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>{t('Compare lists')}</h2>
    {#if !sources.length}
      <p class="dim">
        {t('Nothing to compare with yet: no save game, snapshot or backup was found.')}
      </p>
    {:else}
      <label class="pick">
        {t('Your active list, compared with')}
        <select bind:value={choice}>
          {#each sources as s (s.key)}<option value={s.key}>{s.label}</option>{/each}
        </select>
      </label>
      {#if diff}
        <p class="sum">
          <span class="a">+{diff.added.length}</span>
          <span class="r">−{diff.removed.length}</span>
          <span class="m">↕ {diff.moved}</span>
          <span class="dim">{t('added · removed · moved')}</span>
        </p>
        <div class="cols">
          <section>
            <h3>{t('Added since')}</h3>
            <ul>
              {#each diff.added.slice(0, 200) as r (r.id)}<li>{r.name}</li>{:else}<li class="dim">
                  —
                </li>{/each}
            </ul>
          </section>
          <section>
            <h3>{t('Removed since')}</h3>
            <ul>
              {#each diff.removed.slice(0, 200) as r (r.pid)}
                <li class:gone={!r.row}>{r.row?.name ?? r.pid}</li>
              {:else}<li class="dim">—</li>{/each}
            </ul>
          </section>
        </div>
      {:else}
        <p class="dim">{t('Working…')}</p>
      {/if}
    {/if}
    <footer>
      <button onclick={onclose}>{t('Close')}</button>
      {#if installedRemoved.length}
        <button class="primary" onclick={reenable}
          >{t('Re-enable {n} removed mods', { n: installedRemoved.length })}</button
        >
      {/if}
    </footer>
  </div>
</div>

<style>
  .dialog {
    width: min(760px, 94vw);
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }
  .pick select {
    flex: 1;
    min-width: 0;
  }
  .sum {
    display: flex;
    gap: 0.9rem;
    align-items: baseline;
    margin: 0;
    font-size: 1.05rem;
    font-weight: 650;
  }
  .a {
    color: var(--ok);
  }
  .r {
    color: var(--err);
  }
  .m {
    color: var(--warn);
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.8rem;
  }
  h3 {
    font-size: 0.85em;
    color: var(--dim);
    margin-bottom: 0.25rem;
  }
  ul {
    max-height: 40vh;
    overflow: auto;
    display: grid;
    gap: 1px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    padding: 0.25rem;
    background: var(--bg);
  }
  li {
    padding: 0.1rem 0.3rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  li.gone {
    color: var(--dim);
    text-decoration: line-through;
  }
</style>
