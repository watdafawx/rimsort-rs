# Architecture

**Rule: the UI thread never waits on work.** Core logic lives in `crates/rimsort-core` (no Tauri dependency); `src-tauri` is thin glue; `ui/` is Svelte.

## Long-running work

1. A command validates input and calls `state.tasks.spawn(|ctx| { ... })`, then **returns a `TaskId` immediately**.
2. The closure runs on its own worker thread. It calls `ctx.progress(done, total, msg)` and `ctx.check()?` (returns `Cancelled` if `cancel_task(id)` was called).
3. `TaskManager` reports through a `TaskSink`. `src-tauri` implements the sink by emitting the typed `task-update` event (`TaskEvent`: `progress | finished | cancelled | failed`).
4. `ui/src/lib/ipc.svelte.ts` applies events to a reactive `tasks` store; failures raise a toast.

Results of a finished task are fetched by a normal command (or a domain event such as `mods://changed`), not carried in `finished`. Anything under ~16 ms can stay a plain synchronous command.

## Typed IPC

Commands are annotated `#[tauri::command] #[specta::specta]` and listed in `builder()` in `src-tauri/src/main.rs`. `ui/src/bindings.ts` is generated from them:

- debug app start rewrites it, or run `just bindings`;
- the `bindings_up_to_date` test fails if it is stale (CI runs `cargo test`).

Commands return `Result<T, ErrorDto>` (`{kind, message}`); wrap calls in `call()` to unwrap and toast errors.

## Debugging the running app

- Debug logs: `debug/logs/rimsort.log.*` (Rust `tracing` + frontend console/uncaught errors).
- `just shot <name>` → `debug/screenshots/<name>.png`; `scripts/click.ps1 x y` clicks at window-relative pixels.

## IDs

`ModId` = uuid5 of the mod folder path: stable across scans; UI lists refer to mods by id and fetch detail lazily.

## Core modules

| Module | What it does |
|---|---|
| `mods`, `xml`, `steamacf` | Parallel scan and tolerant About.xml parsing; Steam manifest update times |
| `sort`, `validate`, `rules` | Tiered topological (and legacy alphabetical) sort; warnings; community/user rules |
| `modsconfig`, `modlist_io` | ModsConfig.xml read/write with 20 timestamped backups; RimSort/other list formats |
| `steamcmd`, `gitmods`, `todds` | External tools run as cancellable tasks. SteamCMD and todds are downloaded on demand into the data folder |
| `steamdb`, `dbupdate` | RimSort's Steam Workshop database (package id → Workshop id, unambiguous matches only); ETag database downloads |
| `search` | Parallel, capped search inside mod folders |
| `troubleshoot` | Config/cache resets; everything goes to the Recycle Bin, `ModsConfig.xml` is never touched |
| `launch` | Game launch, running-game detection (`sysinfo`) |

External processes never block the UI: they run inside a task, stream output into `ctx.progress`, and are killed when the task is cancelled.
