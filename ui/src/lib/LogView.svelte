<script lang="ts">
  import { revealItemInDir } from '@tauri-apps/plugin-opener'
  import { onMount, tick } from 'svelte'
  import { commands, toast } from './ipc.svelte'

  const MAX_LINES = 5000
  type Line = { text: string; level: 'err' | 'warn' | '' }

  let lines = $state<Line[]>([])
  let path = $state('')
  let problem = $state('')
  let follow = $state(true)
  let query = $state('')
  let problemsOnly = $state(false)
  let scroller = $state<HTMLDivElement>()
  let offset: number | null = null
  let prev: Line['level'] = ''

  /** Stack-trace continuation lines inherit the level of the line that started them. */
  const CONT = /^\s+at |^\(Filename:|^\s*$|^UnityEngine\.|^Verse\.|^System\./
  function classify(text: string): Line['level'] {
    if (CONT.test(text)) return prev
    if (/exception|\berror\b|failed to|could not|cannot |unable to/i.test(text)) return 'err'
    if (/\bwarning\b|\bwarn\b/i.test(text)) return 'warn'
    return ''
  }

  async function poll() {
    const r = await commands.readPlayerLog(offset)
    if (r.status === 'error') {
      problem = r.error.message
      return
    }
    problem = ''
    const c = r.data
    path = c.path
    offset = c.offset
    if (c.reset) {
      lines = []
      prev = ''
    }
    if (!c.text) return
    const added = c.text
      .split(/\r?\n/)
      .filter((t, i, a) => t !== '' || i < a.length - 1)
      .map((text): Line => ((prev = classify(text)), { text, level: prev }))
    lines = [...lines, ...added].slice(-MAX_LINES)
    if (follow) {
      await tick()
      if (scroller) scroller.scrollTop = scroller.scrollHeight
    }
  }

  onMount(() => {
    poll()
    const t = setInterval(poll, 1000)
    return () => clearInterval(t)
  })

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase()
    return lines.filter(
      (l) => (!problemsOnly || l.level) && (!q || l.text.toLowerCase().includes(q)),
    )
  })
  const counts = $derived({
    err: lines.filter((l) => l.level === 'err').length,
    warn: lines.filter((l) => l.level === 'warn').length,
  })

  async function copyAll() {
    await navigator.clipboard.writeText(shown.map((l) => l.text).join('\n'))
    toast(`Copied ${shown.length} lines`, 2000)
  }
</script>

<section class="log">
  <header>
    <strong>Player.log</strong>
    <span class="err">{counts.err} errors</span>
    <span class="warn">{counts.warn} warnings</span>
    <label><input type="checkbox" bind:checked={problemsOnly} /> problems only</label>
    <label><input type="checkbox" bind:checked={follow} /> follow</label>
    <input type="search" placeholder="Search log…" bind:value={query} />
    <button onclick={copyAll}>Copy</button>
    <button onclick={() => path && revealItemInDir(path)} disabled={!path}>Open file</button>
    <button onclick={() => (lines = [])}>Clear view</button>
  </header>
  {#if problem}
    <p class="empty">{problem}</p>
  {:else}
    <div
      class="scroll"
      bind:this={scroller}
      onscroll={() => {
        // Scrolling up pauses follow; reaching the bottom resumes it.
        const el = scroller!
        const atEnd = el.scrollHeight - el.scrollTop - el.clientHeight < 24
        if (!atEnd && follow) follow = false
        else if (atEnd && !follow) follow = true
      }}
    >
      {#each shown as l, i (i)}<div class="line {l.level}">{l.text}</div>{/each}
    </div>
  {/if}
</section>

<style>
  .log {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.4rem 0.6rem;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
  }
  header input[type='search'] {
    flex: 1;
    min-width: 6rem;
  }
  label {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    color: var(--dim);
    white-space: nowrap;
  }
  .err {
    color: #f56565;
  }
  .warn {
    color: #ecc94b;
  }
  .scroll {
    flex: 1;
    overflow: auto;
    font:
      12px/1.45 ui-monospace,
      'Cascadia Mono',
      Consolas,
      monospace;
    padding: 0.4rem 0.6rem;
  }
  .line {
    white-space: pre-wrap;
    word-break: break-word;
  }
  .line.err {
    color: #f56565;
  }
  .line.warn {
    color: #ecc94b;
  }
  .empty {
    color: var(--dim);
    text-align: center;
    margin-top: 2rem;
  }
</style>
