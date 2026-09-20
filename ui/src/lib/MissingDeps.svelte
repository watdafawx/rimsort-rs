<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { onMount } from 'svelte'
  import type { MissingDep } from '../bindings'
  import { call, commands, toast } from './ipc.svelte'
  import { app, enable } from './store.svelte'

  let { onclose }: { onclose: () => void } = $props()
  let deps = $state<MissingDep[]>([])

  const load = async () => (deps = await call(commands.getMissingDependencies()))
  onMount(load)
  // Reload when validation reports a different count (e.g. edits made elsewhere).
  $effect(() => {
    void app.missingDeps
    load()
  })

  const installable = $derived(deps.filter((d) => d.installed))

  async function enableOne(d: MissingDep) {
    if (!d.installed) return
    await enable([d.installed])
    await load()
    toast('Enabled — use Sort to place it correctly', 3000)
  }

  async function enableAll() {
    await enable(installable.map((d) => d.installed!))
    await load()
    toast(`Enabled ${installable.length} mods — use Sort to place them correctly`, 3500)
  }

  const by = (d: MissingDep) =>
    d.required_by.slice(0, 3).join(', ') +
    (d.required_by.length > 3 ? ` +${d.required_by.length - 3} more` : '')
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label="Missing dependencies"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>Missing dependencies</h2>
    {#if !deps.length}
      <p class="dim">Nothing missing. 🎉</p>
    {:else}
      <p class="dim">
        {deps.length} required mod{deps.length === 1 ? ' is' : 's are'} not in your active list.
      </p>
      <ul>
        {#each deps as d (d.package_id)}
          <li>
            <div class="what">
              <strong>{d.name}</strong>
              <span class="dim">{d.package_id}</span>
              <div class="dim">Required by {by(d)}</div>
            </div>
            <div class="acts">
              {#if d.installed}
                <button class="primary" onclick={() => enableOne(d)}>Enable</button>
              {:else}
                <span class="dim">not installed</span>
              {/if}
              <button
                disabled={!d.workshop_id}
                onclick={() =>
                  openUrl(
                    `https://steamcommunity.com/sharedfiles/filedetails/?id=${d.workshop_id}`,
                  )}
              >
                Workshop
              </button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
    <footer>
      {#if installable.length > 1}<button onclick={enableAll}
          >Enable all installed ({installable.length})</button
        >{/if}
      <button onclick={onclose}>Close</button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: grid;
    place-items: center;
    z-index: 10;
  }
  .dialog {
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 1rem 1.25rem;
    width: min(760px, 92vw);
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  h2 {
    margin: 0;
    font-size: 1.1rem;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow: auto;
    display: grid;
    gap: 0.4rem;
  }
  li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    padding: 0.5rem 0.7rem;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--panel);
  }
  .what {
    min-width: 0;
  }
  .acts {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex: none;
  }
  .dim {
    color: var(--dim);
    font-size: 0.85em;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
