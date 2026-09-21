<script lang="ts">
  import { dialogFocus } from './actions'
  import { t } from './i18n.svelte'
  import { onMount } from 'svelte'
  import type { BackupInfo } from '../bindings'
  import { call, commands } from './ipc.svelte'
  import { importList } from './store.svelte'

  let { onclose }: { onclose: () => void } = $props()
  let backups = $state<BackupInfo[]>([])
  let loaded = $state(false)

  onMount(async () => {
    backups = await call(commands.listBackups())
    loaded = true
  })

  async function restore(b: BackupInfo) {
    onclose()
    await importList(b.path) // loads into the lists (undoable); press Save to write it
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={t('Restore from backup')}
    tabindex="-1"
    use:dialogFocus
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>{t('Restore from backup')}</h2>
    <p class="dim">
      {t(
        'Every Save keeps the previous ModsConfig.xml (last 20). Loading one replaces the active list; press Save to write it.',
      )}
    </p>
    {#if loaded && !backups.length}
      <p>{t('No backups yet.')}</p>
    {:else}
      <ul>
        {#each backups as b (b.path)}
          <li>
            <span>{new Date(b.unix * 1000).toLocaleString()}</span>
            <span class="dim">{t('{n} active mods', { n: b.count })}</span>
            <button onclick={() => restore(b)}>{t('Load')}</button>
          </li>
        {/each}
      </ul>
    {/if}
    <footer><button onclick={onclose}>{t('Close')}</button></footer>
  </div>
</div>

<style>
  .dialog {
    width: min(520px, 92vw);
  }

  p {
    margin: 0;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow: auto;
    display: grid;
    gap: 0.3rem;
  }
  li {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.35rem 0.6rem;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--panel);
  }
  li span:first-child {
    flex: 1;
  }
  .dim {
    color: var(--dim);
    font-size: 0.85em;
  }
</style>
