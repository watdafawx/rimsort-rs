<script lang="ts">
  import { onMount } from 'svelte'
  import type { ModDetail } from '../bindings'
  import { call, commands, toast } from './ipc.svelte'
  import { patchRow } from './store.svelte'

  let { id, onclose }: { id: string; onclose: () => void } = $props()

  const PALETTE = [
    '#e53e3e',
    '#ed8936',
    '#ecc94b',
    '#48bb78',
    '#38b2ac',
    '#4299e1',
    '#9f7aea',
    '#ed64a6',
  ]
  let detail = $state<ModDetail | null>(null)
  let color = $state<string | null>(null)
  let tags = $state('')
  let note = $state('')

  onMount(async () => {
    detail = await call(commands.getMod(id))
    if (!detail) return onclose()
    color = detail.color
    tags = detail.tags.join(', ')
    note = detail.note
  })

  async function save() {
    const list = tags
      .split(',')
      .map((t) => t.trim())
      .filter(Boolean)
    await call(commands.setModMeta(id, { color, tags: list, note }))
    patchRow(id, { color, tags: list, has_note: note.trim() !== '' })
    toast('Saved', 1500)
    onclose()
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label="Color, tags and notes"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    {#if detail}
      <h2>{detail.name}</h2>
      <div class="field">
        <span class="lbl">Color</span>
        <div class="swatches">
          {#each PALETTE as c (c)}
            <button
              class="sw"
              class:on={color === c}
              style:background={c}
              aria-label={c}
              onclick={() => (color = c)}
            ></button>
          {/each}
          <input
            type="color"
            value={color ?? '#888888'}
            oninput={(e) => (color = e.currentTarget.value)}
            aria-label="Custom color"
          />
          <button onclick={() => (color = null)} disabled={!color}>Clear</button>
        </div>
      </div>
      <div class="field">
        <label class="lbl" for="tags">Tags (comma separated)</label>
        <input id="tags" bind:value={tags} placeholder="qol, combat, visuals" />
      </div>
      <div class="field">
        <label class="lbl" for="note">Notes</label>
        <textarea id="note" rows="5" bind:value={note}></textarea>
      </div>
      <footer>
        <button onclick={onclose}>Cancel</button>
        <button class="primary" onclick={save}>Save</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .dialog {
    width: min(520px, 92vw);
  }

  .field {
    display: grid;
    gap: 0.3rem;
  }
  .lbl {
    color: var(--dim);
    font-size: 0.85rem;
  }
  .swatches {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    flex-wrap: wrap;
  }
  .sw {
    width: 1.5rem;
    height: 1.5rem;
    border-radius: 50%;
    padding: 0;
    border: 2px solid transparent;
  }
  .sw.on {
    border-color: var(--fg);
  }
  input[type='color'] {
    padding: 0;
    width: 2rem;
    height: 1.7rem;
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
