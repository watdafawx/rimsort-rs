<script module lang="ts">
  // Shared across card instances (module scope) so the cache survives hover-in/out.
  const cardCache = new Map<string, ModDetail>()
</script>

<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core'
  import type { ModDetail, ModRow, Warning } from '../bindings'
  import { call, commands } from './ipc.svelte'
  import { t, T } from './i18n.svelte'
  import Icon from './Icon.svelte'
  import { ago, prefs } from './prefs.svelte'
  import { app, describe, isError } from './store.svelte'

  let {
    row,
    rect,
    listId,
    warnings = [],
  }: { row: ModRow; rect: DOMRect; listId: 'active' | 'inactive'; warnings?: Warning[] } = $props()

  const SOURCE: Record<string, string> = {
    Ludeon: T('RimWorld / official expansion'),
    SteamWorkshop: T('Steam Workshop mod'),
    Local: T('Local mod'),
    Git: T('Local mod'),
    SteamCmd: T('Local mod'),
    Unknown: T('Unknown source'),
  }

  // Details are fetched on demand and cached, so hovering back and forth costs nothing.
  const cache = cardCache
  let detail = $state<ModDetail | null>(null)
  $effect(() => {
    const id = row.id
    detail = cache.get(id) ?? null
    if (detail) return
    call(commands.getMod(id)).then(
      (d) => {
        if (d) {
          if (cache.size > 200) cache.delete(cache.keys().next().value!)
          cache.set(id, d)
        }
        if (row.id === id) detail = d
      },
      () => {},
    )
  })

  const errors = $derived(warnings.filter(isError))
  const others = $derived(warnings.filter((w) => !isError(w)))
  const inSave = $derived(app.save ? app.save.package_ids.includes(row.package_id) : null)
  const isNew = $derived(prefs.saveMarks && listId === 'active' && inSave === false)
  const recent = $derived(
    prefs.recentDays > 0 && row.modified > Date.now() / 1000 - prefs.recentDays * 86400,
  )
  const rel = $derived(
    detail
      ? [
          [T('depends on'), detail.dependencies.length],
          [T('load after'), detail.load_after.length],
          [T('load before'), detail.load_before.length],
          [T('incompatible'), detail.incompatible_with.length],
        ].filter(([, n]) => (n as number) > 0)
      : [],
  )

  // Beside the row (right if there is room, else left), vertically level with it and kept on screen.
  let el = $state<HTMLDivElement>()
  const pos = $derived.by(() => {
    const w = 340
    const h = el?.offsetHeight ?? 260
    const left =
      rect.right + 12 + w < window.innerWidth ? rect.right + 12 : Math.max(8, rect.left - w - 12)
    const top = Math.max(8, Math.min(window.innerHeight - h - 8, rect.top - 8))
    return { left, top }
  })
</script>

<div class="card" bind:this={el} style:left="{pos.left}px" style:top="{pos.top}px" role="tooltip">
  {#if detail?.preview}
    <img class="prev" src={convertFileSrc(detail.preview)} alt="" />
  {/if}
  <div class="body">
    <div class="head">
      <strong>{row.name}</strong>
      <span class="chips">
        <span class="chip">{t(SOURCE[row.mod_type] ?? SOURCE.Unknown)}</span>
        <span class="chip">{row.csharp ? 'C#' : 'XML'}</span>
        {#if isNew}<span class="chip new">{t('Not in latest save')}</span>{/if}
        {#if recent}<span class="chip">{t('Changed {when}', { when: ago(row.modified) })}</span
          >{/if}
      </span>
    </div>

    {#if !row.valid}
      <p class="line err">
        <Icon name="error" size={14} />{detail?.invalid_reason ?? t('Invalid mod')}
      </p>
    {/if}
    {#each errors as w, i (i)}
      <p class="line err"><Icon name="error" size={14} />{describe(w)}</p>
    {/each}
    {#each others as w, i (i)}
      <p class="line warn"><Icon name="alert" size={14} />{describe(w)}</p>
    {/each}
    {#if row.unsupported_version && !others.some((w) => w.kind === 'VersionMismatch')}
      <p class="line warn">
        <Icon name="alert" size={14} />{t('Does not list support for this game version')}
      </p>
    {/if}

    <dl>
      <dt>{t('Package')}</dt>
      <dd class="mono">{row.package_id}</dd>
      {#if row.authors}<dt>{t('Authors')}</dt>
        <dd>{row.authors}</dd>{/if}
      {#if detail?.mod_version}<dt>{t('Version')}</dt>
        <dd>{detail.mod_version}</dd>{/if}
      {#if detail?.supported_versions.length}<dt>{t('Supports')}</dt>
        <dd>{detail.supported_versions.join(', ')}</dd>{/if}
      {#if detail?.workshop_updated}<dt>{t('Updated')}</dt>
        <dd>{new Date(detail.workshop_updated * 1000).toLocaleDateString()}</dd>{/if}
      {#if detail?.added}<dt>{t('Added')}</dt>
        <dd>{new Date(detail.added * 1000).toLocaleDateString()}</dd>{/if}
    </dl>

    {#if rel.length}
      <p class="rel">{rel.map(([k, n]) => `${n} ${t(k as string)}`).join(' · ')}</p>
    {/if}
    {#if row.tags.length}
      <p class="tags">
        {#each row.tags as g (g)}<span class="chip">{g}</span>{/each}
      </p>
    {/if}
    {#if detail?.note}<p class="note">📝 {detail.note}</p>{/if}
  </div>
</div>

<style>
  .card {
    position: fixed;
    z-index: 85;
    width: 340px;
    max-height: calc(100vh - 16px);
    overflow: hidden;
    background: var(--panel-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    pointer-events: none;
    animation: rise 0.14s ease-out;
  }
  .prev {
    display: block;
    width: 100%;
    max-height: 150px;
    object-fit: cover;
    border-bottom: 1px solid var(--line);
  }
  .body {
    padding: 0.6rem 0.75rem 0.7rem;
    display: grid;
    gap: 0.4rem;
  }
  .head {
    display: grid;
    gap: 0.3rem;
  }
  .chips,
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin: 0;
  }
  .chip {
    font-size: 0.72rem;
    padding: 0 0.4rem;
    border-radius: 999px;
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--dim);
  }
  .chip.new {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 50%, var(--line));
  }
  .line {
    display: flex;
    gap: 0.4rem;
    align-items: flex-start;
    margin: 0;
    font-size: 0.9em;
    line-height: 1.3;
  }
  .line :global(svg) {
    margin-top: 0.15em;
  }
  .err {
    color: var(--err);
  }
  .warn {
    color: var(--warn);
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.15rem 0.7rem;
    margin: 0;
    font-size: 0.88em;
  }
  dt {
    color: var(--dim);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: ui-monospace, 'Cascadia Mono', Consolas, monospace;
    font-size: 0.95em;
  }
  .rel,
  .note {
    margin: 0;
    font-size: 0.85em;
    color: var(--dim);
  }
</style>
