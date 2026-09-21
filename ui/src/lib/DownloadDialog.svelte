<script lang="ts">
  import { dialogFocus } from './actions'
  import { t } from './i18n.svelte'
  import { app, cloneGitMods, downloadMods } from './store.svelte'

  let { onclose }: { onclose: () => void } = $props()
  let text = $state('')

  const GITHUB = /https:\/\/github\.com\/[\w-]+\/[\w-][\w.-]*?(?:\.git)?(?=[\s/?#]|$)/g
  /** GitHub repository links (deduplicated). */
  const repos = $derived([...new Set(text.match(GITHUB) ?? [])])
  /** Workshop ids from URLs (`?id=123`, `CommunityFilePage/123`) and bare numbers; GitHub links excluded. */
  const ids = $derived([
    ...new Set(
      [...text.replace(GITHUB, ' ').matchAll(/(?:[?&]id=|CommunityFilePage\/|\b)(\d{6,})/g)].map(
        (m) => m[1],
      ),
    ),
  ])

  async function go() {
    const [w, g] = [ids, repos]
    onclose()
    if (w.length) await downloadMods(w)
    if (g.length) await cloneGitMods(g)
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={t('Download mods')}
    tabindex="-1"
    use:dialogFocus
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>{t('Download mods')}</h2>
    <p class="dim">
      {t(
        'Paste Steam Workshop links/ids (fetched with SteamCMD) and/or GitHub repository links (cloned with git), one per line. Everything lands in your local mods folder.',
      )}
    </p>
    <textarea
      rows="5"
      bind:value={text}
      placeholder="https://steamcommunity.com/sharedfiles/filedetails/?id=2009463077&#10;https://github.com/Zetrith/Prepatcher"
    ></textarea>
    <div class="dim summary">
      {t('Workshop items: {w} · GitHub repositories: {g}', { w: ids.length, g: repos.length })}
    </div>
    <footer>
      <button onclick={onclose}>{t('Cancel')}</button>
      <button
        class="primary"
        disabled={(!ids.length && !repos.length) || !!app.jobTask}
        onclick={go}
      >
        {t('Download')}
      </button>
    </footer>
  </div>
</div>

<style>
  .dialog {
    width: min(600px, 92vw);
  }

  textarea {
    font: inherit;
    color: inherit;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 0.4rem;
    resize: vertical;
  }
</style>
