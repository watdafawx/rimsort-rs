<script lang="ts">
  import { dialogFocus } from './actions'
  import { t } from './i18n.svelte'
  import { onMount } from 'svelte'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import type { ToddsOptions } from '../bindings'
  import { call, commands } from './ipc.svelte'
  import { app, runTodds } from './store.svelte'

  let { onclose }: { onclose: () => void } = $props()

  let opts = $state<ToddsOptions | null>(null)
  onMount(async () => {
    opts = await call(commands.getToddsOptions())
  })

  async function optimize() {
    if (!opts) return
    const o = $state.snapshot(opts)
    if (o.preset === 'Clean') o.preset = 'Optimized' // "clean" has its own button
    await call(commands.setToddsOptions(o))
    onclose()
    await runTodds(o, o.dry_run ? t('todds dry run finished') : t('Textures optimized'))
  }

  async function clean() {
    if (!opts) return
    const o = { ...$state.snapshot(opts), preset: 'Clean' as const }
    if (
      !o.dry_run &&
      !confirm(
        o.active_mods_target
          ? t('Delete the .dds textures todds created for the active mods?')
          : t('Delete the .dds textures todds created for every mod folder?'),
      )
    )
      return
    onclose()
    await runTodds(o, o.dry_run ? t('todds dry run finished') : t('Generated .dds files deleted'))
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={t('Optimize textures')}
    tabindex="-1"
    use:dialogFocus
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>{t('Optimize textures')}</h2>
    <p class="dim">
      <button class="link" onclick={() => openUrl('https://github.com/joseasoler/todds')}
        >todds</button
      >
      {t(
        'converts mod PNG textures to DDS so RimWorld loads faster and uses less memory. It is downloaded on first use.',
      )}
    </p>
    {#if opts}
      <fieldset>
        <legend>{t('Target')}</legend>
        <label class="check">
          <input
            type="radio"
            name="target"
            checked={opts.active_mods_target}
            onchange={() => (opts!.active_mods_target = true)}
          />
          {t('Active mods ({n})', { n: app.active.length })}
        </label>
        <label class="check">
          <input
            type="radio"
            name="target"
            checked={!opts.active_mods_target}
            onchange={() => (opts!.active_mods_target = false)}
          />
          {t('Every mod in the local and Workshop folders')}
        </label>
      </fieldset>
      <fieldset>
        <legend>{t('Options')}</legend>
        <div class="row">
          <label for="todds-preset">{t('Preset')}</label>
          <select id="todds-preset" bind:value={opts.preset}>
            <option value="Optimized">{t('Optimized (BC1 / BC7, recommended)')}</option>
            <option value="Custom">{t('Custom arguments')}</option>
          </select>
        </div>
        {#if opts.preset === 'Custom'}
          <input
            type="text"
            placeholder="e.g. -f BC7 -q 5 -ss Textures"
            aria-label="Custom todds arguments"
            bind:value={opts.custom_command}
          />
        {/if}
        <label class="check">
          <input type="checkbox" bind:checked={opts.overwrite} />
          {t('Re-encode textures that already have a DDS')}
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={opts.auto_before_launch} />
          {t('Optimize automatically before launching the game')}
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={opts.dry_run} />
          {t('Dry run (only list what would change)')}
        </label>
      </fieldset>
    {/if}
    <footer>
      <button onclick={onclose}>{t('Close')}</button>
      <button onclick={clean} disabled={!opts || !!app.jobTask}
        >{t('Delete generated .dds…')}</button
      >
      <button class="primary" onclick={optimize} disabled={!opts || !!app.jobTask}>
        {opts?.dry_run ? t('Dry run') : t('Optimize')}
      </button>
    </footer>
  </div>
</div>

<style>
  .dialog {
    width: min(560px, 92vw);
  }
  fieldset {
    display: grid;
    gap: 0.4rem;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    margin: 0;
  }
  legend {
    padding: 0 0.3rem;
    color: var(--dim);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
</style>
