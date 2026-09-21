<script lang="ts">
  import { dialogFocus } from './actions'
  import { t } from './i18n.svelte'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { onMount } from 'svelte'
  import type { MissingDep } from '../bindings'
  import { call, commands, toast } from './ipc.svelte'
  import { app, downloadMods, enable, steamSubscribe } from './store.svelte'

  let { onclose }: { onclose: () => void } = $props()
  let deps = $state<MissingDep[]>([])

  const load = async () => (deps = await call(commands.getMissingDependencies()))
  onMount(load)
  // Reload when validation reports a different count (e.g. edits made elsewhere).
  $effect(() => {
    void app.missingDeps
    load()
  })

  const installable = $derived(deps.filter((d) => d.installed))

  async function enableOne(d: MissingDep) {
    if (!d.installed) return
    await enable([d.installed])
    await load()
    toast(t('Enabled — use Sort to place it correctly'), 3000)
  }

  async function enableAll() {
    await enable(installable.map((d) => d.installed!))
    await load()
    toast(t('Enabled {n} mods — use Sort to place them correctly', { n: installable.length }), 3500)
  }

  const by = (d: MissingDep) =>
    d.required_by.slice(0, 3).join(', ') +
    (d.required_by.length > 3 ? ' ' + t('+{n} more', { n: d.required_by.length - 3 }) : '')
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={t('Missing dependencies')}
    tabindex="-1"
    use:dialogFocus
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>{t('Missing dependencies')}</h2>
    {#if !deps.length}
      <p class="dim">{t('Nothing missing. 🎉')}</p>
    {:else}
      <p class="dim">
        {deps.length === 1
          ? t('{n} required mod is not in your active list.', { n: 1 })
          : t('{n} required mods are not in your active list.', { n: deps.length })}
      </p>
      <ul>
        {#each deps as d (d.package_id)}
          <li>
            <div class="what">
              <strong>{d.name}</strong>
              <span class="dim">{d.package_id}</span>
              <div class="dim">{t('Required by {names}', { names: by(d) })}</div>
            </div>
            <div class="acts">
              {#if d.installed}
                <button class="primary" onclick={() => enableOne(d)}>{t('Enable')}</button>
              {:else}
                <button
                  disabled={!d.workshop_id || !!app.jobTask}
                  title={t('Download with SteamCMD into your local mods folder')}
                  onclick={() => downloadMods([d.workshop_id!])}>{t('Download')}</button
                >
              {/if}
              <button
                disabled={!d.workshop_id}
                title={t('Subscribe in the Steam client; Steam downloads it')}
                onclick={() => steamSubscribe([d.workshop_id!], true)}>{t('Subscribe')}</button
              >
              <button
                disabled={!d.workshop_id}
                onclick={() =>
                  openUrl(
                    `https://steamcommunity.com/sharedfiles/filedetails/?id=${d.workshop_id}`,
                  )}
              >
                {t('Workshop')}
              </button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
    <footer>
      {#if installable.length > 1}<button onclick={enableAll}
          >{t('Enable all installed ({n})', { n: installable.length })}</button
        >{/if}
      <button onclick={onclose}>{t('Close')}</button>
    </footer>
  </div>
</div>

<style>
  .dialog {
    width: min(760px, 92vw);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow: auto;
    display: grid;
    gap: 0.4rem;
  }
  li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    padding: 0.5rem 0.7rem;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--panel);
  }
  .what {
    min-width: 0;
  }
  .acts {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex: none;
  }
  .dim {
    color: var(--dim);
    font-size: 0.85em;
  }
</style>
