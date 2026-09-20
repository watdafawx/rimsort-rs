<script lang="ts">
  import { onMount } from 'svelte'
  import type { ModRulesView, UserRuleDto } from '../bindings'
  import { call, commands, toast } from './ipc.svelte'
  import { app, revalidate } from './store.svelte'

  let { id, onclose }: { id: string; onclose: () => void } = $props()

  let view = $state<ModRulesView | null>(null)
  let user = $state<UserRuleDto>({
    load_after: [],
    load_before: [],
    load_top: false,
    load_bottom: false,
  })
  let ignored = $state(false)
  let addAfter = $state('')
  let addBefore = $state('')

  onMount(async () => {
    view = await call(commands.getModRules(id))
    if (!view) return onclose()
    user = structuredClone($state.snapshot(view.user))
    ignored = view.ignored
  })

  const names = $derived(
    new Map([...app.active, ...app.inactive].map((r) => [r.package_id, r.name] as const)),
  )
  const label = (pid: string) => (names.has(pid) ? `${names.get(pid)} (${pid})` : pid)

  function add(list: 'load_after' | 'load_before', value: string) {
    const v = value.trim().toLowerCase()
    if (v && !user[list].includes(v)) user[list] = [...user[list], v]
  }

  async function save() {
    await call(commands.setUserRule(id, $state.snapshot(user)))
    if (ignored !== view!.ignored) await call(commands.setIgnored(id, ignored))
    revalidate(0)
    toast('Rules saved — use Sort to apply new load-order rules', 3500)
    onclose()
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label="Edit rules"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    {#if view}
      <h2>Rules — {view.name}</h2>
      <div class="dim">{view.package_id}</div>

      <section class="ro">
        <h3>From the mod &amp; community rules (read-only)</h3>
        {#each [['Load after', [...view.about_load_after, ...view.community_load_after]], ['Load before', [...view.about_load_before, ...view.community_load_before]]] as [title, list] (title)}
          {#if (list as string[]).length}
            <div><span class="dim">{title}:</span> {(list as string[]).map(label).join(', ')}</div>
          {/if}
        {/each}
        {#if view.community_top}<div>Community: load as early as possible</div>{/if}
        {#if view.community_bottom}<div>Community: load last</div>{/if}
      </section>

      <section>
        <h3>Your rules</h3>
        {#each [['Load after', 'load_after', addAfter], ['Load before', 'load_before', addBefore]] as const as [title, key] (key)}
          <div class="rule">
            <strong>{title}</strong>
            <div class="chips">
              {#each user[key] as pid (pid)}
                <span class="chip"
                  >{label(pid)}
                  <button
                    aria-label="Remove"
                    onclick={() => (user[key] = user[key].filter((p) => p !== pid))}>✕</button
                  >
                </span>
              {/each}
            </div>
            <div class="addrow">
              {#if key === 'load_after'}
                <input
                  list="pkgs"
                  placeholder="package id or name…"
                  bind:value={addAfter}
                  onkeydown={(e) => e.key === 'Enter' && (add(key, addAfter), (addAfter = ''))}
                />
                <button onclick={() => (add(key, addAfter), (addAfter = ''))}>Add</button>
              {:else}
                <input
                  list="pkgs"
                  placeholder="package id or name…"
                  bind:value={addBefore}
                  onkeydown={(e) => e.key === 'Enter' && (add(key, addBefore), (addBefore = ''))}
                />
                <button onclick={() => (add(key, addBefore), (addBefore = ''))}>Add</button>
              {/if}
            </div>
          </div>
        {/each}
        <label class="check"
          ><input type="checkbox" bind:checked={user.load_top} /> Load as early as possible (after the
          base game and DLC)</label
        >
        <label class="check"
          ><input type="checkbox" bind:checked={user.load_bottom} /> Load last</label
        >
        <label class="check"
          ><input type="checkbox" bind:checked={ignored} /> Ignore warnings for this mod</label
        >
      </section>

      <datalist id="pkgs">
        {#each [...names] as [pid, name] (pid)}<option value={pid}>{name}</option>{/each}
      </datalist>

      <footer>
        <button onclick={onclose}>Cancel</button>
        <button class="primary" onclick={save}>Save</button>
      </footer>
    {/if}
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
    width: min(720px, 92vw);
    max-height: 85vh;
    overflow: auto;
    display: grid;
    gap: 0.6rem;
  }
  h2 {
    margin: 0;
    font-size: 1.1rem;
  }
  h3 {
    margin: 0.2rem 0;
    font-size: 0.9rem;
    color: var(--dim);
    font-weight: 600;
  }
  .dim {
    color: var(--dim);
    font-size: 0.85em;
  }
  .ro {
    font-size: 0.85rem;
    display: grid;
    gap: 0.2rem;
    padding: 0.5rem 0.7rem;
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  .rule {
    display: grid;
    gap: 0.3rem;
    margin-bottom: 0.5rem;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 0.1rem 0.2rem 0.1rem 0.6rem;
    font-size: 0.85rem;
  }
  .chip button {
    border: 0;
    background: none;
    padding: 0 0.3rem;
    color: var(--dim);
  }
  .addrow {
    display: flex;
    gap: 0.5rem;
  }
  .addrow input {
    flex: 1;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
