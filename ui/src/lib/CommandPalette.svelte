<script lang="ts">
  import { dialogFocus } from './actions'
  import type { ModRow } from '../bindings'
  import { t } from './i18n.svelte'
  import Icon from './Icon.svelte'
  import { app } from './store.svelte'

  export type PaletteAction = { label: string; keys?: string; run: () => void }

  let {
    actions,
    onpickmod,
    onclose,
  }: {
    actions: PaletteAction[]
    onpickmod: (row: ModRow, list: 'active' | 'inactive') => void
    onclose: () => void
  } = $props()

  let query = $state('')
  let cursor = $state(0)
  let listEl = $state<HTMLElement>()

  /** Substring hits rank by position, in-order subsequences rank below them; -1 = no match. */
  function score(q: string, text: string): number {
    const s = text.toLowerCase()
    const i = s.indexOf(q)
    if (i >= 0) return 1000 - i
    let at = 0
    let gaps = 0
    for (const ch of q) {
      const j = s.indexOf(ch, at)
      if (j < 0) return -1
      gaps += j - at
      at = j + 1
    }
    return 500 - Math.min(gaps, 400)
  }

  type Item =
    | { kind: 'action'; action: PaletteAction }
    | { kind: 'mod'; row: ModRow; list: 'active' | 'inactive' }

  const results = $derived.by((): Item[] => {
    const q = query.trim().toLowerCase()
    const acts = actions
      .map((a) => ({ a, s: q ? score(q, a.label) : 0 }))
      .filter((x) => x.s >= 0)
      .sort((x, y) => y.s - x.s)
      .slice(0, q ? 6 : 8)
      .map((x): Item => ({ kind: 'action', action: x.a }))
    if (!q) return acts
    const mods: { item: Item; s: number }[] = []
    for (const [list, rows] of [
      ['active', app.active],
      ['inactive', app.inactive],
    ] as const) {
      for (const row of rows) {
        const s = Math.max(
          score(q, row.name),
          score(q, row.package_id) - 50,
          row.authors ? score(q, row.authors) - 100 : -1,
        )
        if (s >= 0) mods.push({ item: { kind: 'mod', row, list }, s })
      }
    }
    mods.sort((x, y) => y.s - x.s)
    return [...acts, ...mods.slice(0, 40).map((m) => m.item)]
  })

  $effect(() => {
    void query
    cursor = 0
  })
  $effect(() => {
    listEl?.querySelector('.on')?.scrollIntoView({ block: 'nearest' })
  })

  function run(item: Item | undefined) {
    if (!item) return
    onclose()
    if (item.kind === 'action') item.action.run()
    else onpickmod(item.row, item.list)
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      cursor = Math.min(results.length - 1, cursor + 1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      cursor = Math.max(0, cursor - 1)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      run(results[cursor])
    } else if (e.key === 'Escape') onclose()
  }

  const focusNow = (n: HTMLElement) => queueMicrotask(() => n.focus())
</script>

<div class="backdrop top" role="presentation" onclick={onclose}>
  <div
    class="pal"
    role="dialog"
    aria-modal="true"
    aria-label={t('Command palette')}
    tabindex="-1"
    use:dialogFocus
    onclick={(e) => e.stopPropagation()}
    onkeydown={onkey}
  >
    <label class="q">
      <Icon name="search" />
      <input
        type="text"
        placeholder={t('Type a command or a mod name…')}
        aria-label={t('Command palette')}
        bind:value={query}
        use:focusNow
        spellcheck="false"
      />
      <kbd>Esc</kbd>
    </label>
    <div class="list" bind:this={listEl} role="listbox">
      {#each results as item, i (item.kind === 'action' ? 'a' + item.action.label : 'm' + item.row.id)}
        <button
          class="it"
          class:on={i === cursor}
          role="option"
          aria-selected={i === cursor}
          onmousemove={() => (cursor = i)}
          onclick={() => run(item)}
        >
          {#if item.kind === 'action'}
            <span class="tag">{t('Action')}</span>
            <span class="name">{item.action.label}</span>
            {#if item.action.keys}<span class="keys"
                >{#each item.action.keys.split('+') as k (k)}<kbd>{k.trim()}</kbd>{/each}</span
              >{/if}
          {:else}
            <span class="tag mod">{item.list === 'active' ? t('Active') : t('Inactive')}</span>
            <span class="name">{item.row.name}</span>
            <span class="sub">{item.row.package_id}</span>
          {/if}
        </button>
      {:else}
        <p class="none">{t('No matches.')}</p>
      {/each}
    </div>
  </div>
</div>

<style>
  .backdrop.top {
    place-items: start center;
    padding-top: 12vh;
  }
  .pal {
    width: min(620px, 92vw);
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 12px;
    box-shadow: var(--shadow);
    overflow: hidden;
    animation: pop 0.14s ease-out;
  }
  .q {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.7rem 0.9rem;
    border-bottom: 1px solid var(--line);
    color: var(--dim);
  }
  .q input {
    flex: 1;
    border: 0;
    background: transparent;
    box-shadow: none;
    padding: 0;
    font-size: 1.05rem;
    color: var(--fg);
  }
  .q input:focus {
    box-shadow: none;
  }
  .list {
    max-height: min(52vh, 420px);
    overflow: auto;
    padding: 0.3rem;
  }
  .it {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    text-align: left;
    background: transparent;
    border: 0;
    padding: 0.4rem 0.55rem;
    border-radius: var(--radius-sm);
  }
  .it.on {
    background: var(--sel);
  }
  .tag {
    flex: none;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--accent);
    width: 4.2rem;
  }
  .tag.mod {
    color: var(--dim);
  }
  .name {
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    color: var(--dim);
    font-size: 0.85em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .keys {
    margin-left: auto;
    display: flex;
    gap: 0.2rem;
  }
  .none {
    margin: 0;
    padding: 0.8rem;
    color: var(--dim);
  }
</style>
