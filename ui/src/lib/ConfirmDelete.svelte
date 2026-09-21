<script lang="ts">
  import type { ModDetail } from '../bindings'

  let { mod, onconfirm, onclose }: { mod: ModDetail; onconfirm: () => void; onclose: () => void } =
    $props()

  const workshop = $derived(mod.mod_type === 'SteamWorkshop')
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="dialog"
    role="alertdialog"
    aria-modal="true"
    aria-label="Delete mod"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && onclose()}
  >
    <h2>Delete “{mod.name}”?</h2>
    <p>This moves the mod's folder to the Recycle Bin:</p>
    <p class="path">{mod.path}</p>
    {#if workshop}
      <p class="warn">
        This is a Steam Workshop mod. While you stay subscribed, Steam may download it again —
        unsubscribe in Steam to remove it for good.
      </p>
    {/if}
    <footer>
      <button onclick={onclose}>Cancel</button>
      <button class="danger" onclick={onconfirm}>Move to Recycle Bin</button>
    </footer>
  </div>
</div>

<style>
  .dialog {
    width: min(560px, 92vw);
  }

  p {
    margin: 0;
  }
  .path {
    font-size: 0.8rem;
    color: var(--dim);
    word-break: break-all;
  }
  .warn {
    color: #ecc94b;
    font-size: 0.9rem;
  }

  .danger {
    background: #c53030;
    border-color: #c53030;
    color: #fff;
  }
  .danger:hover {
    background: #e53e3e !important;
  }
</style>
