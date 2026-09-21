<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { open as pickFile, save as pickSave } from '@tauri-apps/plugin-dialog'
  import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener'
  import { onMount } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import { initLanguage, t } from './lib/i18n.svelte'
  import { initTheme } from './lib/theme.svelte'
  import type { ExportFormat, ModDetail, ModRow } from './bindings'
  import {
    call,
    commands,
    dismissToast,
    external,
    listenTasks,
    tasks,
    toast,
    toasts,
  } from './lib/ipc.svelte'
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

  // Warn before closing the window with unsaved list changes.
  onMount(() => {
    const un = getCurrentWindow().onCloseRequested((e) => {
      if (app.dirty && !confirm('You have unsaved changes. Close without saving?'))
        e.preventDefault()
    })
    return () => un.then((f) => f())
  })

  onMount(() => {
    initLanguage()
    initTheme()
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
  <header class="topbar">
    <div class="brand"><Icon name="package" size={18} /><strong>RimSort-rs</strong></div>
    {#if app.settings}
      <select
        class="instance"
        aria-label="Instance"
        title="Instance"
        value={app.settings.current_instance}
        onchange={(e) => switchInstance(e.currentTarget.value)}
      >
        {#each app.settings.instances as i (i.name)}<option>{i.name}</option>{/each}
      </select>
    {/if}
    <span class="vsep"></span>
    <div class="group">
      <button id="refresh" onclick={doRefresh} disabled={app.scanning} title="Rescan mods (F5)">
        <Icon name="refresh" />{t('Refresh')}
      </button>
      <button id="sort" onclick={sort} disabled={!app.loaded} title="Sort the active list">
        <Icon name="sort" />{t('Sort')}
      </button>
      <button
        id="save"
        class="primary"
        onclick={save}
        disabled={!app.loaded}
        title="Write ModsConfig.xml (Ctrl+S)"
      >
        <Icon name="save" />{t('Save')}
      </button>
      <button id="run" onclick={run} disabled={!app.loaded} title="Launch RimWorld">
        <Icon name="play" />{t('Run')}
      </button>
    </div>
    <span class="vsep"></span>
    <div class="group">
      <div class="dropdown">
        <button
          id="listmenu"
          class="ghost"
          disabled={!app.loaded}
          onclick={(e) => {
            e.stopPropagation()
            listMenu = !listMenu
          }}><Icon name="list" />{t('List')}<Icon name="chevron" size={14} /></button
        >
        {#if listMenu}
          <div
            class="menu"
            role="menu"
            style:position="absolute"
            style:top="calc(100% + 4px)"
            style:left="0"
          >
            <button role="menuitem" onclick={doImport}>{t('Import list…')}</button>
            <button role="menuitem" onclick={() => (showBackups = true)}
              >{t('Restore from backup…')}</button
            >
            <button role="menuitem" onclick={() => (showDownload = true)}
              >{t('Download mods…')}</button
            >
            <hr />
            {#each EXPORTS as x (x.format)}
              <button role="menuitem" onclick={() => doExport(x)}>{x.label}</button>
            {/each}
            <hr />
            <button role="menuitem" onclick={() => copyList('Report', 'Report')}
              >{t('Copy report')}</button
            >
            <button role="menuitem" onclick={() => copyList('PackageIds', 'Package ids')}>
              Copy package ids
            </button>
          </div>
        {/if}
      </div>
      <button
        id="undo"
        class="ghost icon"
        onclick={undo}
        disabled={!app.undoDepth}
        title="Undo (Ctrl+Z)"
        aria-label="Undo"
      >
        <Icon name="undo" />
      </button>
      <button
        id="redo"
        class="ghost icon"
        onclick={redo}
        disabled={!app.redoDepth}
        title="Redo (Ctrl+Y)"
        aria-label="Redo"
      >
        <Icon name="redo" />
      </button>
      <button
        id="clear"
        class="ghost"
        onclick={clearActive}
        disabled={!app.loaded}
        title="Disable every mod except the base game and DLC"
      >
        <Icon name="trash" />{t('Clear')}
      </button>
    </div>
    <span class="vsep"></span>
    <div class="group">
      {#if app.missingDeps}
        <button
          id="deps"
          class="ghost attn"
          onclick={() => (showDeps = true)}
          title="Required mods that are not active"
        >
          <Icon name="link" />{app.missingDeps} missing
        </button>
      {/if}
      <button
        id="logtab"
        class="ghost"
        class:active={view === 'log'}
        onclick={() => (view = view === 'log' ? 'mods' : 'log')}
        title="Show RimWorld's Player.log"
      >
        <Icon name="log" />Log
      </button>
    </div>
    <span class="spacer"></span>
    {#if app.dirty}
      <span class="dirty" title="Changes not yet written to ModsConfig.xml"
        ><Icon name="dot" size={22} />{t('unsaved')}</span
      >
    {/if}
    <button id="settings" class="ghost" onclick={() => (showSettings = true)}>
      <Icon name="settings" />{t('Settings')}
    </button>
  </header>

  {#if external.changed}
    <div class="banner external">
      {external.changed === 'config'
        ? 'ModsConfig.xml was changed outside RimSort-rs.'
        : 'Mods were added, removed or changed on disk.'}
      {#if app.dirty && external.changed === 'config'}<span class="dim"
          >{t('(refreshing discards your unsaved changes)')}</span
        >{/if}
      {#if external.changed === 'mods' && app.dirty}
        <button onclick={() => refresh(true)}>{t('Rescan, keep my changes')}</button>
      {:else}
        <button onclick={doRefresh}>{t('Refresh')}</button>
      {/if}
      <button class="link" onclick={() => (external.changed = null)}>{t('Dismiss')}</button>
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
          <dt>{t('Package')}</dt>
          <dd>{detail.package_id}</dd>
          {#if detail.authors.length}<dt>{t('Authors')}</dt>
            <dd>{detail.authors.join(', ')}</dd>{/if}
          <dt>{t('Type')}</dt>
          <dd>
            {detail.mod_type}{detail.published_file_id ? ` · ${detail.published_file_id}` : ''}
          </dd>
          {#if detail.supported_versions.length}<dt>{t('Supports')}</dt>
            <dd>{detail.supported_versions.join(', ')}</dd>{/if}
          {#if detail.workshop_updated}<dt>{t('Updated')}</dt>
            <dd>{new Date(detail.workshop_updated * 1000).toLocaleDateString()}</dd>{/if}
          {#if detail.mod_version}<dt>{t('Version')}</dt>
            <dd>{detail.mod_version}</dd>{/if}
          <dt>{t('Path')}</dt>
          <dd class="path">{detail.path}</dd>
          {#if detail.dependencies.length}<dt>{t('Depends on')}</dt>
            <dd>{detail.dependencies.join(', ')}</dd>{/if}
          {#if detail.load_after.length}<dt>{t('Load after')}</dt>
            <dd>{detail.load_after.join(', ')}</dd>{/if}
          {#if detail.load_before.length}<dt>{t('Load before')}</dt>
            <dd>{detail.load_before.join(', ')}</dd>{/if}
          {#if detail.incompatible_with.length}<dt>{t('Incompatible')}</dt>
            <dd>{detail.incompatible_with.join(', ')}</dd>{/if}
        </dl>
        {#if app.warnings[detail.id]}
          <ul class="warnings">
            {#each app.warnings[detail.id] as w (w.kind + w.other)}
              <li class:err={isError(w)}>
                {describe(w)}
                {#if w.kind === 'UseThisInstead' && w.other}
                  <button class="link" onclick={() => openUrl(workshopUrl(w.other))}
                    >{t('Open')}</button
                  >
                  <button
                    class="link"
                    disabled={!!app.jobTask}
                    onclick={() => downloadMods([w.other])}>{t('Download')}</button
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
        <div class="empty">
          <Icon name="info" size={30} />
          <p>{t('Select a mod to see its details.')}</p>
        </div>
      {/if}
    </aside>
    <div
      class="gutter"
      role="separator"
      aria-orientation="vertical"
      onpointerdown={(e) => startDrag(e, 'info')}
    ></div>

    <ModList
      title={t('Inactive')}
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
      title={t('Active')}
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
      <Icon name="download" size={14} />
      <span class="job">{j.msg || 'Working…'}</span>
      <progress max={j.total || 1} value={j.done}></progress>
      <button class="ghost small" onclick={() => call(commands.cancelTask(j.id))}
        >{t('Cancel')}</button
      >
    {:else if app.scanning}
      <Icon name="refresh" size={14} />
      <span>Scanning… {scan?.done ?? 0} / {scan?.total ?? '?'}</span>
      <progress max={scan?.total || 1} value={scan?.done ?? 0}></progress>
    {:else if app.loaded}
      <span class="chip">RimWorld {app.gameVersion}</span>
      <span class="chip strong">{app.active.length} active · {app.inactive.length} inactive</span>
      {#if app.errorCount}<span class="chip err"
          ><Icon name="error" size={13} />{app.errorCount} with errors</span
        >{/if}
      {#if app.warningCount}<span class="chip warn"
          ><Icon name="alert" size={13} />{app.warningCount} with warnings</span
        >{/if}
      {#if app.duplicates}<button class="chip link" onclick={() => (showDups = true)}
          >{app.duplicates} duplicate package ids</button
        >{/if}
      <span class="spacer"></span>
      <span class="dim"
        >scanned in {app.scanMs} ms · {app.communityRules
          ? `${app.communityRules} community rules`
          : 'no community rules'}</span
      >
    {:else}
      <span class="dim">{t('No mods loaded — check Settings → Locations.')}</span>
    {/if}
  </footer>

  <div class="toasts">
    {#each toasts as t (t.id)}
      <div class="toast">
        {t.text}
        {#if t.action}
          <button
            onclick={() => {
              dismissToast(t.id)
              t.action!.run()
            }}>{t.action.label}</button
          >
        {/if}
      </div>
    {/each}
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
        <h2>{t('Unable to sort')}</h2>
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
        <button onclick={() => (app.cycles = [])}>{t('Close')}</button>
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
        >{t('Open folder')}</button
      >
      <button
        role="menuitem"
        disabled={!menu.row.published_file_id}
        onclick={() => act(() => openUrl(workshopUrl(menu!.row.published_file_id!)))}
        >{t('Open Workshop page')}</button
      >
      <button
        role="menuitem"
        disabled={!menu.row.published_file_id}
        onclick={() => act(() => openUrl(steamUrl(menu!.row.published_file_id!)))}
        >{t('Open in Steam client')}</button
      >
      <button
        role="menuitem"
        disabled={!d?.url.startsWith('http')}
        onclick={() => act(() => openUrl(d!.url))}>{t('Open mod URL')}</button
      >
      <hr />
      {#if menu.row.mod_type === 'SteamWorkshop'}
        <button role="menuitem" onclick={() => act(() => createLocalCopy(menu!.row.id))}>
          {t('Create local copy')}
        </button>
      {/if}
      {#if menu.row.mod_type === 'Git'}
        <button role="menuitem" onclick={() => act(() => updateGit(menu!.row.id))}>
          {t('Update (git pull)')}
        </button>
      {/if}
      <button role="menuitem" onclick={() => act(() => (editMetaId = menu!.row.id))}
        >{t('Color, tags & notes…')}</button
      >
      <button role="menuitem" onclick={() => act(() => (editRuleId = menu!.row.id))}
        >{t('Edit rules…')}</button
      >
      <button
        role="menuitem"
        class="danger-item"
        disabled={!d || menu.row.mod_type === 'Ludeon'}
        onclick={() => act(() => (deleting = d))}>{t('Delete mod…')}</button
      >
      <hr />
      <button role="menuitem" onclick={() => act(() => copy(menu!.row.package_id))}
        >{t('Copy package id')}</button
      >
      <button role="menuitem" onclick={() => act(() => copy(menu!.row.name))}
        >{t('Copy name')}</button
      >
      <button role="menuitem" disabled={!d} onclick={() => act(() => copy(d!.path))}
        >{t('Copy path')}</button
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
    grid-template-rows: auto auto auto auto 1fr auto;
  }

  /* Top bar */
  .topbar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.45rem 0.75rem;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    min-width: 0;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    color: var(--accent);
    margin-right: 0.15rem;
  }
  .brand strong {
    color: var(--fg);
    letter-spacing: -0.01em;
  }
  .instance {
    max-width: 12rem;
    font-weight: 500;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }
  .vsep {
    width: 1px;
    height: 1.4rem;
    background: var(--line);
    margin: 0 0.25rem;
  }
  .spacer {
    flex: 1;
  }
  .topbar button.icon {
    padding: 0.32rem 0.45rem;
  }
  .topbar button.active {
    background: var(--sel);
    border-color: var(--sel-line);
  }
  .topbar button.attn {
    color: var(--warn);
  }
  .dirty {
    display: inline-flex;
    align-items: center;
    color: var(--warn);
    font-weight: 600;
    margin-right: 0.25rem;
  }
  .dirty :global(svg) {
    margin-right: -0.2rem;
  }

  /* Banners */
  .banner {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.4rem 0.9rem;
    background: color-mix(in srgb, var(--warn) 14%, var(--panel));
    border-bottom: 1px solid color-mix(in srgb, var(--warn) 35%, var(--line));
    font-size: 0.92em;
  }
  .banner.external {
    background: color-mix(in srgb, var(--accent) 13%, var(--panel));
    border-bottom-color: color-mix(in srgb, var(--accent) 35%, var(--line));
  }
  .banner button:not(.link) {
    padding: 0.15rem 0.6rem;
  }
  .missing {
    font-size: 0.85em;
    margin-top: 0.25rem;
    word-break: break-all;
    color: var(--dim);
  }

  /* Main body */
  .body {
    display: grid;
    gap: 0;
    padding: 0.65rem 0.75rem;
    min-height: 0;
  }
  .body.single {
    grid-template-columns: 1fr;
  }
  .gutter {
    cursor: col-resize;
    touch-action: none;
    border-radius: 4px;
    transition: background 0.12s;
  }
  .gutter:hover,
  .gutter:active {
    background: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  /* Info panel */
  .info {
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 0.85rem 0.95rem;
    min-width: 0;
  }
  .info h3 {
    font-size: 1.12rem;
    margin: 0.1rem 0 0.7rem;
    line-height: 1.25;
  }
  .empty {
    display: grid;
    place-items: center;
    gap: 0.5rem;
    margin-top: 3rem;
    color: var(--dim);
    text-align: center;
  }
  .empty p {
    margin: 0;
    max-width: 22ch;
  }
  .preview {
    width: 100%;
    border-radius: var(--radius-sm);
    margin-bottom: 0.7rem;
    border: 1px solid var(--line);
    display: block;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.3rem 0.75rem;
    margin: 0;
    font-size: 0.92em;
  }
  dt {
    color: var(--dim);
  }
  dd {
    margin: 0;
    word-break: break-word;
  }
  .path {
    font-size: 0.8em;
    color: var(--dim);
  }
  .desc {
    white-space: pre-wrap;
    font-size: 0.9em;
    max-height: 40vh;
    overflow: auto;
    color: color-mix(in srgb, var(--fg) 88%, var(--dim));
    border-top: 1px solid var(--line);
    padding-top: 0.7rem;
    margin-top: 0.8rem;
  }
  .note {
    white-space: pre-wrap;
    font-size: 0.88em;
    background: var(--panel-2);
    border-left: 3px solid var(--accent);
    padding: 0.4rem 0.6rem;
    margin: 0.6rem 0;
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
  }
  .warnings {
    margin: 0.7rem 0;
    padding: 0.5rem 0.7rem 0.5rem 1.6rem;
    font-size: 0.9em;
    color: var(--warn);
    background: color-mix(in srgb, var(--warn) 9%, var(--panel));
    border: 1px solid color-mix(in srgb, var(--warn) 28%, var(--line));
    border-radius: var(--radius-sm);
  }
  .warnings li.err {
    color: var(--err);
  }
  .bad,
  .err {
    color: var(--err);
  }
  .warn {
    color: var(--warn);
  }
  .dim {
    color: var(--dim);
  }

  /* Status bar */
  .status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.3rem 0.75rem;
    background: var(--panel);
    border-top: 1px solid var(--line);
    font-size: 0.88em;
    min-height: 2rem;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.1rem 0.55rem;
    border-radius: 999px;
    background: var(--panel-2);
    border: 1px solid var(--line);
    color: var(--dim);
  }
  .chip.strong {
    color: var(--fg);
  }
  .chip.err {
    color: var(--err);
    border-color: color-mix(in srgb, var(--err) 40%, var(--line));
    background: color-mix(in srgb, var(--err) 10%, var(--panel-2));
  }
  .chip.warn {
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 40%, var(--line));
    background: color-mix(in srgb, var(--warn) 10%, var(--panel-2));
  }
  button.chip {
    cursor: pointer;
    padding: 0.1rem 0.55rem;
  }
  button.chip:hover {
    color: var(--fg);
  }
  .job {
    max-width: 50ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  progress {
    width: 12rem;
    height: 0.5rem;
    accent-color: var(--accent);
  }
  button.small {
    padding: 0.05rem 0.5rem;
  }

  /* Menus */
  .dropdown {
    position: relative;
  }
  .menu {
    position: fixed;
    z-index: 30;
    min-width: 13rem;
    background: var(--panel-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    padding: 0.3rem;
    display: grid;
    box-shadow: var(--shadow);
    animation: rise 0.12s ease-out;
  }
  .menu button {
    text-align: left;
    border: 0;
    background: none;
    padding: 0.38rem 0.65rem;
    border-radius: 5px;
  }
  .menu button:hover:not(:disabled) {
    background: var(--hover);
  }
  .menu hr {
    border: 0;
    border-top: 1px solid var(--line);
    margin: 0.25rem 0.2rem;
    width: auto;
  }
  .menu .danger-item:not(:disabled) {
    color: var(--err);
  }

  /* Sort-cycle dialog */
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(5, 8, 14, 0.6);
    backdrop-filter: blur(2px);
    display: grid;
    place-items: center;
    z-index: 40;
    animation: fade 0.12s ease-out;
  }
  .dialog {
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 12px;
    box-shadow: var(--shadow);
    padding: 1.1rem 1.3rem;
    max-width: 80vw;
    max-height: 80vh;
    overflow: auto;
  }
  .dialog h2 {
    margin-bottom: 0.4rem;
  }
  .cycle {
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    padding: 0.55rem 0.75rem;
    margin: 0.5rem 0;
    font-size: 0.88em;
    background: var(--panel-2);
  }
  .cycle ul {
    margin: 0.3rem 0 0;
    padding-left: 1.1rem;
  }
</style>
