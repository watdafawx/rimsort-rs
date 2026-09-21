<script module lang="ts">
  // Shared between the two list instances so a drag can cross lists.
  let dragging: { from: string; ids: string[] } | null = null
</script>

<script lang="ts">
  import type { ModRow, Warning } from '../bindings'
  import Icon from './Icon.svelte'
  import { t } from './i18n.svelte'
  import { describe, inactiveSort, isError, setInactiveSort, type SortKey } from './store.svelte'

  const ROW = 28

  let {
    title,
    listId,
    rows,
    warnings = {},
    onmove,
    onactivate,
    onselect,
    oncontext,
  }: {
    title: string
    listId: 'active' | 'inactive'
    rows: ModRow[]
    /** Warnings by mod id (active list only). */
    warnings?: Record<string, Warning[]>
    /** Rows dropped on this list; `beforeId` is the row they were dropped above (null = end). */
    onmove: (from: string, ids: string[], beforeId: string | null) => void
    /** Double-click / Enter / Delete on the selection. */
    onactivate: (ids: string[]) => void
    onselect: (row: ModRow) => void
    /** Right-click: the clicked row, the (possibly updated) selection, and screen position. */
    oncontext?: (row: ModRow, ids: string[], x: number, y: number) => void
  } = $props()

  let query = $state('')
  let onlyWarn = $state(false)
  let typeFilter = $state('')
  let scroller: HTMLDivElement
  let scrollTop = $state(0)
  let viewH = $state(400)
  let sel = $state(new Set<string>())
  let anchor = -1
  let cursor = -1
  let dropAt = $state<number | null>(null)

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase()
    const typed = typeFilter ? rows.filter((r) => r.mod_type === typeFilter) : rows
    const base = onlyWarn ? typed.filter((r) => warnings[r.id] || r.unsupported_version) : typed
    if (!q) return base
    return base.filter(
      (r) =>
        r.name.toLowerCase().includes(q) ||
        r.authors.toLowerCase().includes(q) ||
        r.package_id.includes(q) ||
        r.tags.some((t) => t.toLowerCase().includes(q)) ||
        r.published_file_id === q,
    )
  })
  const start = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 6))
  const end = $derived(Math.min(shown.length, Math.ceil((scrollTop + viewH) / ROW) + 6))
  const visible = $derived(shown.slice(start, end))

  const TYPE: Record<string, [string, string]> = {
    Ludeon: ['LUD', 'ludeon'],
    SteamWorkshop: ['WS', 'workshop'],
    Local: ['LOC', 'local'],
    SteamCmd: ['CMD', 'cmd'],
    Git: ['GIT', 'git'],
    Unknown: ['?', 'unknown'],
  }

  function rowTitle(r: ModRow): string {
    const lines = [r.name, r.package_id]
    if (r.tags.length) lines.push(`Tags: ${r.tags.join(', ')}`)
    if (r.has_note) lines.push('(has a note)')
    if (!r.valid) lines.push('', 'Invalid mod (no usable About.xml)')
    const w = warnings[r.id]
    if (w) lines.push('', ...w.map(describe))
    else if (r.unsupported_version) lines.push('', 'Does not list support for this game version')
    return lines.join('\n')
  }

  function selectedIds(): string[] {
    return shown.filter((r) => sel.has(r.id)).map((r) => r.id)
  }

  function ensureVisible(i: number) {
    const top = i * ROW
    if (top < scroller.scrollTop) scroller.scrollTop = top
    else if (top + ROW > scroller.scrollTop + viewH) scroller.scrollTop = top + ROW - viewH
  }

  function pick(i: number, e: { shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }) {
    const id = shown[i].id
    if (e.shiftKey && anchor >= 0) {
      const [a, b] = [Math.min(anchor, i), Math.max(anchor, i)]
      const range = shown.slice(a, b + 1).map((r) => r.id)
      sel = e.ctrlKey || e.metaKey ? new Set([...sel, ...range]) : new Set(range)
    } else if (e.ctrlKey || e.metaKey) {
      const next = new Set(sel)
      if (!next.delete(id)) next.add(id)
      sel = next
      anchor = i
    } else {
      sel = new Set([id])
      anchor = i
    }
    cursor = i
    onselect(shown[i])
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const i = Math.max(
        0,
        Math.min(shown.length - 1, (cursor < 0 ? -1 : cursor) + (e.key === 'ArrowDown' ? 1 : -1)),
      )
      if (shown.length) {
        pick(i, e)
        ensureVisible(i)
      }
    } else if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault()
      const i = e.key === 'Home' ? 0 : shown.length - 1
      if (shown.length) {
        pick(i, e)
        ensureVisible(i)
      }
    } else if (e.key === 'Enter' || e.key === 'Delete') {
      e.preventDefault()
      const ids = selectedIds()
      if (ids.length) onactivate(ids)
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
      e.preventDefault()
      sel = new Set(shown.map((r) => r.id))
    }
  }

  function dragstart(e: DragEvent, i: number) {
    if (!sel.has(shown[i].id)) pick(i, { shiftKey: false, ctrlKey: false, metaKey: false })
    dragging = { from: listId, ids: selectedIds() }
    e.dataTransfer?.setData('text/plain', `${dragging.ids.length} mods`)
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
  }

  function dragover(e: DragEvent) {
    if (!dragging) return
    e.preventDefault()
    const rect = scroller.getBoundingClientRect()
    const y = e.clientY - rect.top
    if (y < 28)
      scroller.scrollTop -= 14 // edge autoscroll
    else if (y > rect.height - 28) scroller.scrollTop += 14
    dropAt = Math.max(0, Math.min(shown.length, Math.round((y + scroller.scrollTop) / ROW)))
  }

  function drop(e: DragEvent) {
    e.preventDefault()
    const d = dragging
    const at = dropAt
    dropAt = null
    dragging = null
    if (!d || at === null) return
    if (d.from === listId && listId === 'inactive') return // order there is fixed (by name)
    onmove(d.from, d.ids, at < shown.length ? shown[at].id : null)
  }
