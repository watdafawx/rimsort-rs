<script lang="ts">
  import { dialogFocus } from './actions'
  import { onMount } from 'svelte'
  import type { SnapshotInfo } from '../bindings'
  import { call, commands, toast } from './ipc.svelte'
  import { t } from './i18n.svelte'
  import { loadSnapshot } from './store.svelte'

  let { oncompare, onclose }: { oncompare: (key: string) => void; onclose: () => void } = $props()

  let list = $state<SnapshotInfo[]>([])
  let name = $state('')
  let loaded = $state(false)

  const refresh = async () => {
    list = await call(commands.listSnapshots())
    loaded = true
  }
  onMount(refresh)

  async function save() {
    const n = name.trim()
    if (!n) return
    if (
      list.some((s) => s.name.toLowerCase() === n.toLowerCase()) &&
      !confirm(t('Replace the snapshot “{name}”?', { name: n }))
    )
      return
    await call(commands.saveSnapshot(n))
    name = ''
    toast(t('Snapshot saved'), 2500)
    await refresh()
  }

  async function load(s: SnapshotInfo) {
    onclose()
    await loadSnapshot(s.name)
  }

  async function remove(s: SnapshotInfo) {
    if (!confirm(t('Delete the snapshot “{name}”?', { name: s.name }))) return
    await call(commands.deleteSnapshot(s.name))
    await refresh()
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={t('Snapshots')}
    tabindex="-1"
    use:dialogFocus
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>{t('Snapshots')}</h2>
    <p class="dim">
      {t(
        'Named load orders you can switch between. Loading one replaces your active list (undoable); press Save to write it.',
      )}
    </p>
    <form
      class="new"
      onsubmit={(e) => {
        e.preventDefault()
        void save()
      }}
    >
      <input
        placeholder={t('Name for the current list…')}
        bind:value={name}
        aria-label={t('Snapshot name')}
      />
      <button class="primary" type="submit" disabled={!name.trim()}>{t('Save snapshot')}</button>
    </form>
    <ul>
      {#each list as s (s.name)}
        <li>
          <div class="what">
            <strong>{s.name}</strong>
            <span class="dim"
              >{t('{n} mods', { n: s.count })} · {new Date(s.created * 1000).toLocaleString()}</span
            >
          </div>
          <div class="acts">
            <button onclick={() => load(s)}>{t('Load')}</button>
            <button onclick={() => oncompare(`snapshot:${s.name}`)}>{t('Compare')}</button>
            <button class="ghost" onclick={() => remove(s)} aria-label={t('Delete')}>✕</button>
          </div>
        </li>
      {:else}
        {#if loaded}<li class="dim">{t('No snapshots yet.')}</li>{/if}
      {/each}
    </ul>
    <footer><button onclick={onclose}>{t('Close')}</button></footer>
  </div>
</div>

<style>
  .dialog {
    width: min(620px, 92vw);
  }
  .new {
    display: flex;
    gap: 0.5rem;
  }
  .new input {
    flex: 1;
  }
  ul {
    max-height: 46vh;
    overflow: auto;
    display: grid;
    gap: 0.3rem;
  }
  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.4rem 0.6rem;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--bg);
  }
  .what {
    display: grid;
  }
  .acts {
    display: flex;
    gap: 0.35rem;
  }
</style>
