<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { open as pickFile, save as pickSave } from '@tauri-apps/plugin-dialog'
  import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener'
  import { onMount } from 'svelte'
  import type { ExportFormat, ModDetail, ModRow } from './bindings'
  import { call, commands, external, listenTasks, tasks, toast, toasts } from './lib/ipc.svelte'
  import Backups from './lib/Backups.svelte'
  import ConfirmDelete from './lib/ConfirmDelete.svelte'
  import DownloadDialog from './lib/DownloadDialog.svelte'
  import Duplicates from './lib/Duplicates.svelte'
  import LogView from './lib/LogView.svelte'
  import MetaEditor from './lib/MetaEditor.svelte'
  import MissingDeps from './lib/MissingDeps.svelte'
  import RuleEditor from './lib/RuleEditor.svelte'
  import ModList from './lib/ModList.svelte'
  import SettingsDialog from './lib/SettingsDialog.svelte'
  import {
    app,
    clearActive,
    createLocalCopy,
    deleteMod,
    downloadMods,
    describe,
    isError,
    disable,
    importList,
    enable,
    loadSettings,
    moveActive,
    redo,
    refresh,
    save,
    sort,
    undo,
  } from './lib/store.svelte'

  let showSettings = $state(false)
  let detail = $state<ModDetail | null>(null)
  let showMissing = $state(false)
  let listMenu = $state(false)
  let showDeps = $state(false)
  let showDups = $state(false)
  let showDownload = $state(false)
  let showBackups = $state(false)
  let deleting = $state<ModDetail | null>(null)
  let editRuleId = $state<string | null>(null)
  let editMetaId = $state<string | null>(null)
  let view = $state<'mods' | 'log'>('mods')

  async function doImport() {
    const p = await pickFile({
      title: 'Import mod list',
      filters: [{ name: 'Mod lists', extensions: ['json', 'xml', 'rml', 'rws', 'txt'] }],
    })
    if (typeof p === 'string') await importList(p)
  }

  const EXPORTS: { format: ExportFormat; label: string; file: string; ext: string }[] = [
    { format: 'Xml', label: 'Export as ModsConfig XML…', file: 'ModsConfig', ext: 'xml' },
    { format: 'Json', label: 'Export as RimSort JSON…', file: 'modlist', ext: 'json' },
    { format: 'PackageIds', label: 'Export package ids…', file: 'modlist', ext: 'txt' },
  ]
  async function doExport(x: (typeof EXPORTS)[number]) {
    const p = await pickSave({
      defaultPath: `${x.file}.${x.ext}`,
      filters: [{ name: x.ext.toUpperCase(), extensions: [x.ext] }],
    })
    if (p) {
      await call(commands.exportModlist(p, x.format))
      toast('Exported', 2500)
    }
  }
  async function copyList(format: ExportFormat, what: string) {
    await navigator.clipboard.writeText(await call(commands.exportModlistText(format)))
    toast(`${what} copied`, 2000)
  }
  let menu = $state<{
    x: number
    y: number
    row: ModRow
    ids: string[]
    list: 'active' | 'inactive'
    d: ModDetail | null
  } | null>(null)

  async function openMenu(
    list: 'active' | 'inactive',
    row: ModRow,
    ids: string[],
    x: number,
    y: number,
  ) {
    menu = { x, y, row, ids, list, d: null }
    const d = await call(commands.getMod(row.id))
    if (menu?.row.id === row.id) menu.d = d
  }

  const copy = (text: string) =>
    navigator.clipboard.writeText(text).then(() => toast('Copied', 1500))
  const workshopUrl = (pfid: string) =>
    `https://steamcommunity.com/sharedfiles/filedetails/?id=${pfid}`
  async function updateGit(id: string) {
    toast('Running git pull…', 2000)
    const out = await call(commands.updateGitMod(id))
    toast(out.split('\n').slice(-1)[0] || 'Up to date', 5000)
  }
  const steamUrl = (pfid: string) => `steam://url/CommunityFilePage/${pfid}`
  function act(fn: () => unknown) {
    try {
      fn()
    } finally {
      menu = null
    }
  }

  // ── resizable panes (persisted per viewer) ──────────────────────────
  const LAYOUT_KEY = 'rimsort-rs.layout'
  const layout = $state({ info: 280, split: 0.5 })
  let bodyEl: HTMLElement
  try {
    Object.assign(layout, JSON.parse(localStorage.getItem(LAYOUT_KEY) ?? '{}'))
  } catch {
    /* storage unavailable or corrupt: keep defaults */
  }

  function startDrag(e: PointerEvent, which: 'info' | 'split') {
    e.preventDefault()
    const target = e.currentTarget as HTMLElement
    target.setPointerCapture(e.pointerId)
    const rect = bodyEl.getBoundingClientRect()
    const move = (ev: PointerEvent) => {
      if (which === 'info') {
        layout.info = Math.max(160, Math.min(rect.width * 0.5, ev.clientX - rect.left))
      } else {
        const left = rect.left + layout.info + 6
        const width = rect.width - layout.info - 12
        layout.split = Math.max(0.2, Math.min(0.8, (ev.clientX - left) / width))
      }
    }
    const up = () => {
      target.removeEventListener('pointermove', move)
      target.removeEventListener('pointerup', up)
      try {
        localStorage.setItem(LAYOUT_KEY, JSON.stringify(layout))
      } catch {
        /* ignore */
      }
    }
    target.addEventListener('pointermove', move)
    target.addEventListener('pointerup', up)
  }

  // ── theme ────────────────────────────────────────────────────────────
  const THEME_KEY = 'rimsort-rs.theme'
  let theme = $state<'auto' | 'dark' | 'light'>('auto')
  try {
    const saved = localStorage.getItem(THEME_KEY)
    if (saved === 'dark' || saved === 'light') theme = saved
  } catch {
    /* storage unavailable */
  }
  $effect(() => {
    const root = document.documentElement
    if (theme === 'auto') root.removeAttribute('data-theme')
    else root.dataset.theme = theme
    try {
      localStorage.setItem(THEME_KEY, theme)
    } catch {
      /* ignore */
    }
  })

  // Warn before closing the window with unsaved list changes.
  onMount(() => {
    const un = getCurrentWindow().onCloseRequested((e) => {
      if (app.dirty && !confirm('You have unsaved changes. Close without saving?'))
        e.preventDefault()
    })
    return () => un.then((f) => f())
  })

  onMount(() => {
    const unlisten = listenTasks()
    ;(async () => {
      await unlisten
      await loadSettings()
      const s = app.settings!
      if (s.warning) toast(s.warning, 20000)
      if (s.checks.some((c) => c.kind === 'game' && !c.ok)) showSettings = true
      else await refresh()
    })()
    return () => unlisten.then((f) => f())
  })

  async function select(row: ModRow) {
    detail = await call(commands.getMod(row.id))
  }

  async function switchInstance(name: string) {
    if (app.dirty && !confirm('Discard unsaved changes?')) return
    await call(commands.switchInstance(name))
    await loadSettings()
    await refresh()
  }

  async function doRefresh() {
    if (app.dirty && !confirm('Discard unsaved changes and rescan?')) return
    await refresh()
  }

  async function run() {
    if (
      app.dirty &&
      !confirm('You have unsaved changes; RimWorld will use the last saved list. Launch anyway?')
    )
      return
    await call(commands.launchGame())
  }

  const scan = $derived(tasks[app.scanTask])
  const dropInto =
    (to: 'active' | 'inactive') => (from: string, ids: string[], beforeId: string | null) => {
      if (to === 'active') {
        if (from === 'active') moveActive(ids, beforeId)
        else enable(ids, beforeId)
      } else if (from === 'active') disable(ids)
    }
