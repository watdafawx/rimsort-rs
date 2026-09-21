<script lang="ts">
  import { dialogFocus } from './actions'
  import { t } from './i18n.svelte'
  import { revealItemInDir } from '@tauri-apps/plugin-opener'
  import type { SearchResult } from '../bindings'
  import { call, commands } from './ipc.svelte'
  import Icon from './Icon.svelte'

  const focusNow = (n: HTMLElement) => queueMicrotask(() => n.focus())

  let { onclose }: { onclose: () => void } = $props()

  let text = $state('')
  let regex = $state(false)
  let caseSensitive = $state(false)
  let fileNames = $state(false)
  let exts = $state('xml, txt, json, cs')
  let busy = $state(false)
  let result = $state<SearchResult | null>(null)

  async function run() {
    if (!text || busy) return
    busy = true
    try {
      result = await call(
        commands.searchMods({
          text,
          regex,
          case_sensitive: caseSensitive,
          extensions: exts.split(/[\s,;]+/).filter(Boolean),
          file_names: fileNames,
        }),
      )
    } catch {
      /* call() already toasted the error (e.g. an invalid regex) */
    } finally {
      busy = false
    }
  }

  /** Hits grouped by mod, keeping the backend's (name-sorted) order. */
  const groups = $derived.by(() => {
    const out: { id: string; name: string; hits: SearchResult['hits'] }[] = []
    for (const h of result?.hits ?? []) {
      const last = out[out.length - 1]
      if (last && last.id === h.mod_id) last.hits.push(h)
      else out.push({ id: h.mod_id, name: h.mod_name, hits: [h] })
    }
    return out
  })
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={t('Search in mods')}
    tabindex="-1"
    use:dialogFocus
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>{t('Search in mods')}</h2>
    <form
      onsubmit={(e) => {
        e.preventDefault()
        void run()
      }}
    >
      <div class="query">
        <input
          type="search"
          placeholder={t('Text or pattern to find in mod files…')}
          aria-label={t('Search text')}
          bind:value={text}
          use:focusNow
        />
        <button class="primary" type="submit" disabled={!text || busy}>
          <Icon name="search" />{busy ? t('Searching…') : t('Search')}
        </button>
      </div>
      <div class="opts">
        <label class="check"><input type="checkbox" bind:checked={regex} /> {t('Regex')}</label>
        <label class="check"
          ><input type="checkbox" bind:checked={caseSensitive} /> {t('Match case')}</label
        >
        <label class="check"
          ><input type="checkbox" bind:checked={fileNames} /> {t('File names too')}</label
        >
        <label class="ext"
          >{t('Extensions')}
          <input type="text" bind:value={exts} placeholder={t('all text files')} />
        </label>
      </div>
    </form>

    <div class="results" aria-live="polite">
      {#if result}
        {#each groups as g (g.id)}
          <section>
            <h3>{g.name}</h3>
            {#each g.hits as h (h.rel + ':' + h.line)}
              <button
                class="hit"
                title={t('Show in folder')}
                onclick={() => revealItemInDir(h.path)}
              >
                <span class="where">{h.rel}{h.line ? `:${h.line}` : ''}</span>
                <span class="line">{h.text}</span>
              </button>
            {/each}
          </section>
        {:else}
          <p class="dim">{t('No matches.')}</p>
        {/each}
      {:else}
        <p class="dim">
          {t(
            'Searches every installed mod’s files. Results are grouped by mod; click one to show the file in your file manager.',
          )}
        </p>
      {/if}
    </div>
    <footer>
      <span class="dim status">
        {#if result}
          {result.hits.length === 1 && groups.length === 1
            ? t('1 match in 1 mod · {f} files · {ms} ms', {
                f: result.files_searched,
                ms: result.ms,
              })
            : t('{n} matches in {m} mods · {f} files · {ms} ms', {
                n: result.hits.length,
                m: groups.length,
                f: result.files_searched,
                ms: result.ms,
              })}{result.truncated
            ? ' · ' + t('stopped at the result limit, refine your search')
            : ''}
        {/if}
      </span>
      <button onclick={onclose}>{t('Close')}</button>
    </footer>
  </div>
</div>

<style>
  .dialog {
    width: min(920px, 94vw);
    height: min(720px, 86vh);
    grid-template-rows: auto auto 1fr auto;
  }
  form {
    display: grid;
    gap: 0.5rem;
  }
  .query {
    display: flex;
    gap: 0.5rem;
  }
  .query input {
    flex: 1;
  }
  .opts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 1.1rem;
  }
  .check,
  .ext {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }
  .ext input {
    width: 14rem;
  }
  .results {
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--bg);
    padding: 0.4rem;
  }
  section + section {
    margin-top: 0.6rem;
  }
  h3 {
    font-size: 0.92em;
    padding: 0.1rem 0.3rem;
    position: sticky;
    top: -0.4rem;
    background: var(--bg);
  }
  .hit {
    display: flex;
    gap: 0.8rem;
    width: 100%;
    text-align: left;
    background: none;
    border: 0;
    padding: 0.15rem 0.3rem;
    border-radius: 4px;
    font-family: ui-monospace, 'Cascadia Mono', Consolas, monospace;
    font-size: 0.85em;
  }
  .hit:hover {
    background: var(--hover);
  }
  .where {
    flex: none;
    color: var(--accent);
    min-width: 0;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .line {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--fg);
  }
  .results p {
    padding: 0.5rem;
  }
  footer {
    align-items: center;
    justify-content: space-between !important;
  }
  .status {
    min-width: 0;
  }
</style>
