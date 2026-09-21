<script lang="ts">
  import { dialogFocus } from './actions'
  import { t } from './i18n.svelte'
  import { revealItemInDir } from '@tauri-apps/plugin-opener'
  import { onMount } from 'svelte'
  import type { DupGroup } from '../bindings'
  import { call, commands } from './ipc.svelte'
  import { app, useCopy } from './store.svelte'

  let { onclose }: { onclose: () => void } = $props()
  let groups = $state<DupGroup[]>([])

  const load = async () => (groups = await call(commands.getDuplicates()))
  onMount(load)

  const LABEL: Record<string, string> = {
    Ludeon: 'Ludeon',
    SteamWorkshop: 'Steam Workshop',
    Local: 'Local',
    SteamCmd: 'SteamCMD',
    Git: 'Git',
    Unknown: 'Unknown',
  }

  async function use(id: string) {
    await useCopy(id)
    await load()
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={t('Duplicate mods')}
    tabindex="-1"
    use:dialogFocus
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>{t('Duplicate mods')}</h2>
    {#if !groups.length}
      <p class="dim">{t('No package id is installed more than once.')}</p>
    {:else}
      <p class="dim">
        {t(
          'RimWorld identifies mods by package id only, so exactly one copy of each can be active.',
        )}
        {app.dirty ? t('Changes are unsaved until you press Save.') : ''}
      </p>
      <div class="groups">
        {#each groups as g (g.package_id)}
          <section>
            <strong>{g.name}</strong> <span class="dim">{g.package_id}</span>
            {#each g.copies as c (c.id)}
              <div class="copy" class:active={c.active}>
                <div class="what">
                  <span class="src">{LABEL[c.mod_type] ?? c.mod_type}</span>
                  {#if c.mod_version}<span class="dim">v{c.mod_version}</span>{/if}
                  <div class="path dim">{c.path}</div>
                </div>
                <div class="acts">
                  {#if c.active}<span class="using">{t('in use')}</span>
                  {:else}<button onclick={() => use(c.id)}>{t('Use this copy')}</button>{/if}
                  <button onclick={() => revealItemInDir(c.path)}>{t('Open folder')}</button>
                </div>
              </div>
            {/each}
          </section>
        {/each}
      </div>
    {/if}
    <footer><button onclick={onclose}>{t('Close')}</button></footer>
  </div>
</div>

<style>
  .dialog {
    width: min(820px, 94vw);
  }

  .groups {
    overflow: auto;
    display: grid;
    gap: 0.7rem;
  }
  section {
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 0.5rem 0.7rem;
    background: var(--panel);
    display: grid;
    gap: 0.35rem;
  }
  .copy {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    padding: 0.35rem 0.5rem;
    border-radius: 5px;
    border: 1px solid transparent;
  }
  .copy.active {
    border-color: var(--accent);
  }
  .what {
    min-width: 0;
  }
  .path {
    font-size: 0.75rem;
    word-break: break-all;
  }
  .src {
    font-weight: 600;
  }
  .acts {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex: none;
  }
  .using {
    color: var(--accent);
    font-weight: 600;
  }
  .dim {
    color: var(--dim);
    font-size: 0.85em;
  }
</style>