</script>

<svelte:window
  onclick={() => {
    menu = null
    listMenu = false
  }}
  onkeydown={(e) => {
    if (e.key === 'Escape') menu = null
    if ((e.ctrlKey || e.metaKey) && !(e.target instanceof HTMLInputElement)) {
      const k = e.key.toLowerCase()
      if (k === 'z' && !e.shiftKey) undo()
      else if (k === 'y' || (k === 'z' && e.shiftKey)) redo()
    }
  }}
  oncontextmenu={(e) =>
    e.target instanceof Element && !e.target.closest('.row') && e.preventDefault()}
/>

<div class="app">
  <header class="top">
    <strong>RimSort-rs</strong>
    {#if app.settings}
      <select
        aria-label="Instance"
        value={app.settings.current_instance}
        onchange={(e) => switchInstance(e.currentTarget.value)}
      >
        {#each app.settings.instances as i (i.name)}<option>{i.name}</option>{/each}
      </select>
    {/if}
    <span class="spacer"></span>
    {#if app.dirty}<span class="dirty" title="Changes not yet written to ModsConfig.xml"
        >● unsaved</span
      >{/if}
    <select id="theme" aria-label="Theme" title="Theme" bind:value={theme}>
      <option value="auto">Auto theme</option>
      <option value="dark">Dark</option>
      <option value="light">Light</option>
    </select>
    <button id="settings" onclick={() => (showSettings = true)}>⚙ Settings</button>
  </header>

  <nav class="toolbar">
    <button id="refresh" onclick={doRefresh} disabled={app.scanning}>⟳ Refresh</button>
    <button id="sort" onclick={sort} disabled={!app.loaded}>⇅ Sort</button>
    <button id="save" class="primary" onclick={save} disabled={!app.loaded}>💾 Save</button>
    <button id="run" onclick={run} disabled={!app.loaded}>▶ Run</button>
    <div class="dropdown">
      <button
        id="listmenu"
        disabled={!app.loaded}
        onclick={(e) => {
          e.stopPropagation()
          listMenu = !listMenu
        }}>⇄ List ▾</button
      >
      {#if listMenu}
        <div class="menu" role="menu" style:position="absolute" style:top="100%" style:left="0">
          <button role="menuitem" onclick={doImport}>Import list…</button>
          <button role="menuitem" onclick={() => (showBackups = true)}>Restore from backup…</button>
          <button role="menuitem" onclick={() => (showDownload = true)}> Download mods… </button>
          <hr />
          {#each EXPORTS as x (x.format)}
            <button role="menuitem" onclick={() => doExport(x)}>{x.label}</button>
          {/each}
          <hr />
          <button role="menuitem" onclick={() => copyList('Report', 'Report')}>Copy report</button>
          <button role="menuitem" onclick={() => copyList('PackageIds', 'Package ids')}>
            Copy package ids
          </button>
        </div>
      {/if}
    </div>
    <button id="undo" onclick={undo} disabled={!app.undoDepth} title="Undo (Ctrl+Z)">↶</button>
    <button id="redo" onclick={redo} disabled={!app.redoDepth} title="Redo (Ctrl+Y)">↷</button>
    {#if app.missingDeps}
      <button id="deps" onclick={() => (showDeps = true)}>🧩 {app.missingDeps} missing</button>
    {/if}
    <button
      id="logtab"
      class:active={view === 'log'}
      onclick={() => (view = view === 'log' ? 'mods' : 'log')}
    >
      📜 Log
    </button>
    <button id="clear" onclick={clearActive} disabled={!app.loaded}>Clear</button>
  </nav>

  {#if external.changed}
    <div class="banner external">
      {external.changed === 'config'
        ? 'ModsConfig.xml was changed outside RimSort-rs.'
        : 'Mods were added, removed or changed on disk.'}
      {#if app.dirty && external.changed === 'config'}<span class="dim"
          >(refreshing discards your unsaved changes)</span
        >{/if}
      {#if external.changed === 'mods' && app.dirty}
        <button onclick={() => refresh(true)}>Rescan, keep my changes</button>
      {:else}
        <button onclick={doRefresh}>Refresh</button>
      {/if}
      <button class="link" onclick={() => (external.changed = null)}>Dismiss</button>
    </div>
  {/if}

  {#if app.missing.length}
    <div class="banner">
      <button class="link" onclick={() => (showMissing = !showMissing)}>
        ⚠ {app.missing.length} mod{app.missing.length === 1 ? '' : 's'} in ModsConfig.xml {app
          .missing.length === 1
          ? 'is'
          : 'are'} not installed
      </button>
      {#if showMissing}<div class="missing">{app.missing.join(', ')}</div>{/if}
    </div>
  {/if}

  {#if view === 'log'}
    <div class="body single"><LogView /></div>
  {/if}
  <main
    class="body"
    bind:this={bodyEl}
    style:display={view === 'log' ? 'none' : undefined}
    style:grid-template-columns="{layout.info}px 6px {layout.split}fr 6px {1 - layout.split}fr"
  >
    <aside class="info">
      {#if detail}
        {#if detail.preview}<img class="preview" src={convertFileSrc(detail.preview)} alt="" />{/if}
        <h3>{detail.name}</h3>
        <dl>
          <dt>Package</dt>
          <dd>{detail.package_id}</dd>
          {#if detail.authors.length}<dt>Authors</dt>
            <dd>{detail.authors.join(', ')}</dd>{/if}
          <dt>Type</dt>
          <dd>
            {detail.mod_type}{detail.published_file_id ? ` · ${detail.published_file_id}` : ''}
          </dd>
          {#if detail.supported_versions.length}<dt>Supports</dt>
            <dd>{detail.supported_versions.join(', ')}</dd>{/if}
          {#if detail.mod_version}<dt>Version</dt>
            <dd>{detail.mod_version}</dd>{/if}
          <dt>Path</dt>
          <dd class="path">{detail.path}</dd>
          {#if detail.dependencies.length}<dt>Depends on</dt>
            <dd>{detail.dependencies.join(', ')}</dd>{/if}
          {#if detail.load_after.length}<dt>Load after</dt>
            <dd>{detail.load_after.join(', ')}</dd>{/if}
          {#if detail.load_before.length}<dt>Load before</dt>
            <dd>{detail.load_before.join(', ')}</dd>{/if}
          {#if detail.incompatible_with.length}<dt>Incompatible</dt>
            <dd>{detail.incompatible_with.join(', ')}</dd>{/if}
        </dl>
        {#if app.warnings[detail.id]}
          <ul class="warnings">
            {#each app.warnings[detail.id] as w (w.kind + w.other)}
              <li class:err={isError(w)}>
                {describe(w)}
                {#if w.kind === 'UseThisInstead' && w.other}
                  <button class="link" onclick={() => openUrl(workshopUrl(w.other))}>Open</button>
                  <button
                    class="link"
                    disabled={!!app.jobTask}
                    onclick={() => downloadMods([w.other])}>Download</button
                  >
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
        {#if detail.tags.length}<p class="dim">Tags: {detail.tags.join(', ')}</p>{/if}
        {#if detail.note}<p class="note">📝 {detail.note}</p>{/if}
        {#if detail.invalid_reason}<p class="bad">{detail.invalid_reason}</p>{/if}
        <p class="desc">{detail.description}</p>
      {:else}
        <p class="dim">Select a mod to see its details.</p>
      {/if}
    </aside>
    <div
      class="gutter"
      role="separator"
      aria-orientation="vertical"
      onpointerdown={(e) => startDrag(e, 'info')}
    ></div>

    <ModList
      title="Inactive"
      listId="inactive"
      rows={app.inactive}
      onmove={dropInto('inactive')}
      onactivate={(ids) => enable(ids)}
      onselect={select}
      oncontext={(r, ids, x, y) => openMenu('inactive', r, ids, x, y)}
    />
    <div
      class="gutter"
      role="separator"
      aria-orientation="vertical"
      onpointerdown={(e) => startDrag(e, 'split')}
    ></div>
    <ModList
      title="Active"
      listId="active"
      rows={app.active}
      warnings={app.warnings}
      onmove={dropInto('active')}
      onactivate={(ids) => disable(ids)}
      onselect={select}
      oncontext={(r, ids, x, y) => openMenu('active', r, ids, x, y)}
    />
  </main>

  <footer class="status">
    {#if app.jobTask && tasks[app.jobTask]}
      {@const j = tasks[app.jobTask]}
      <span class="job">⬇ {j.msg || 'Working…'}</span>
      <progress max={j.total || 1} value={j.done}></progress>
      <button onclick={() => call(commands.cancelTask(j.id))}>Cancel</button>
    {:else if app.scanning}
      <span>Scanning… {scan?.done ?? 0} / {scan?.total ?? '?'}</span>
      <progress max={scan?.total || 1} value={scan?.done ?? 0}></progress>
    {:else if app.loaded}
      <span>RimWorld {app.gameVersion}</span>
      <span>{app.active.length} active · {app.inactive.length} inactive</span>
      {#if app.errorCount}<span class="err">⛔ {app.errorCount} with errors</span>{/if}
      {#if app.warningCount}<span class="warn">⚠ {app.warningCount} with warnings</span>{/if}
      <span class="dim"
        >scanned in {app.scanMs} ms · {app.communityRules
          ? `${app.communityRules} community rules`
          : 'no community rules'}</span
      >
      {#if app.duplicates}<button class="link dim" onclick={() => (showDups = true)}
          >{app.duplicates} duplicate package ids</button
        >{/if}
    {:else}
      <span class="dim">No mods loaded — check Settings → Locations.</span>
    {/if}
  </footer>

  <div class="toasts">
    {#each toasts as t (t.id)}<div class="toast">{t.text}</div>{/each}
  </div>

  {#if app.cycles.length}
    <div class="backdrop" role="presentation" onclick={() => (app.cycles = [])}>
      <div
        class="dialog"
        role="alertdialog"
        aria-label="Unable to sort"
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
        onkeydown={() => {}}
      >
        <h2>Unable to sort</h2>
        <p>
          These load-order rules contradict each other, so no valid order exists. Remove or change
          one rule in each group (mods → right-click → Edit rules… for community/your rules).
        </p>
        {#each app.cycles as c (c.members.join())}
          <section class="cycle">
            <strong>{c.members.join(' ⇄ ')}</strong>
            <ul>
              {#each c.rules as line (line)}<li>{line}</li>{/each}
            </ul>
          </section>
        {/each}
        <button onclick={() => (app.cycles = [])}>Close</button>
      </div>
    </div>
  {/if}

  {#if menu}
    {@const d = menu.d}
    {@const many = menu.ids.length > 1}
    <div
      class="menu"
      role="menu"
      style:left="{Math.min(menu.x, innerWidth - 230)}px"
      style:top="{Math.min(menu.y, innerHeight - 260)}px"
    >
      {#if menu.list === 'active'}
        <button role="menuitem" onclick={() => act(() => disable(menu!.ids))}
          >Disable{many ? ` ${menu.ids.length} mods` : ''}</button
        >
      {:else}
        <button role="menuitem" onclick={() => act(() => enable(menu!.ids))}
          >Enable{many ? ` ${menu.ids.length} mods` : ''}</button
        >
      {/if}
      <hr />
      <button role="menuitem" disabled={!d} onclick={() => act(() => revealItemInDir(d!.path))}
        >Open folder</button
      >
      <button
        role="menuitem"
        disabled={!menu.row.published_file_id}
        onclick={() => act(() => openUrl(workshopUrl(menu!.row.published_file_id!)))}
        >Open Workshop page</button
      >
      <button
        role="menuitem"
        disabled={!menu.row.published_file_id}
        onclick={() => act(() => openUrl(steamUrl(menu!.row.published_file_id!)))}
        >Open in Steam client</button
      >
      <button
        role="menuitem"
        disabled={!d?.url.startsWith('http')}
        onclick={() => act(() => openUrl(d!.url))}>Open mod URL</button
      >
      <hr />
      {#if menu.row.mod_type === 'SteamWorkshop'}
        <button role="menuitem" onclick={() => act(() => createLocalCopy(menu!.row.id))}>
          Create local copy
        </button>
      {/if}
      {#if menu.row.mod_type === 'Git'}
        <button role="menuitem" onclick={() => act(() => updateGit(menu!.row.id))}>
          Update (git pull)
        </button>
      {/if}
      <button role="menuitem" onclick={() => act(() => (editMetaId = menu!.row.id))}
        >Color, tags &amp; notes…</button
      >
      <button role="menuitem" onclick={() => act(() => (editRuleId = menu!.row.id))}
        >Edit rules…</button
      >
      <button
        role="menuitem"
        class="danger-item"
        disabled={!d || menu.row.mod_type === 'Ludeon'}
        onclick={() => act(() => (deleting = d))}>Delete mod…</button
      >
      <hr />
      <button role="menuitem" onclick={() => act(() => copy(menu!.row.package_id))}
        >Copy package id</button
      >
      <button role="menuitem" onclick={() => act(() => copy(menu!.row.name))}>Copy name</button>
      <button role="menuitem" disabled={!d} onclick={() => act(() => copy(d!.path))}
        >Copy path</button
      >
    </div>
  {/if}

  {#if editMetaId}<MetaEditor id={editMetaId} onclose={() => (editMetaId = null)} />{/if}

  {#if editRuleId}<RuleEditor id={editRuleId} onclose={() => (editRuleId = null)} />{/if}

  {#if deleting}
    <ConfirmDelete
      mod={deleting}
      onclose={() => (deleting = null)}
      onconfirm={async () => {
        const id = deleting!.id
        deleting = null
        await deleteMod(id)
      }}
    />
  {/if}

  {#if showBackups}<Backups onclose={() => (showBackups = false)} />{/if}

  {#if showDownload}<DownloadDialog onclose={() => (showDownload = false)} />{/if}

  {#if showDups}<Duplicates onclose={() => (showDups = false)} />{/if}

  {#if showDeps}<MissingDeps onclose={() => (showDeps = false)} />{/if}

  {#if showSettings}<SettingsDialog onclose={() => (showSettings = false)} />{/if}
</div>

<style>
  .app {
    height: 100vh;
    display: grid;
    grid-template-rows: auto auto auto 1fr auto;
  }
  .top,
  .toolbar,
  .status {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.4rem 0.75rem;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
  }
  .status {
    border-top: 1px solid var(--line);
    border-bottom: 0;
    font-size: 0.85rem;
    gap: 1.25rem;
  }
  .spacer {
    flex: 1;
  }
  .dirty {
    color: #ecc94b;
  }
  .job {
    max-width: 50ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .banner.external {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    background: #23405f;
  }
  .banner {
    padding: 0.3rem 0.75rem;
    background: #4a3b10;
  }
  .missing {
    font-size: 0.8rem;
    margin-top: 0.25rem;
    word-break: break-all;
  }
  .body {
    display: grid;
    gap: 0;
    padding: 0.6rem;
    min-height: 0;
  }
  .gutter {
    cursor: col-resize;
    background: transparent;
    touch-action: none;
  }
  .gutter:hover,
  .gutter:active {
    background: var(--accent);
    opacity: 0.5;
  }
  .info {
    overflow: auto;
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 0.6rem 0.8rem;
    min-width: 0;
  }
  .info h3 {
    margin: 0 0 0.5rem;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.2rem 0.6rem;
    margin: 0;
    font-size: 0.85rem;
  }
  dt {
    color: var(--dim);
  }
  dd {
    margin: 0;
    word-break: break-word;
  }
  .preview {
    width: 100%;
    border-radius: 4px;
    margin-bottom: 0.5rem;
  }
  .body.single {
    grid-template-columns: 1fr;
  }
  button.active {
    background: var(--sel);
  }
  .menu .danger-item:not(:disabled) {
    color: #f56565;
  }
  .cycle {
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 0.5rem 0.7rem;
    margin: 0.5rem 0;
    font-size: 0.85rem;
  }
  .cycle ul {
    margin: 0.3rem 0 0;
    padding-left: 1.1rem;
  }
  .dropdown {
    position: relative;
  }
  .menu {
    position: fixed;
    z-index: 30;
    min-width: 200px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 0.25rem;
    display: grid;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.4);
  }
  .menu button {
    text-align: left;
    border: 0;
    background: none;
    padding: 0.35rem 0.6rem;
    border-radius: 4px;
  }
  .menu hr {
    border: 0;
    border-top: 1px solid var(--line);
    margin: 0.25rem 0;
    width: 100%;
  }
  .path {
    font-size: 0.75rem;
  }
  .note {
    white-space: pre-wrap;
    font-size: 0.85rem;
    background: var(--panel);
    border-left: 3px solid var(--accent);
    padding: 0.3rem 0.5rem;
    margin: 0.4rem 0;
  }
  .desc {
    white-space: pre-wrap;
    font-size: 0.85rem;
    max-height: 40vh;
    overflow: auto;
  }
  .bad,
  .err {
    color: #f56565;
  }
  .warn {
    color: #ecc94b;
  }
  .warnings {
    margin: 0.5rem 0;
    padding-left: 1.1rem;
    font-size: 0.85rem;
    color: #ecc94b;
  }
  .warnings li.err {
    color: #f56565;
  }
  .dim {
    color: var(--dim);
  }
  .link {
    background: none;
    border: 0;
    color: inherit;
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
  }
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
    max-width: 80vw;
    max-height: 80vh;
    overflow: auto;
  }
</style>
