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
