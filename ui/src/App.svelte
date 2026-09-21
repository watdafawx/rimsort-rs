<script lang="ts">
  import { dialogFocus } from './lib/actions'
  import { convertFileSrc } from '@tauri-apps/api/core'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { open as pickFile, save as pickSave } from '@tauri-apps/plugin-dialog'
  import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener'
  import { onMount, untrack } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import { initLanguage, t, T } from './lib/i18n.svelte'
  import { initTheme } from './lib/theme.svelte'
  import { initZoom, stepZoom } from './lib/zoom.svelte'
  import type {
    DependentDto,
    ExportFormat,
    FolderKind,
    ModDetail,
    ModRow,
    WorkshopMatch,
  } from './bindings'
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
  import SearchDialog from './lib/SearchDialog.svelte'
  import ToddsDialog from './lib/ToddsDialog.svelte'
  import { richText } from './lib/richtext'
  import { prefs } from './lib/prefs.svelte'
  import { tip } from './lib/tip'
  import HoverCard from './lib/HoverCard.svelte'
  import PkgList from './lib/PkgList.svelte'
  import CommandPalette, { type PaletteAction } from './lib/CommandPalette.svelte'
  import newIcon from './assets/mod/new.png'
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
    runTodds,
    steamSubscribe,
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

  /** Row the pointer is resting on (rich hover card). */
  let hover = $state<{ row: ModRow; rect: DOMRect; list: 'active' | 'inactive' } | null>(null)
  let showSettings = $state(false)
  let detail = $state<ModDetail | null>(null)
  const FOLDERS: [FolderKind, string][] = [
    ['Game', T('RimWorld folder')],
    ['Config', T('Config folder (ModsConfig.xml)')],
    ['Local', T('Local mods')],
    ['Workshop', T('Steam Workshop mods')],
    ['Data', T('RimSort-rs data')],
    ['Logs', T('RimSort-rs logs')],
  ]
  let showMissing = $state(false)
  let missingMatches = $state<WorkshopMatch[]>([])
  async function toggleMissing() {
    showMissing = !showMissing
    if (showMissing)
      missingMatches = await call(commands.workshopMatches($state.snapshot(app.missing)))
  }
  let listMenu = $state(false)
  let foldersMenu = $state(false)
  let showDeps = $state(false)
  let showDups = $state(false)
  let showDownload = $state(false)
  let showSearch = $state(false)
  let showPalette = $state(false)
  /** Warnings-only filter of the Active list (set by clicking the status chips). */
  let activeWarnOnly = $state(false)
  let focusReq = $state<{ id: string; n: number } | null>(null)
  let showTodds = $state(false)
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
    { format: 'Xml', label: T('Export as ModsConfig XML…'), file: 'ModsConfig', ext: 'xml' },
    { format: 'Json', label: T('Export as RimSort JSON…'), file: 'modlist', ext: 'json' },
    { format: 'PackageIds', label: T('Export package ids…'), file: 'modlist', ext: 'txt' },
  ]
  async function doExport(x: (typeof EXPORTS)[number]) {
    const p = await pickSave({
      defaultPath: `${x.file}.${x.ext}`,
      filters: [{ name: x.ext.toUpperCase(), extensions: [x.ext] }],
    })
    if (p) {
      await call(commands.exportModlist(p, x.format))
      toast(t('Exported'), 2500)
    }
  }
  async function copyList(format: ExportFormat, what: string) {
    await navigator.clipboard.writeText(await call(commands.exportModlistText(format)))
    toast(what === 'Report' ? t('Report copied') : t('Package ids copied'), 2000)
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
    navigator.clipboard.writeText(text).then(() => toast(t('Copied'), 1500))
  const workshopUrl = (pfid: string) =>
    `https://steamcommunity.com/sharedfiles/filedetails/?id=${pfid}`
  async function updateGit(id: string) {
    toast(t('Running git pull…'), 2000)
    const out = await call(commands.updateGitMod(id))
    toast(out.split('\n').slice(-1)[0] || t('Up to date'), 5000)
  }
  const steamUrl = (pfid: string) => `steam://url/CommunityFilePage/${pfid}`
  async function unsubscribe(row: ModRow) {
    if (
      !confirm(
        t('Unsubscribe from “{name}” in Steam? Steam will remove the mod.', { name: row.name }),
      )
    )
      return
    await steamSubscribe([row.published_file_id!], false)
  }
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
      if (app.dirty && !confirm(t('You have unsaved changes. Close without saving?')))
        e.preventDefault()
    })
    return () => un.then((f) => f())
  })

  onMount(() => {
    initLanguage()
    initTheme()
    void initZoom()
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

  /** Links in a description open in the system browser, never inside the app window. */
  function openDescLink(e: MouseEvent) {
    const a = (e.target as Element).closest('a[href]')
    if (!a) return
    e.preventDefault()
    void openUrl(a.getAttribute('href')!)
  }

  /** Folder size of the selected mod, computed lazily (null while walking the folder). */
  let folderSize = $state<number | null>(null)
  /** Installed mods by package id, so dependency names in the info panel can jump to the mod. */
  const byPackage = $derived.by(() => {
    const m = new Map<string, ModRow>()
    for (const r of [...app.active, ...app.inactive])
      if (!m.has(r.package_id)) m.set(r.package_id, r)
    return m
  })
  let dependents = $state<DependentDto[]>([])

  async function select(row: ModRow) {
    detail = await call(commands.getMod(row.id))
    dependents = []
    call(commands.getDependents(row.id)).then(
      (d) => {
        if (detail?.id === row.id) dependents = d
      },
      () => {},
    )
    folderSize = null
    const id = row.id
    call(commands.folderSize(id)).then(
      (n) => {
        if (detail?.id === id) folderSize = n
      },
      () => {},
    )
  }

  const fmtBytes = (n: number) =>
    n < 1024
      ? `${n} B`
      : n < 1024 ** 2
        ? `${(n / 1024).toFixed(0)} KB`
        : n < 1024 ** 3
          ? `${(n / 1024 ** 2).toFixed(1)} MB`
          : `${(n / 1024 ** 3).toFixed(2)} GB`

  /**
   * Mods or ModsConfig.xml changed on disk: rescan right away (keeping unsaved list edits). Waits for a
   * running scan/job and re-runs when it ends. A changed ModsConfig.xml with unsaved edits still asks first.
   */
  const promptOnly = $derived(
    !!external.changed && (!prefs.autoRefresh || (external.changed === 'config' && app.dirty)),
  )
  $effect(() => {
    const what = external.changed
    if (!what || promptOnly || !app.loaded || app.scanning || app.jobTask) return
    untrack(() => {
      toast(t('Rescanned — mods changed on disk'), 2500)
      void refresh(app.dirty)
    })
  })

  async function switchInstance(name: string) {
    if (app.dirty && !confirm(t('Discard unsaved changes?'))) return
    await call(commands.switchInstance(name))
    await loadSettings()
    await refresh()
  }

  async function doRefresh() {
    if (app.dirty && !confirm(t('Discard unsaved changes and rescan?'))) return
    await refresh()
  }

  async function trySave() {
    if (
      (await call(commands.gameRunning())) &&
      !confirm(
        t(
          'RimWorld is running. It rewrites ModsConfig.xml when it closes, which would undo this save. Save anyway?',
        ),
      )
    )
      return
    await save()
  }

  async function run() {
    if (await call(commands.gameRunning())) {
      toast(t('RimWorld is already running'), 3000)
      return
    }
    if (
      app.dirty &&
      !confirm(t('You have unsaved changes; RimWorld will use the last saved list. Launch anyway?'))
    )
      return
    const todds = await call(commands.getToddsOptions())
    if (todds.auto_before_launch) {
      toast(t('Optimizing textures before launch…'), 2500)
      // The saved options apply; a failed run is reported by the job itself and aborts the launch.
      if (
        !(await runTodds(
          {
            ...todds,
            dry_run: false,
            preset: todds.preset === 'Clean' ? 'Optimized' : todds.preset,
          },
          t('Textures optimized'),
        ))
      )
        return
    }
    await call(commands.launchGame())
  }

  const paletteActions = $derived<PaletteAction[]>([
    { label: t('Rescan mods'), keys: 'F5', run: doRefresh },
    { label: t('Sort active list'), run: sort },
    { label: t('Save load order'), keys: 'Ctrl+S', run: () => void trySave() },
    { label: t('Launch RimWorld'), run: () => void run() },
    { label: t('Undo'), keys: 'Ctrl+Z', run: undo },
    { label: t('Redo'), keys: 'Ctrl+Y', run: redo },
    { label: t('Clear active list'), run: clearActive },
    { label: t('Search in mod files…'), keys: 'Ctrl+Shift+F', run: () => (showSearch = true) },
    { label: t('Missing dependencies'), run: () => (showDeps = true) },
    { label: t('Duplicate mods'), run: () => (showDups = true) },
    { label: t('Download mods…'), run: () => (showDownload = true) },
    { label: t('Optimize textures…'), run: () => (showTodds = true) },
    { label: t('Restore from backup…'), run: () => (showBackups = true) },
    { label: t('Import list…'), run: doImport },
    { label: t('Player.log'), run: () => (view = view === 'log' ? 'mods' : 'log') },
    { label: t('Settings'), run: () => (showSettings = true) },
  ])

  function pickFromPalette(row: ModRow) {
    view = 'mods'
    focusReq = { id: row.id, n: (focusReq?.n ?? 0) + 1 }
  }

  const scan = $derived(tasks[app.scanTask])
  /** Package ids of the latest save game (null = no save, so no "new" markers). */
  const saveIds = $derived(app.save ? new Set(app.save.package_ids) : null)
  const newCount = $derived(
    saveIds ? app.active.filter((r) => !saveIds.has(r.package_id)).length : 0,
  )
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
    foldersMenu = false
  }}
  onkeydown={(e) => {
    if (e.key === 'Escape') {
      menu = null
      listMenu = false
      foldersMenu = false
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault()
      showPalette = !showPalette
      return
    }
    if (e.ctrlKey || e.metaKey) {
      const dir = { '+': 1, '=': 1, '-': -1, _: -1, '0': 0 }[e.key]
      if (dir !== undefined) {
        e.preventDefault()
        void stepZoom(dir as -1 | 0 | 1)
      }
    }
    if ((e.ctrlKey || e.metaKey) && !(e.target instanceof HTMLInputElement)) {
      const k = e.key.toLowerCase()
      if (k === 'z' && !e.shiftKey) undo()
      else if (k === 'y' || (k === 'z' && e.shiftKey)) redo()
      else if (k === 'f' && e.shiftKey) {
        e.preventDefault()
        showSearch = true
      } else if (k === 's') {
        e.preventDefault()
        if (app.dirty) void trySave()
      }
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
        use:tip={{
          title: T('Instance'),
          text: T('Separate game folders, mod folders and settings. Add or edit them in Settings.'),
        }}
        value={app.settings.current_instance}
        onchange={(e) => switchInstance(e.currentTarget.value)}
      >
        {#each app.settings.instances as i (i.name)}<option>{i.name}</option>{/each}
      </select>
    {/if}
    <span class="vsep"></span>
    <div class="group">
      <button
        id="refresh"
        onclick={doRefresh}
        disabled={app.scanning}
        use:tip={{
          title: T('Rescan mods'),
          text: T('Reads every mod folder and ModsConfig.xml again.'),
          keys: 'F5',
        }}
      >
        <Icon name="refresh" />{t('Refresh')}
      </button>
      <button
        id="sort"
        onclick={sort}
        disabled={!app.loaded}
        use:tip={{
          title: T('Sort active list'),
          text: T(
            'Orders the active mods by their load-order rules (About.xml, community rules, your rules). Nothing is written until you press Save.',
          ),
        }}
      >
        <Icon name="sort" />{t('Sort')}
      </button>
      <button
        id="save"
        class="primary"
        onclick={trySave}
        disabled={!app.loaded}
        use:tip={{
          title: T('Save load order'),
          text: T('Writes ModsConfig.xml and keeps a timestamped backup of the previous file.'),
          keys: 'Ctrl+S',
        }}
      >
        <Icon name="save" />{t('Save')}
      </button>
      <button
        id="run"
        onclick={run}
        disabled={!app.loaded}
        use:tip={{
          title: T('Launch RimWorld'),
          text: T('Starts the game with the last saved load order. Unsaved changes are not used.'),
        }}
      >
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
            foldersMenu = false
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
            <button role="menuitem" onclick={() => (showSearch = true)}
              >{t('Search in mod files…')}</button
            >
            <button role="menuitem" onclick={() => (showTodds = true)}
              >{t('Optimize textures…')}</button
            >
            <hr />
            {#each EXPORTS as x (x.format)}
              <button role="menuitem" onclick={() => doExport(x)}>{t(x.label)}</button>
            {/each}
            <hr />
            <button role="menuitem" onclick={() => copyList('Report', 'Report')}
              >{t('Copy report')}</button
            >
            <button role="menuitem" onclick={() => copyList('PackageIds', 'Package ids')}>
              {t('Copy package ids')}
            </button>
          </div>
        {/if}
      </div>
      <button
        id="undo"
        class="ghost icon"
        onclick={undo}
        disabled={!app.undoDepth}
        use:tip={{
          title: T('Undo'),
          text: T('Reverts the last change to the mod lists.'),
          keys: 'Ctrl+Z',
        }}
        aria-label="Undo"
      >
        <Icon name="undo" />
      </button>
      <button
        id="redo"
        class="ghost icon"
        onclick={redo}
        disabled={!app.redoDepth}
        use:tip={{
          title: T('Redo'),
          text: T('Re-applies the change you just undid.'),
          keys: 'Ctrl+Y',
        }}
        aria-label="Redo"
      >
        <Icon name="redo" />
      </button>
      <button
        id="clear"
        class="ghost"
        onclick={clearActive}
        disabled={!app.loaded}
        use:tip={{
          title: T('Clear active list'),
          text: T(
            'Disables every mod except the base game and official expansions. You can undo it.',
          ),
        }}
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
          use:tip={{
            title: T('Missing dependencies'),
            text: T(
              'Required mods that are not in your active list. Click to enable or download them.',
            ),
          }}
        >
          <Icon name="link" />{app.missingDeps} missing
        </button>
      {/if}
      <button
        id="logtab"
        class="ghost"
        class:active={view === 'log'}
        onclick={() => (view = view === 'log' ? 'mods' : 'log')}
        use:tip={{
          title: T('Player.log'),
          text: T("RimWorld's log from its last run, with errors and warnings highlighted."),
        }}
      >
        <Icon name="log" />Log
      </button>
    </div>
    <span class="spacer"></span>
    <div class="dropdown">
      <button
        id="foldersmenu"
        class="ghost"
        aria-haspopup="menu"
        aria-expanded={foldersMenu}
        onclick={(e) => {
          e.stopPropagation()
          listMenu = false
          foldersMenu = !foldersMenu
        }}><Icon name="folder" />{t('Folders')}<Icon name="chevron" size={14} /></button
      >
      {#if foldersMenu}
        <div
          class="menu"
          role="menu"
          style:position="absolute"
          style:top="calc(100% + 4px)"
          style:right="0"
        >
          {#each FOLDERS as [kind, label] (kind)}
            <button role="menuitem" onclick={() => call(commands.openFolder(kind))}
              >{t(label)}</button
            >
          {/each}
        </div>
      {/if}
    </div>
    {#if app.dirty}
      <span
        class="dirty"
        use:tip={{
          title: T('Unsaved changes'),
          text: T('The lists differ from ModsConfig.xml. Press Save to write them.'),
          keys: 'Ctrl+S',
        }}><Icon name="dot" size={22} />{t('unsaved')}</span
      >
    {/if}
    <button id="settings" class="ghost" onclick={() => (showSettings = true)}>
      <Icon name="settings" />{t('Settings')}
    </button>
  </header>

  {#if promptOnly}
    <div class="banner external">
      {external.changed === 'config'
        ? t('ModsConfig.xml was changed outside RimSort-rs.')
        : t('Mods were added, removed or changed on disk.')}
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
      <button class="link" onclick={toggleMissing}>
        ⚠ {app.missing.length === 1
          ? t('{n} mod in ModsConfig.xml is not installed', { n: 1 })
          : t('{n} mods in ModsConfig.xml are not installed', { n: app.missing.length })}
      </button>
      {#if showMissing}
        <ul class="missing">
          {#each app.missing as pid (pid)}
            {@const m = missingMatches.find((x) => x.package_id === pid)}
            <li>
              <code>{pid}</code>
              {#if m}
                <span class="dim">{m.name}</span>
                <button
                  class="link"
                  onclick={() =>
                    openUrl(
                      `https://steamcommunity.com/sharedfiles/filedetails/?id=${m.workshop_id}`,
                    )}>{t('Workshop page')}</button
                >
                <button class="link" onclick={() => downloadMods([m.workshop_id])}
                  >{t('Download')}</button
                >
                <button class="link" onclick={() => steamSubscribe([m.workshop_id], true)}
                  >{t('Subscribe')}</button
                >
              {:else}
                <span class="dim">{t('no unambiguous Workshop match')}</span>
              {/if}
            </li>
          {/each}
        </ul>
        <p class="dim note">{t('Saving drops these from ModsConfig.xml.')}</p>
      {/if}
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
          {#if detail.added}<dt title={t('When this mod folder first appeared on this PC')}>
              {t('Added')}
            </dt>
            <dd>{new Date(detail.added * 1000).toLocaleDateString()}</dd>{/if}
          {#if folderSize != null}<dt>{t('Size')}</dt>
            <dd>{fmtBytes(folderSize)}</dd>{/if}
          {#if detail.workshop_updated}<dt>{t('Updated')}</dt>
            <dd>{new Date(detail.workshop_updated * 1000).toLocaleDateString()}</dd>{/if}
          {#if detail.startup_ms != null}<dt title="Load time measured by the Loading Progress mod">
              Startup
            </dt>
            <dd>
              {Math.round(detail.startup_ms)} ms{detail.startup_off_thread_ms
                ? ` (+${Math.round(detail.startup_off_thread_ms)} ms off-thread)`
                : ''}
            </dd>{/if}
          {#if detail.mod_version}<dt>{t('Version')}</dt>
            <dd>{detail.mod_version}</dd>{/if}
          <dt>{t('Path')}</dt>
          <dd class="path">{detail.path}</dd>
          {#if detail.dependencies.length}<dt>{t('Depends on')}</dt>
            <dd>
              <PkgList ids={detail.dependencies} lookup={byPackage} onjump={pickFromPalette} />
            </dd>{/if}
          {#if dependents.length}<dt>{t('Required by')}</dt>
            <dd>
              <PkgList
                ids={dependents.map((d) => d.package_id)}
                lookup={byPackage}
                onjump={pickFromPalette}
              />
            </dd>{/if}
          {#if detail.load_after.length}<dt>{t('Load after')}</dt>
            <dd>
              <PkgList ids={detail.load_after} lookup={byPackage} onjump={pickFromPalette} />
            </dd>{/if}
          {#if detail.load_before.length}<dt>{t('Load before')}</dt>
            <dd>
              <PkgList ids={detail.load_before} lookup={byPackage} onjump={pickFromPalette} />
            </dd>{/if}
          {#if detail.incompatible_with.length}<dt>{t('Incompatible')}</dt>
            <dd>
              <PkgList ids={detail.incompatible_with} lookup={byPackage} onjump={pickFromPalette} />
            </dd>{/if}
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
        <div class="desc" role="presentation" onclick={openDescLink}>
          <!-- eslint-disable-next-line svelte/no-at-html-tags -- richText() escapes everything and re-enables only a tag whitelist -->
          {@html richText(detail.description)}
        </div>
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
      {saveIds}
      focus={focusReq}
      onhover={(row, rect) => (hover = { row, rect, list: 'inactive' })}
      onhoverend={() => (hover = null)}
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
      bind:onlyWarn={activeWarnOnly}
      {saveIds}
      focus={focusReq}
      onhover={(row, rect) => (hover = { row, rect, list: 'active' })}
      onhoverend={() => (hover = null)}
      onmove={dropInto('active')}
      onactivate={(ids) => disable(ids)}
      onselect={select}
      oncontext={(r, ids, x, y) => openMenu('active', r, ids, x, y)}
    />
  </main>

  {#if hover && !menu}
    <HoverCard
      row={hover.row}
      rect={hover.rect}
      listId={hover.list}
      warnings={hover.list === 'active' ? app.warnings[hover.row.id] : []}
    />
  {/if}

  <footer class="status">
    {#if app.jobTask && tasks[app.jobTask]}
      {@const j = tasks[app.jobTask]}
      <Icon name="download" size={14} />
      <span class="job">{j.msg || t('Working…')}</span>
      <progress max={j.total || 1} value={j.done}></progress>
      <button class="ghost small" onclick={() => call(commands.cancelTask(j.id))}
        >{t('Cancel')}</button
      >
    {:else if app.scanning}
      <Icon name="refresh" size={14} />
      <span
        >{t('Scanning… {done} / {total}', {
          done: scan?.done ?? 0,
          total: scan?.total ?? '?',
        })}</span
      >
      <progress max={scan?.total || 1} value={scan?.done ?? 0}></progress>
    {:else if app.loaded}
      <span class="chip">{t('RimWorld {version}', { version: app.gameVersion })}</span>
      <span class="chip strong"
        >{t('{active} active · {inactive} inactive', {
          active: app.active.length,
          inactive: app.inactive.length,
        })}</span
      >
      {#if app.errorCount}<button
          class="chip err"
          onclick={() => (activeWarnOnly = !activeWarnOnly)}
          use:tip={{
            title: T('Mods with errors'),
            text: T(
              'Missing required mods or incompatible mods in your active list. Click to filter the Active list; hover a mod to see why.',
            ),
          }}><Icon name="error" size={13} />{t('{n} with errors', { n: app.errorCount })}</button
        >{/if}
      {#if app.warningCount}<button
          class="chip warn"
          onclick={() => (activeWarnOnly = !activeWarnOnly)}
          use:tip={{
            title: T('Mods with warnings'),
            text: T(
              'Load-order problems, version mismatches or available replacements. Click to filter the Active list; hover a mod for details.',
            ),
          }}
          ><Icon name="alert" size={13} />{t('{n} with warnings', { n: app.warningCount })}</button
        >{/if}
      {#if saveIds && prefs.saveMarks && newCount}<span
          class="chip"
          use:tip={{
            title: T('New since your latest save'),
            text: T(
              'Active mods the newest save game was not made with. Adding mods to a running colony can break it.',
            ),
          }}
          ><img src={newIcon} alt="" width="16" height="16" />{t('{n} new', { n: newCount })}</span
        >{/if}
      {#if app.duplicates}<button
          class="chip link"
          use:tip={{
            title: T('Duplicate package ids'),
            text: T(
              'Several installed mods share a package id, but RimWorld can only use one. Click to choose.',
            ),
          }}
          onclick={() => (showDups = true)}
          >{t('{n} duplicate package ids', { n: app.duplicates })}</button
        >{/if}
      {#if app.syncTask && tasks[app.syncTask]}
        {@const sy = tasks[app.syncTask]}
        <span
          class="chip"
          use:tip={{
            title: T('Fetching Steam details'),
            text: T(
              'Reading subscriber counts, tags and removal status for your Workshop mods, slowly in the background. Results are cached.',
            ),
          }}
          ><Icon name="download" size={13} />{t('Steam details {done}/{total}', {
            done: sy.done,
            total: sy.total,
          })}</span
        >
      {/if}
      <span class="spacer"></span>
      <span class="dim"
        >{app.communityRules
          ? t('scanned in {ms} ms · {rules} community rules', {
              ms: app.scanMs,
              rules: app.communityRules,
            })
          : t('scanned in {ms} ms · no community rules', { ms: app.scanMs })}</span
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
        use:dialogFocus
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.key === 'Escape' && (app.cycles = [])}
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
        <button
          role="menuitem"
          disabled={!menu.row.published_file_id}
          onclick={() => act(() => unsubscribe(menu!.row))}>{t('Unsubscribe in Steam…')}</button
        >
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
  {#if showPalette}<CommandPalette
      actions={paletteActions}
      onpickmod={pickFromPalette}
      onclose={() => (showPalette = false)}
    />{/if}
  {#if showSearch}<SearchDialog onclose={() => (showSearch = false)} />{/if}
  {#if showTodds}<ToddsDialog onclose={() => (showTodds = false)} />{/if}

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
    flex-wrap: wrap; /* large interface sizes wrap the toolbar instead of clipping it */
    gap: 0.35rem 0.5rem;
    padding: 0.45rem 0.75rem;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    min-width: 0;
  }
  .brand {
    white-space: nowrap;
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
    flex-wrap: wrap;
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
    list-style: none;
    margin: 0.35rem 0 0;
    padding: 0;
    display: grid;
    gap: 0.2rem;
    font-size: 0.9em;
    max-height: 9rem;
    overflow: auto;
  }
  .missing li {
    display: flex;
    align-items: baseline;
    gap: 0.7rem;
  }
  .note {
    margin: 0.3rem 0 0;
    font-size: 0.85em;
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
  .desc :global(a) {
    color: var(--accent);
    text-decoration: none;
  }
  .desc :global(a:hover) {
    text-decoration: underline;
  }
  .desc :global(ul),
  .desc :global(ol) {
    margin: 0.2rem 0;
    padding-left: 1.3rem;
    white-space: normal;
  }
  .desc :global(blockquote) {
    margin: 0.3rem 0;
    padding: 0.1rem 0.7rem;
    border-left: 3px solid var(--line-strong);
    color: var(--dim);
  }
  .desc :global(.h) {
    display: block;
    font-size: 1.05em;
    margin-top: 0.3rem;
  }
  .desc :global(.h1) {
    font-size: 1.2em;
  }
  .desc :global(hr) {
    border: 0;
    border-top: 1px solid var(--line);
  }
  .desc {
    white-space: pre-wrap;
    font-size: 0.9em;
    max-height: 40vh;
    overflow: auto;
    color: color-mix(in srgb, var(--fg) 88%, var(--dim));
    border-top: 1px solid var(--line);
    padding-top: 0.7rem;
    overflow-wrap: anywhere;
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

  .dialog {
    max-width: 80vw;
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
