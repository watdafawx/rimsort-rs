<script lang="ts">
  import { dialogFocus } from './actions'
  import { onMount } from 'svelte'
  import { call, commands } from './ipc.svelte'
  import { t } from './i18n.svelte'
  import { movedItems } from './listdiff'
  import { app, sort } from './store.svelte'

  let { onclose }: { onclose: () => void } = $props()

  type Move = { id: string; name: string; from: number; to: number }
  let moves = $state<Move[] | null>(null)
  let ok = $state(true)

  onMount(async () => {
    const r = await call(commands.sortPreview())
    ok = r.ok
    if (!r.ok) {
      moves = []
      return
    }
    const before = app.active.map((m) => m.id)
    const oldPos = new Map(before.map((id, i) => [id, i]))
    const newPos = new Map(r.order.map((id, i) => [id, i]))
    const names = new Map(app.active.map((m) => [m.id, m.name]))
    moves = movedItems(before, r.order)
      .map((id) => ({
        id,
        name: names.get(id) ?? id,
        from: oldPos.get(id)! + 1,
        to: newPos.get(id)! + 1,
      }))
      .sort((a, b) => Math.abs(b.to - b.from) - Math.abs(a.to - a.from))
  })

  async function apply() {
    onclose()
    await sort()
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={t('Sort preview')}
    tabindex="-1"
    use:dialogFocus
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>{t('Sort preview')}</h2>
    {#if moves === null}
      <p class="dim">{t('Working…')}</p>
    {:else if !ok}
      <p class="err">
        {t('The load-order rules contradict each other, so there is no valid order.')}
      </p>
    {:else if !moves.length}
      <p class="dim">{t('Already sorted')}</p>
    {:else}
      <p class="dim">
        {t('{n} mods would move; everything else only shifts to make room.', { n: moves.length })}
      </p>
      <ul>
        {#each moves.slice(0, 80) as m (m.id)}
          <li>
            <span class="arrow" class:up={m.to < m.from}>{m.to < m.from ? '↑' : '↓'}</span>
            <span class="name">{m.name}</span>
            <span class="pos">#{m.from} → #{m.to}</span>
          </li>
        {/each}
      </ul>
      {#if moves.length > 80}<p class="dim">{t('+{n} more', { n: moves.length - 80 })}</p>{/if}
    {/if}
    <footer>
      <button onclick={onclose}>{t('Cancel')}</button>
      <button class="primary" onclick={apply} disabled={moves === null || (ok && !moves.length)}
        >{ok ? t('Apply sort') : t('Show conflicts')}</button
      >
    </footer>
  </div>
</div>

<style>
  .dialog {
    width: min(620px, 92vw);
  }
  ul {
    max-height: 46vh;
    overflow: auto;
    display: grid;
    gap: 1px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.2rem 0.4rem;
    border-radius: var(--radius-sm);
  }
  li:hover {
    background: var(--hover);
  }
  .arrow {
    color: var(--warn);
    width: 1rem;
    text-align: center;
  }
  .arrow.up {
    color: var(--ok);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pos {
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }
  .err {
    color: var(--err);
  }
</style>
