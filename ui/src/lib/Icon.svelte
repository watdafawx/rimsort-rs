<script module lang="ts">
  // Small stroke-icon set (24x24 grid, Lucide-style geometry), inlined so the app stays offline and themeable.
  const ICONS = {
    refresh: [
      'M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8',
      'M21 3v5h-5',
      'M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16',
      'M8 16H3v5',
    ],
    sort: ['m21 16-4 4-4-4', 'M17 20V4', 'm3 8 4-4 4 4', 'M7 4v16'],
    save: [
      'M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z',
      'M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7',
      'M7 3v4a1 1 0 0 0 1 1h7',
    ],
    play: ['M6 4v16l14-8z'],
    undo: ['M9 14 4 9l5-5', 'M4 9h10.5a5.5 5.5 0 0 1 0 11H11'],
    redo: ['m15 14 5-5-5-5', 'M20 9H9.5a5.5 5.5 0 0 0 0 11H13'],
    log: [
      'M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7z',
      'M14 2v4a2 2 0 0 0 2 2h4',
      'M10 9H8',
      'M16 13H8',
      'M16 17H8',
    ],
    settings: [
      'M21 4h-7',
      'M10 4H3',
      'M21 12h-9',
      'M8 12H3',
      'M21 20h-5',
      'M12 20H3',
      'M14 2v4',
      'M8 10v4',
      'M16 18v4',
    ],
    trash: [
      'M3 6h18',
      'M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6',
      'M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2',
    ],
    list: ['M3 6h.01', 'M3 12h.01', 'M3 18h.01', 'M8 6h13', 'M8 12h13', 'M8 18h13'],
    search: ['M11 3a8 8 0 1 0 0 16 8 8 0 0 0 0-16z', 'm21 21-4.3-4.3'],
    alert: [
      'm21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3',
      'M12 9v4',
      'M12 17h.01',
    ],
    error: ['M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z', 'm15 9-6 6', 'm9 9 6 6'],
    chevron: ['m6 9 6 6 6-6'],
    link: ['M9 17H7A5 5 0 0 1 7 7h2', 'M15 7h2a5 5 0 1 1 0 10h-2', 'M8 12h8'],
    download: ['M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4', 'm7 10 5 5 5-5', 'M12 15V3'],
    close: ['M18 6 6 18', 'm6 6 12 12'],
    info: ['M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z', 'M12 16v-4', 'M12 8h.01'],
    package: [
      'm7.5 4.27 9 5.15',
      'M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z',
      'm3.3 7 8.7 5 8.7-5',
      'M12 22V12',
    ],
    check: ['M20 6 9 17l-5-5'],
    folder: [
      'M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2z',
    ],
    dot: ['M12 12h.01'],
  } as const
  export type IconName = keyof typeof ICONS
</script>

<script lang="ts">
  let { name, size = 16 }: { name: IconName; size?: number } = $props()
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#each ICONS[name] as d (d)}<path {d} />{/each}
</svg>

<style>
  .icon {
    flex: none;
    display: inline-block;
    vertical-align: -0.15em;
  }
</style>
