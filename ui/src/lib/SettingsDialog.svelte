<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import type { InstanceDto } from '../bindings'
  import { call, commands, toast } from './ipc.svelte'
  import { app, loadSettings, refresh } from './store.svelte'

  let { onclose }: { onclose: () => void } = $props()

  const current = () =>
    app.settings!.instances.find((i) => i.name === app.settings!.current_instance)!
  let draft = $state<InstanceDto>({ ...current() })
  let notes = $state<string[]>([])
  let newName = $state('')
  let opts = $state({ ...app.settings!.options })

  type PathKey = 'game_folder' | 'config_folder' | 'local_folder' | 'workshop_folder'
  const FIELDS: [PathKey, string][] = [
    ['game_folder', 'Game folder'],
    ['config_folder', 'Config folder (ModsConfig.xml)'],
    ['local_folder', 'Local mods folder'],
    ['workshop_folder', 'Steam Workshop folder'],
  ]

  async function browse(key: PathKey) {
    const picked = await open({ directory: true, defaultPath: draft[key] || undefined })
    if (typeof picked === 'string') draft[key] = picked
  }

  async function detect() {
    const d = await call(commands.autodetectPaths())
    draft.game_folder = d.game_folder ?? draft.game_folder
    draft.config_folder = d.config_folder ?? draft.config_folder
    draft.local_folder = d.local_folder ?? draft.local_folder
    draft.workshop_folder = d.workshop_folder ?? draft.workshop_folder
    notes = d.notes
  }

  async function saveAndRescan() {
    await call(commands.updateOptions($state.snapshot(opts)))
    await call(commands.saveInstance($state.snapshot(draft)))
    await loadSettings()
    onclose()
    await refresh()
  }

  async function switchTo(name: string) {
    await call(commands.switchInstance(name))
    await loadSettings()
    draft = { ...current() }
    await refresh()
  }

  async function create() {
    if (!newName.trim()) return
    await call(commands.createInstance(newName))
    newName = ''
    await loadSettings()
    toast('Instance created')
  }

  async function remove() {
    if (!confirm(`Delete instance "${draft.name}"? (Mods on disk are not touched.)`)) return
    await call(commands.deleteInstance(draft.name))
    await loadSettings()
    draft = { ...current() }
    await refresh()
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label="Settings"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>Settings — Locations</h2>

    <div class="row">
      <label for="inst">Instance</label>
      <select
        id="inst"
        value={app.settings!.current_instance}
        onchange={(e) => switchTo(e.currentTarget.value)}
      >
        {#each app.settings!.instances as i (i.name)}<option>{i.name}</option>{/each}
      </select>
      <button onclick={remove} disabled={app.settings!.instances.length < 2}>Delete</button>
      <input placeholder="New instance name" bind:value={newName} />
      <button onclick={create} disabled={!newName.trim()}>Create</button>
    </div>

    {#each FIELDS as [key, label] (key)}
      {@const check = app.settings!.checks.find((c) => c.kind === key.replace('_folder', ''))}
      <div class="field">
        <label for={key}>{label}</label>
        <div class="row">
          <input id={key} bind:value={draft[key]} spellcheck="false" />
          <button onclick={() => browse(key)}>Browse…</button>
        </div>
        {#if check && !check.ok && check.path === draft[key]}<small class="bad"
            >{check.reason}</small
          >{/if}
      </div>
    {/each}

    <div class="field">
      <label for="run_args">Game launch arguments</label>
      <input
        id="run_args"
        bind:value={draft.run_args}
        spellcheck="false"
        placeholder="-popupwindow"
      />
      <label class="check"
        ><input type="checkbox" bind:checked={draft.launch_via_steam} /> Launch through Steam (arguments
        are ignored)</label
      >
    </div>

    <fieldset>
      <legend>Sorting &amp; validation</legend>
      <label class="check"
        ><input type="checkbox" bind:checked={opts.dependencies_as_load_after} /> Treat declared dependencies
        as “load after” when sorting</label
      >
      <label class="check"
        ><input type="checkbox" bind:checked={opts.use_alternative_ids} /> Alternative package ids satisfy
        a dependency</label
      >
      <label class="check"
        ><input type="checkbox" bind:checked={opts.prefer_versioned} /> Prefer version-specific About.xml
        entries (needs rescan)</label
      >
    </fieldset>

    <div class="row">
      <button onclick={detect}>Auto-detect</button>
      <span class="dim">{notes.join(' · ')}</span>
    </div>

    <footer>
      <button onclick={onclose}>Cancel</button>
      <button class="primary" onclick={saveAndRescan}>Save &amp; rescan</button>
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
    width: min(720px, 92vw);
    display: grid;
    gap: 0.75rem;
  }
  h2 {
    margin: 0;
    font-size: 1.1rem;
  }
  .row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }
  .row input {
    flex: 1;
  }
  .field {
    display: grid;
    gap: 0.25rem;
  }
  fieldset {
    display: grid;
    gap: 0.35rem;
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  legend {
    color: var(--dim);
    padding: 0 0.3rem;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .check input {
    flex: none;
  }
  .bad {
    color: #f56565;
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
