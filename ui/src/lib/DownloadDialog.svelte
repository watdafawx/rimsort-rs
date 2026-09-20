<script lang="ts">
  import { app, downloadMods } from './store.svelte'

  let { onclose }: { onclose: () => void } = $props()
  let text = $state('')

  /** Workshop ids from any mix of URLs (`?id=123`, `CommunityFilePage/123`) and bare numbers. */
  const ids = $derived([
    ...new Set([...text.matchAll(/(?:[?&]id=|CommunityFilePage\/|\b)(\d{6,})/g)].map((m) => m[1])),
  ])

  async function go() {
    onclose()
    await downloadMods(ids)
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label="Download from Steam Workshop"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>Download from the Steam Workshop</h2>
    <p class="dim">
      Paste Workshop links or ids (one per line or separated by spaces). Items are fetched with
      SteamCMD into your local mods folder.
    </p>
    <textarea
      rows="5"
      bind:value={text}
      placeholder="https://steamcommunity.com/sharedfiles/filedetails/?id=2009463077"></textarea>
    <div class="dim">{ids.length} item{ids.length === 1 ? '' : 's'} detected</div>
    <footer>
      <button onclick={onclose}>Cancel</button>
      <button class="primary" disabled={!ids.length || !!app.jobTask} onclick={go}>Download</button>
    </footer>
  </div>
</div>

<style>
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
    width: min(600px, 92vw);
    display: grid;
    gap: 0.6rem;
  }
  h2 {
    margin: 0;
    font-size: 1.1rem;
  }
  p {
    margin: 0;
  }
  .dim {
    color: var(--dim);
    font-size: 0.85em;
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
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
