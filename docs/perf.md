# Performance

Measured with the synthetic install generator (`scripts/gen_mods.mjs`) and the ignored `synthetic` test.
Release build, Windows 11, local NVMe, 2026-09-20. Numbers are wall time for the whole operation in core
(none of it runs on the UI thread).

| Operation (5,000 mods, all active) | Time |
|-----------------------------------|------|
| Scan + parse all About.xml + read ModsConfig (first) | ~590 ms |
| Same, warm file cache | ~530 ms |
| Build both lists (rows for UI) | ~2.3 ms |
| Validate (deps, incompatibilities, order, versions) | ~3 ms |
| Topological sort (tiers, ~3.7k edges) | ~69 ms |

Real install (user's machine): 719 mods scan in ~100 ms warm; sorting 584 active mods takes ~11 ms.

UI (WebView2, synthetic 5,000-row active list): worst frame gap over 200 scroll steps 8 ms (36 rows in the DOM);
disabling 200 selected rows took 32 ms end to end. Real 585-row list: 8 ms worst frame gap, clearing 580 rows 14 ms.

## Reproduce

```sh
just bench      # generates debug/synth (5,000 mods) and runs the benchmark
```

## Not measured yet

Memory footprint of the index, startup-to-window time, head-to-head against Python RimSort on the same list (needs a
harness around the Python app), Criterion micro-benchmarks.
