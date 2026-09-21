<script lang="ts">
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
    aria-label="Restore from backup"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>Restore from backup</h2>
    <p class="dim">
      Every Save keeps the previous <code>ModsConfig.xml</code> (last 20). Loading one replaces the active
      list; press Save to write it.
    </p>
    {#if loaded && !backups.length}
      <p>No backups yet.</p>
    {:else}
      <ul>
        {#each backups as b (b.path)}
          <li>
            <span>{new Date(b.unix * 1000).toLocaleString()}</span>
            <span class="dim">{b.count} active mods</span>
            <button onclick={() => restore(b)}>Load</button>
          </li>
        {/each}
      </ul>
    {/if}
    <footer><button onclick={onclose}>Close</button></footer>
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
