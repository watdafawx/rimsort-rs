<script lang="ts">
  import { dialogFocus } from './actions'
  import { open } from '@tauri-apps/plugin-dialog'
  import { getVersion } from '@tauri-apps/api/app'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { onMount } from 'svelte'
  import type { DbResult, Fix, InstanceDto } from '../bindings'
  import { i18n, LANGS, setLanguage, t, T } from './i18n.svelte'
  import { call, commands, toast } from './ipc.svelte'
  import { setTheme, theme, type ThemeMode } from './theme.svelte'
  import { RECENT_CHOICES, prefs, setDensity, setPref, setRecentDays } from './prefs.svelte'
  import { setZoom, ZOOM_STEPS, zoom } from './zoom.svelte'
  import { app, loadSettings, refresh } from './store.svelte'

  let { onclose }: { onclose: () => void } = $props()

  const current = () =>
    app.settings!.instances.find((i) => i.name === app.settings!.current_instance)!
  let draft = $state<InstanceDto>({ ...current() })
  let notes = $state<string[]>([])
  let newName = $state('')
  const TABS = [
    ['locations', T('Locations')],
    ['sorting', T('Sorting')],
    ['appearance', T('Appearance')],
    ['databases', T('Databases')],
    ['maintenance', T('Maintenance')],
    ['about', T('About')],
  ] as const
  let tab = $state<(typeof TABS)[number][0]>('locations')
  let dbResults = $state<DbResult[]>([])
  let dbBusy = $state(false)
  let version = $state('')
  onMount(() => {
    getVersion().then(
      (v) => (version = v),
      () => {},
    )
  })
  const SHORTCUTS: [string, string][] = [
    ['Ctrl + S', 'Save ModsConfig.xml'],
    ['Ctrl + Z / Ctrl + Y', 'Undo / redo list changes'],
    ['Ctrl + K', 'Command palette: jump to any mod or run any action'],
    ['Ctrl + Shift + F', 'Search in mod files'],
    ['Ctrl + + / − / 0', 'Interface size'],
    ['Enter / Delete', 'Move the selected mods to the other list'],
    ['Double-click', 'Move a mod to the other list'],
    ['Esc', 'Close dialog or menu'],
  ]

  async function updateDbs() {
    dbBusy = true
    try {
      dbResults = await call(commands.updateDatabases())
    } finally {
      dbBusy = false
    }
  }
  let opts = $state({ ...app.settings!.options })

  const FIXES: { fix: Fix; title: string; help: string }[] = [
    {
      fix: 'ModSettings',
      title: 'Reset mod settings',
      help: 'Moves the per-mod settings files (Config/Mod_*.xml) to the Recycle Bin. Mods start with their defaults; your load order is untouched.',
    },
    {
      fix: 'GameSettings',
      title: 'Reset game settings',
      help: 'Moves Prefs.xml and KeyPrefs.xml (graphics, volume, key bindings) to the Recycle Bin. ModsConfig.xml is never touched.',
    },
    {
      fix: 'SteamDownloadCache',
      title: 'Clear Steam download cache',
      help: 'Moves Steam’s unfinished Workshop downloads to the Recycle Bin — fixes mods stuck on “updating”.',
    },
  ]

  async function runFix(f: (typeof FIXES)[number]) {
    const files = await call(commands.troubleshootPreview(f.fix))
    if (!files.length) {
      toast('Nothing to clear', 2500)
      return
    }
    const shown = files.slice(0, 8).map((p) => p.split(/[/\\]/).pop())
    if (files.length > shown.length) shown.push(`…and ${files.length - shown.length} more`)
    const question = `${f.title}: move ${files.length} item(s) to the Recycle Bin?`
    if (!confirm([question, '', ...shown].join('\n'))) return
    const n = await call(commands.troubleshootApply(f.fix))
    toast(`Moved ${n} item(s) to the Recycle Bin`, 3500)
  }

  type PathKey = 'game_folder' | 'config_folder' | 'local_folder' | 'workshop_folder'
  const FIELDS: [PathKey, string][] = [
    ['game_folder', T('Game folder')],
    ['config_folder', T('Config folder (ModsConfig.xml)')],
    ['local_folder', T('Local mods folder')],
    ['workshop_folder', T('Steam Workshop folder')],
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
    use:dialogFocus
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>{t('Settings')}</h2>
    <div class="tabs" role="tablist">
      {#each TABS as [id, label] (id)}
        <button
          role="tab"
          aria-selected={tab === id}
          class:on={tab === id}
          onclick={() => (tab = id)}
        >
          {t(label)}
        </button>
      {/each}
    </div>

    {#if tab === 'locations'}
      <div class="row">
        <label for="inst">{t('Instance')}</label>
        <select
          id="inst"
          value={app.settings!.current_instance}
          onchange={(e) => switchTo(e.currentTarget.value)}
        >
          {#each app.settings!.instances as i (i.name)}<option>{i.name}</option>{/each}
        </select>
        <button onclick={remove} disabled={app.settings!.instances.length < 2}>{t('Delete')}</button
        >
        <input placeholder={t('New instance name')} bind:value={newName} />
        <button onclick={create} disabled={!newName.trim()}>{t('Create')}</button>
      </div>

      {#each FIELDS as [key, label] (key)}
        {@const check = app.settings!.checks.find((c) => c.kind === key.replace('_folder', ''))}
        <div class="field">
          <label for={key}>{t(label)}</label>
          <div class="row">
            <input id={key} bind:value={draft[key]} spellcheck="false" />
            <button onclick={() => browse(key)}>{t('Browse…')}</button>
          </div>
          {#if check && !check.ok && check.path === draft[key]}<small class="bad"
              >{check.reason}</small
            >{/if}
        </div>
      {/each}

      <div class="field">
        <label for="run_args">{t('Game launch arguments')}</label>
        <input
          id="run_args"
          bind:value={draft.run_args}
          spellcheck="false"
          placeholder="-popupwindow"
        />
        <label class="check"
          ><input type="checkbox" bind:checked={draft.launch_via_steam} />
          {t('Launch through Steam (arguments are ignored)')}</label
        >
      </div>

      <div class="row">
        <button onclick={detect}>{t('Auto-detect')}</button>
        <span class="dim">{notes.join(' · ')}</span>
      </div>
    {/if}

    {#if tab === 'sorting'}
      <fieldset>
        <legend>{t('Sorting & validation')}</legend>
        <div class="row">
          <label for="sort-algo">{t('Sorting algorithm')}</label>
          <select
            id="sort-algo"
            value={opts.alphabetical_sort ? 'alpha' : 'topo'}
            onchange={(e) => (opts.alphabetical_sort = e.currentTarget.value === 'alpha')}
          >
            <option value="topo">{t('Topological (recommended)')}</option>
            <option value="alpha">{t('Alphabetical (deprecated in RimSort)')}</option>
          </select>
        </div>
        <label class="check"
          ><input type="checkbox" bind:checked={opts.dependencies_as_load_after} />
          {t('Treat declared dependencies as “load after” when sorting')}</label
        >
        <label class="check"
          ><input type="checkbox" bind:checked={opts.use_alternative_ids} />
          {t('Alternative package ids satisfy a dependency')}</label
        >
        <label class="check"
          ><input type="checkbox" bind:checked={opts.prefer_versioned} />
          {t('Prefer version-specific About.xml entries (needs rescan)')}</label
        >
      </fieldset>
    {/if}

    {#if tab === 'appearance'}
      <fieldset>
        <legend>{t('Appearance')}</legend>
        <div class="row">
          <label for="theme-select">{t('Theme')}</label>
          <select
            id="theme-select"
            value={theme.mode}
            onchange={(e) => setTheme(e.currentTarget.value as ThemeMode)}
          >
            <option value="auto">{t('Match system')}</option>
            <option value="dark">{t('Dark')}</option>
            <option value="light">{t('Light')}</option>
          </select>
          <label for="lang-select">{t('Language')}</label>
          <select
            id="lang-select"
            value={i18n.lang}
            onchange={(e) => setLanguage(e.currentTarget.value)}
          >
            {#each LANGS as l (l.code)}<option value={l.code}>{l.label}</option>{/each}
          </select>
        </div>
        <div class="row">
          <label for="zoom-select">{t('Interface size')}</label>
          <select
            id="zoom-select"
            value={zoom.level}
            onchange={(e) => setZoom(Number(e.currentTarget.value))}
          >
            {#each ZOOM_STEPS as z (z)}<option value={z}>{Math.round(z * 100)}%</option>{/each}
          </select>
          <span class="dim">Ctrl + / Ctrl − / Ctrl 0</span>
        </div>
        <div class="row">
          <label for="recent-select">{t('Mark recently changed mods')}</label>
          <select
            id="recent-select"
            value={prefs.recentDays}
            onchange={(e) => setRecentDays(Number(e.currentTarget.value))}
          >
            {#each RECENT_CHOICES as d (d)}
              <option value={d}
                >{d === 0
                  ? t('Off')
                  : d === 1
                    ? t('Last day')
                    : t('Last {n} days', { n: d })}</option
              >
            {/each}
          </select>
        </div>
        <div class="row">
          <label for="density-select">{t('Row density')}</label>
          <select
            id="density-select"
            value={prefs.density}
            onchange={(e) =>
              setDensity(e.currentTarget.value as 'compact' | 'normal' | 'comfortable')}
          >
            <option value="compact">{t('Compact')}</option>
            <option value="normal">{t('Normal')}</option>
            <option value="comfortable">{t('Comfortable')}</option>
          </select>
        </div>
        <label class="check"
          ><input
            type="checkbox"
            checked={prefs.sourceIcons}
            onchange={(e) => setPref('sourceIcons', e.currentTarget.checked)}
          />
          {t('Show source icons (Steam, Ludeon, local folder)')}</label
        >
        <label class="check"
          ><input
            type="checkbox"
            checked={prefs.typeIcons}
            onchange={(e) => setPref('typeIcons', e.currentTarget.checked)}
          />
          {t('Show C# / XML content icons')}</label
        >
        <label class="check"
          ><input
            type="checkbox"
            checked={prefs.previewSort}
            onchange={(e) => setPref('previewSort', e.currentTarget.checked)}
          />
          {t('Preview what Sort will change before applying it')}</label
        >
        <label class="check"
          ><input
            type="checkbox"
            checked={prefs.autoRefresh}
            onchange={(e) => setPref('autoRefresh', e.currentTarget.checked)}
          />
          {t('Rescan automatically when mods change on disk')}</label
        >
        <label class="check"
          ><input
            type="checkbox"
            checked={prefs.saveMarks}
            onchange={(e) => setPref('saveMarks', e.currentTarget.checked)}
          />
          {t('Mark mods that are new since the latest save game')}</label
        >
      </fieldset>
    {/if}

    {#if tab === 'databases'}
      <fieldset>
        <legend>{t('Rule databases')}</legend>
        <div class="row">
          <button onclick={updateDbs} disabled={dbBusy}
            >{dbBusy ? t('Updating…') : t('Update databases')}</button
          >
          <span class="dim"
            >{t(
              'Community rules, Use This Instead, No Version Warning. Applied on the next rescan.',
            )}</span
          >
        </div>
        {#each dbResults as r (r.name)}
          <div class="dbrow" class:bad={r.status === 'Failed'}>
            <strong>{r.name}</strong>
            <span
              >{r.status === 'Updated'
                ? t('updated')
                : r.status === 'NotModified'
                  ? t('already up to date')
                  : t('failed')} — {r.detail}</span
            >
          </div>
        {/each}
      </fieldset>
    {/if}

    {#if tab === 'about'}
      <fieldset>
        <legend>RimSort-rs {version}</legend>
        <p class="dim">
          A fast rewrite of RimSort in Rust, Tauri and Svelte. It reads and writes
          RimSort-compatible settings, rules and lists. Licensed GPL-3.0, derived from RimSort.
        </p>
        <div class="row">
          <button onclick={() => openUrl('https://github.com/RimSort/RimSort')}
            >RimSort project</button
          >
          <button onclick={() => openUrl('https://github.com/joseasoler/todds')}>todds</button>
        </div>
      </fieldset>
      <fieldset>
        <legend>Keyboard shortcuts</legend>
        <dl class="keys">
          {#each SHORTCUTS as [k, what] (k)}
            <dt><kbd>{k}</kbd></dt>
            <dd>{what}</dd>
          {/each}
        </dl>
      </fieldset>
    {/if}

    {#if tab === 'maintenance'}
      <fieldset>
        <legend>Troubleshooting</legend>
        {#each FIXES as f (f.fix)}
          <div class="fix">
            <div>
              <strong>{f.title}</strong>
              <p class="dim">{f.help}</p>
            </div>
            <button onclick={() => runFix(f)}>Review…</button>
          </div>
        {/each}
        <div class="fix">
          <div>
            <strong>Verify game files</strong>
            <p class="dim">Asks Steam to check and repair the RimWorld installation.</p>
          </div>
          <button onclick={() => openUrl('steam://validate/294100')}>Open in Steam</button>
        </div>
      </fieldset>
    {/if}

    <footer>
      <button onclick={onclose}>{t('Cancel')}</button>
      <button class="primary" onclick={saveAndRescan}>{t('Save & rescan')}</button>
    </footer>
  </div>
</div>

<style>
  .tabs {
    display: flex;
    gap: 0.15rem;
    border-bottom: 1px solid var(--line);
    margin: 0 -0.25rem;
  }
  .tabs button {
    border: 0;
    border-bottom: 2px solid transparent;
    border-radius: 6px 6px 0 0;
    background: transparent;
    color: var(--dim);
    padding: 0.4rem 0.85rem;
  }
  .tabs button:hover:not(:disabled) {
    background: var(--hover);
    color: var(--fg);
  }
  .tabs button.on {
    color: var(--fg);
    border-bottom-color: var(--accent);
    font-weight: 600;
  }
  .dialog {
    min-height: 26rem;
  }
  .dialog {
    width: min(720px, 92vw);
  }

  .keys {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.3rem 1rem;
    margin: 0;
  }
  .keys dd {
    margin: 0;
    color: var(--dim);
  }
  .fix {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.35rem 0;
  }
  .fix + .fix {
    border-top: 1px solid var(--line);
  }
  .fix p {
    margin: 0.1rem 0 0;
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
  .dbrow {
    display: flex;
    gap: 0.6rem;
    font-size: 0.85rem;
  }
  .bad {
    color: #f56565;
  }
  .dim {
    color: var(--dim);
    font-size: 0.85em;
  }
</style>
