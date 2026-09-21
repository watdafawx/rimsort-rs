<script lang="ts">
  import type { ModRow } from '../bindings'
  import { t } from './i18n.svelte'

  let {
    ids,
    lookup,
    onjump,
  }: {
    /** Package ids (or, for "required by", already-resolved rows' package ids). */
    ids: string[]
    lookup: Map<string, ModRow>
    onjump: (row: ModRow) => void
  } = $props()

  const LIMIT = 10
  let all = $state(false)
  // A mod can list the same dependency twice (e.g. once per game version); keyed lists need unique ids.
  const unique = $derived([...new Set(ids)])
  const shown = $derived(all ? unique : unique.slice(0, LIMIT))
</script>

<span class="pkgs">
  {#each shown as id (id)}
    {@const row = lookup.get(id.toLowerCase())}
    {#if row}
      <button class="link" title={id} onclick={() => onjump(row)}>{row.name}</button>
    {:else}
      <span class="gone" title={t('Not installed')}>{id}</span>
    {/if}
  {/each}
  {#if unique.length > LIMIT && !all}
    <button class="link" onclick={() => (all = true)}
      >{t('+{n} more', { n: unique.length - LIMIT })}</button
    >
  {/if}
</span>

<style>
  .pkgs {
    display: flex;
    flex-wrap: wrap;
    gap: 0.15rem 0.6rem;
  }
  .gone {
    color: var(--dim);
    text-decoration: line-through;
    text-decoration-color: color-mix(in srgb, var(--err) 70%, transparent);
  }
</style>
