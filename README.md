# rimsort-rs

> **Personal fork, use at your own risk.** This is a personal-use project. It is not affiliated with or endorsed by the
> RimSort team, and no support is promised.
>
> **Written largely by AI.** This Rust port was built with heavy use of an AI coding assistant (Claude). It is tested
> (unit tests, and sort output compared against RimSort's Python), but read the code before trusting it with a mod setup
> you care about. Saving always keeps a timestamped backup of your `ModsConfig.xml`.

A fast rewrite of [RimSort](https://github.com/RimSort/RimSort) (RimWorld mod manager) in Rust + Tauri 2 + Svelte 5.
Goal: **speed and stability** — nothing on the UI thread waits on disk, parsing, or sorting.

It reads and writes RimSort-compatible data (settings, community rules, user rules, aux metadata), so you can point it
at an existing RimSort setup and it just works.

## Download

Grab the latest build from [Releases](https://github.com/watdafawx/rimsort-rs/releases):

- **Installer** (`RimSort-rs_<version>_x64-setup.exe`) — per-user Windows installer.
- **Portable** (`RimSort-rs_<version>_x64-portable.zip`) — unzip anywhere and run `rimsort-rs.exe`. The `portable.txt`
  next to it keeps settings, databases and logs in a `data` folder beside the exe; delete that file to use
  `%LOCALAPPDATA%\RimSort-rs` instead.

Windows 10/11 with WebView2 (built in on Windows 11). The builds are not code-signed, so SmartScreen may warn.

## What it does today

- **Instances & paths** — auto-detects RimWorld, config, local and Workshop folders (Steam registry + libraries);
  imports your RimSort instance/settings on first run.
- **Fast scan** — ~700 mods in ~100 ms warm (parallel, tolerant About.xml parsing, never blocks the UI).
- **Mod lists** — virtualized active/inactive lists with multi-select, drag & drop, keyboard, search (name, author,
  package id, tags), source filter, sorting of the inactive list, undo/redo, resizable panes.
- **Sort** — port of RimSort's tiered topological sort with community + user rules. Verified identical to RimSort's
  Python on a real 584-mod list (`just golden`).
- **Validation** — missing dependencies (with one-click enable / Workshop link), incompatibilities, load-order and
  game-version warnings; duplicate-mod panel; per-mod ignore.
- **Rules** — community rules DB + your own rules (editor writes RimSort's `userRules.json` format).
- **Save/launch** — writes `ModsConfig.xml` with automatic timestamped backups; launches the game; Player.log viewer.
- **Import/export** — RimSort JSON, ModsConfig/`.rml`/`.rws`, plain ids, clipboard reports.
- **Downloads** — Workshop mods via SteamCMD (installed on demand), GitHub mods via git; missing dependencies and
  missing-from-disk mods resolve to Workshop items through RimSort's Steam database.
- **Search** — find text/regex/file names across every mod's files (Ctrl+Shift+F).
- **Textures** — todds optimizer (downloaded on demand): presets, dry run, clean-up, live progress, cancel.
- **Maintenance** — reset per-mod/game settings and clear Steam's download cache (always via the Recycle Bin);
  warns before saving or launching while RimWorld is running.
- **Extras** — per-mod colors/tags/notes (imported from RimSort's aux DB), delete to Recycle Bin, file watching with a
  refresh banner, light/dark theme, UI in 11 languages (menus and main screen).

See [plan/](plan/README.md) for the roadmap and what is still missing (Steamworks subscribe, installers/updater,
some translations).

## Run

Requirements: Rust (stable), Node 22+, [`just`](https://github.com/casey/just), `cargo install tauri-cli --version "^2" --locked`,
and WebView2 (bundled with Windows 11).

```sh
cd ui && npm ci && cd ..
just dev        # hot-reloading app against your real config
just dev-safe   # same, but Save writes to a copy (debug/testconfig) — use while developing
just test       # cargo tests (includes "bindings.ts is up to date")
just lint       # clippy -D warnings, svelte-check, eslint
just build      # debug installers
just bindings   # regenerate ui/src/bindings.ts after changing commands/types
just golden     # compare our sort with RimSort's Python (needs `just golden-setup`, a RimSort install)
```

## Layout

```text
crates/rimsort-core/   pure Rust library, no Tauri: scan, parse, sort, validate, settings, I/O   (unit-tested)
src-tauri/             thin Tauri glue: typed commands (tauri-specta), events
ui/                    Svelte 5 + TypeScript frontend
docs/                  architecture, sorting spec, release process
tests/golden/          Python-vs-Rust sort comparison harness
scripts/               dev tooling (CDP driver, screenshots)
plan/                  roadmap, one file per step (gitignored)
reference-python/      snapshot of the original RimSort source (gitignored, read-only)
```

Start with [docs/architecture.md](docs/architecture.md). Debugging the running app (logs, screenshots, DevTools-protocol
driver) is described there too.

## License

GPL-3.0, derived from RimSort.