</script>

<section class="list">
  <header>
    <div class="hrow">
      <strong>{title}</strong>
      <span class="count"
        >{shown.length !== rows.length ? `${shown.length} / ` : ''}{rows.length}</span
      >
      <span class="grow"></span>
      <select
        class="compact"
        bind:value={typeFilter}
        title={t('Filter by mod source')}
        aria-label="Filter by type"
      >
        <option value="">{t('All')}</option>
        {#each Object.entries(TYPE) as [k, [tag]] (k)}<option value={k}>{tag}</option>{/each}
      </select>
      {#if listId === 'inactive'}
        <select
          class="compact"
          title={t('Sort')}
          aria-label="Sort by"
          value={inactiveSort.key}
          onchange={(e) => setInactiveSort(e.currentTarget.value as SortKey, inactiveSort.desc)}
        >
          <option value="name">{t('Name')}</option>
          <option value="author">{t('Author')}</option>
          <option value="modified">{t('Modified')}</option>
          <option value="type">{t('Type')}</option>
        </select>
        <button
          class="dir ghost icon"
          title={inactiveSort.desc ? 'Descending' : 'Ascending'}
          aria-label="Toggle sort direction"
          onclick={() => setInactiveSort(inactiveSort.key, !inactiveSort.desc)}
          ><span class:flip={!inactiveSort.desc}><Icon name="sort" size={15} /></span></button
        >
      {/if}
      <button
        class="ghost icon toggle"
        class:on={onlyWarn}
        title={t('Show only mods with warnings')}
        aria-label="Show only mods with warnings"
        aria-pressed={onlyWarn}
        onclick={() => (onlyWarn = !onlyWarn)}><Icon name="alert" size={15} /></button
      >
    </div>
    <label class="search">
      <Icon name="search" size={14} />
      <input type="search" placeholder={t('Search name, author, package id…')} bind:value={query} />
    </label>
  </header>
  <div
    class="scroll"
    role="listbox"
    aria-label={title}
    aria-multiselectable="true"
    tabindex="0"
    bind:this={scroller}
    bind:clientHeight={viewH}
    onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
    {onkeydown}
    ondragover={dragover}
    ondragleave={() => (dropAt = null)}
    ondrop={drop}
  >
    <div class="spacer" style:height="{shown.length * ROW}px">
      {#each visible as r, k (r.id)}
        {@const i = start + k}
        {@const w = warnings[r.id]}
        {@const [tag, cls] = TYPE[r.mod_type] ?? TYPE.Unknown}
        <div
          class="row"
          class:selected={sel.has(r.id)}
          class:invalid={!r.valid}
          role="option"
          aria-selected={sel.has(r.id)}
          tabindex="-1"
          style:transform="translateY({i * ROW}px)"
          style:box-shadow={r.color ? `inset 4px 0 0 ${r.color}` : undefined}
          title={rowTitle(r)}
          draggable="true"
          onclick={(e) => pick(i, e)}
          ondblclick={() => onactivate(selectedIds())}
          oncontextmenu={(e) => {
            e.preventDefault()
            if (!sel.has(r.id)) pick(i, { shiftKey: false, ctrlKey: false, metaKey: false })
            oncontext?.(r, selectedIds(), e.clientX, e.clientY)
          }}
          onkeydown={() => {}}
          ondragstart={(e) => dragstart(e, i)}
          ondragend={() => {
            dragging = null
            dropAt = null
          }}
        >
          <span class="tag {cls}">{tag}</span>
          <span class="name">{r.name}</span>
          <span class="author">{r.authors}</span>
          {#if !r.valid}<span class="badge err" title="Invalid mod"
              ><Icon name="error" size={15} /></span
            >{:else if w}<span class="badge" class:err={w.some(isError)}
              ><Icon name={w.some(isError) ? 'error' : 'alert'} size={15} /></span
            >{:else if r.unsupported_version}<span class="badge"
              ><Icon name="alert" size={15} /></span
            >{/if}
        </div>
      {/each}
      {#if dropAt !== null}<div
          class="drop"
          style:transform="translateY({dropAt * ROW - 1}px)"
        ></div>{/if}
    </div>
    {#if !rows.length}<p class="empty">{t('Nothing here.')}</p>{/if}
  </div>
</section>

<style>
  .list {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  header {
    display: grid;
    gap: 0.4rem;
    padding: 0.55rem 0.65rem 0.5rem;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
  }
  .hrow {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    min-width: 0;
  }
  .hrow strong {
    font-size: 0.98rem;
    letter-spacing: -0.01em;
  }
  .grow {
    flex: 1;
  }
  .count {
    color: var(--dim);
    font-variant-numeric: tabular-nums;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 0 0.5rem;
    font-size: 0.85em;
  }
  select.compact {
    padding: 0.18rem 0.35rem;
    font-size: 0.88em;
    border-radius: 5px;
  }
  header button.icon {
    padding: 0.25rem 0.35rem;
  }
  .toggle.on {
    background: color-mix(in srgb, var(--warn) 18%, transparent);
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 40%, transparent);
  }
  .flip {
    display: inline-flex;
    transform: scaleY(-1);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    padding: 0 0.55rem;
    color: var(--dim);
    transition:
      border-color 0.12s,
      box-shadow 0.12s;
  }
  .search:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    padding: 0.32rem 0;
    box-shadow: none;
  }
  .search input:focus {
    box-shadow: none;
  }

  .scroll {
    position: relative;
    flex: 1;
    overflow-y: auto;
    outline: none;
  }
  .scroll:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--accent) 60%, transparent);
  }
  .spacer {
    position: relative;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 28px;
    display: flex;
    align-items: center;
    gap: 0.55rem;
    padding: 0 0.7rem;
    box-sizing: border-box;
    user-select: none;
    cursor: default;
    white-space: nowrap;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 45%, transparent);
    transition: background 0.08s;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.selected {
    background: var(--sel);
    border-bottom-color: var(--sel-line);
  }
  .row.invalid {
    opacity: 0.5;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 0 1 auto;
    font-weight: 500;
  }
  .author {
    color: var(--dim);
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1 1 0;
    min-width: 0;
    font-size: 0.86em;
  }
  .badge {
    display: inline-flex;
    color: var(--warn);
  }
  .badge.err {
    color: var(--err);
  }
  .tag {
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.02em;
    padding: 1px 0;
    border-radius: 4px;
    color: #fff;
    width: 2.1rem;
    flex: none;
    text-align: center;
    opacity: 0.92;
  }
  .ludeon {
    background: #b3861a;
  }
  .workshop {
    background: #3a78c2;
  }
  .local {
    background: #338a62;
  }
  .cmd {
    background: #7a56c8;
  }
  .git {
    background: #c26a2d;
  }
  .unknown {
    background: #718096;
  }
  .drop {
    position: absolute;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--accent);
    pointer-events: none;
    box-shadow: 0 0 6px var(--accent);
  }
  .empty {
    color: var(--dim);
    text-align: center;
    margin-top: 2.5rem;
  }
</style>
