<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core'
  import type { ModDetail, ModRow, Warning, WorkshopMeta } from '../bindings'
  import { cachedDetail, cachedSteam, prefetch } from './hovercache'
  import { t, T } from './i18n.svelte'
  import Icon from './Icon.svelte'
  import steamIcon from '../assets/mod/steam_icon.png'
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

  // Filled by `prefetch` on mouse-enter, so both are normally there on the first frame.
  let detail = $state<ModDetail | null>(null)
  let steam = $state<WorkshopMeta | null>(null)
  $effect(() => {
    const r = row
    detail = cachedDetail(r.id)
    steam = cachedSteam(r.published_file_id)
    if (detail && (steam || !r.published_file_id)) return
    prefetch(r).then(() => {
      if (row.id === r.id) {
        detail = cachedDetail(r.id)
        steam = cachedSteam(r.published_file_id)
      }
    })
  })
  const fmtCount = (n: number) =>
    n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(1)}K` : String(n)
  /** Steam description without BBCode, cut to a couple of lines. */
  const steamBlurb = $derived(
    (steam?.description ?? '')
      .replace(/\[img\][^[]*\[\/img\]/gi, ' ')
      .replace(/https?:\/\/\S+/g, ' ')
      .replace(/\[\/?[a-z0-9*]+(=[^\]]*)?\]/gi, ' ')
      .replace(/\s+/g, ' ')
      .trim()
      .slice(0, 190),
  )
  const steamNewer = $derived(
    !!steam &&
      !steam.removed &&
      !!detail?.workshop_updated &&
      (steam.time_updated ?? 0) > detail.workshop_updated + 3600,
  )

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
  let h = $state(260)
  const pos = $derived.by(() => {
    const w = 340
    const left =
      rect.right + 12 + w < window.innerWidth ? rect.right + 12 : Math.max(8, rect.left - w - 12)
    const top = Math.max(8, Math.min(window.innerHeight - h - 8, rect.top - 8))
    return { left, top }
  })
</script>

<div
  class="card"
  bind:offsetHeight={h}
  style:left="{pos.left}px"
  style:top="{pos.top}px"
  role="tooltip"
>
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
    {#if steam?.removed}
      <p class="line err">
        <Icon name="error" size={14} />{t('No longer available on the Steam Workshop')}
      </p>
    {:else if steamNewer}
      <p class="line warn">
        <Icon name="alert" size={14} />{t('A newer version is on the Workshop')}
      </p>
    {/if}
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

    {#if steam && !steam.removed}
      <p class="steam">
        <img src={steamIcon} alt="" width="14" height="14" />
        {t('{n} subscribers', { n: fmtCount(steam.subscriptions ?? 0) })}
        {#if steam.favorited}· {t('{n} favorites', { n: fmtCount(steam.favorited ?? 0) })}{/if}
      </p>
      {#if steam.tags?.length}
        <p class="tags">
          {#each (steam.tags ?? []).filter((g) => g !== 'Mod').slice(0, 8) as g (g)}<span
              class="chip">{g}</span
            >{/each}
        </p>
      {/if}
      {#if steamBlurb}<p class="blurb">
          {steamBlurb}{(steam.description ?? '').length > 190 ? '…' : ''}
        </p>{/if}
    {/if}
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
    max-height: 110px;
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
    grid-template-columns: max-content minmax(0, 1fr);
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
  .steam {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    margin: 0;
    font-size: 0.85em;
    color: var(--dim);
  }
  .blurb {
    margin: 0;
    font-size: 0.83em;
    color: var(--dim);
    line-height: 1.35;
    border-left: 2px solid var(--line-strong);
    padding-left: 0.5rem;
  }
  .rel,
  .note {
    margin: 0;
    font-size: 0.85em;
    color: var(--dim);
  }
</style>
