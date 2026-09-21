#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rimsort_core::{
    AppState, ErrorDto, ModId, TaskEvent, TaskId, TaskManager, TaskSink,
    dto::{
        ExportFormat, ImportResult, InstanceDto, ListsView, LogChunk, ModDetail, SaveResult,
        SettingsView, SortResultDto,
    },
    paths::DetectedPaths,
};
use serde::{Deserialize, Serialize};
use specta::Type;
#[cfg(any(debug_assertions, test))]
use specta_typescript::Typescript;
use std::{path::PathBuf, sync::Arc};
use tauri::{AppHandle, Manager, State, Wry};
use tauri_specta::{Builder, Event, collect_commands, collect_events};
use tracing_subscriber::EnvFilter;

type Cmd<T> = Result<T, ErrorDto>;

/// Emitted for every task state change (`task://` in the plan; one typed event carrying a tagged enum).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
struct TaskUpdate(TaskEvent);

struct TauriSink(AppHandle);
impl TaskSink for TauriSink {
    fn emit(&self, event: TaskEvent) {
        if let Err(e) = TaskUpdate(event).emit(&self.0) {
            tracing::warn!("task event emit failed: {e}");
        }
    }
}

#[tauri::command]
#[specta::specta]
fn ping() -> String {
    rimsort_core::ping()
}

/// Frontend console/error sink so UI problems land in the same log file.
#[tauri::command]
#[specta::specta]
fn log_frontend(level: String, message: String) {
    match level.as_str() {
        "error" => tracing::error!(target: "ui", "{message}"),
        "warn" => tracing::warn!(target: "ui", "{message}"),
        "debug" => tracing::debug!(target: "ui", "{message}"),
        _ => tracing::info!(target: "ui", "{message}"),
    }
}

type St<'a> = State<'a, Arc<AppState>>;

#[tauri::command]
#[specta::specta]
async fn get_settings(state: St<'_>) -> Cmd<SettingsView> {
    Ok(state.settings_view())
}

#[tauri::command]
#[specta::specta]
async fn save_instance(state: St<'_>, instance: InstanceDto) -> Cmd<()> {
    Ok(state.save_instance(instance)?)
}

#[tauri::command]
#[specta::specta]
async fn update_options(state: St<'_>, options: rimsort_core::dto::OptionsDto) -> Cmd<()> {
    Ok(state.update_options(options)?)
}

#[tauri::command]
#[specta::specta]
async fn switch_instance(state: St<'_>, name: String) -> Cmd<()> {
    Ok(state.switch_instance(&name)?)
}

#[tauri::command]
#[specta::specta]
async fn create_instance(state: St<'_>, name: String) -> Cmd<()> {
    Ok(state.create_instance(&name)?)
}

#[tauri::command]
#[specta::specta]
async fn delete_instance(state: St<'_>, name: String) -> Cmd<()> {
    Ok(state.delete_instance(&name)?)
}

#[tauri::command]
#[specta::specta]
async fn autodetect_paths(state: St<'_>) -> Cmd<DetectedPaths> {
    Ok(state.autodetect())
}

/// Scan mods + read ModsConfig.xml in the background; refetch lists on the task's `finished` event.
#[tauri::command]
#[specta::specta]
async fn start_scan(app: AppHandle, state: St<'_>, keep_active: bool) -> Cmd<TaskId> {
    // Let the webview load preview images from the configured mod folders (and nothing else).
    let view = state.settings_view();
    if let Some(i) = view
        .instances
        .iter()
        .find(|i| i.name == view.current_instance)
    {
        for dir in [&i.game_folder, &i.local_folder, &i.workshop_folder] {
            if !dir.is_empty() {
                let _ = app.asset_protocol_scope().allow_directory(dir, true);
            }
        }
    }
    Ok(state.inner().start_scan(keep_active)?)
}

#[tauri::command]
#[specta::specta]
async fn get_lists(state: St<'_>) -> Cmd<ListsView> {
    Ok(state.lists())
}

#[tauri::command]
#[specta::specta]
async fn get_mod(state: St<'_>, id: ModId) -> Cmd<Option<ModDetail>> {
    Ok(state.mod_detail(id))
}

#[tauri::command]
#[specta::specta]
async fn set_active(state: St<'_>, ids: Vec<ModId>) -> Cmd<()> {
    state.set_active(ids);
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn get_validation(state: St<'_>) -> Cmd<rimsort_core::validate::ValidationView> {
    Ok(state.validate())
}

#[tauri::command]
#[specta::specta]
async fn get_missing_dependencies(state: St<'_>) -> Cmd<Vec<rimsort_core::validate::MissingDep>> {
    Ok(state.missing_dependencies())
}

#[tauri::command]
#[specta::specta]
async fn get_mod_rules(state: St<'_>, id: ModId) -> Cmd<Option<rimsort_core::dto::ModRulesView>> {
    Ok(state.mod_rules(id))
}

#[tauri::command]
#[specta::specta]
async fn set_user_rule(state: St<'_>, id: ModId, rule: rimsort_core::dto::UserRuleDto) -> Cmd<()> {
    Ok(state.set_user_rule(id, rule)?)
}

#[tauri::command]
#[specta::specta]
async fn set_ignored(state: St<'_>, id: ModId, ignored: bool) -> Cmd<()> {
    Ok(state.set_ignored(id, ignored)?)
}

#[tauri::command]
#[specta::specta]
async fn get_duplicates(state: St<'_>) -> Cmd<Vec<rimsort_core::dto::DupGroup>> {
    Ok(state.duplicates())
}

#[tauri::command]
#[specta::specta]
async fn delete_mod(state: St<'_>, id: ModId) -> Cmd<()> {
    Ok(state.delete_mod(id)?)
}

#[tauri::command]
#[specta::specta]
async fn set_mod_meta(state: St<'_>, id: ModId, meta: rimsort_core::dto::MetaDto) -> Cmd<()> {
    Ok(state.set_mod_meta(id, meta)?)
}

#[tauri::command]
#[specta::specta]
async fn update_git_mod(state: St<'_>, id: ModId) -> Cmd<String> {
    let state = state.inner().clone();
    let out = tauri::async_runtime::spawn_blocking(move || state.update_git_mod(id))
        .await
        .map_err(|e| ErrorDto {
            kind: "error".into(),
            message: e.to_string(),
        })??;
    Ok(out)
}

#[tauri::command]
#[specta::specta]
async fn update_databases(state: St<'_>) -> Cmd<Vec<rimsort_core::dbupdate::DbResult>> {
    let version = state.settings_view().game_version;
    tauri::async_runtime::spawn_blocking(move || rimsort_core::dbupdate::update_all(&version))
        .await
        .map_err(|e| ErrorDto {
            kind: "error".into(),
            message: e.to_string(),
        })
        .map(Ok)?
}

#[tauri::command]
#[specta::specta]
async fn download_mods(state: St<'_>, ids: Vec<String>) -> Cmd<TaskId> {
    Ok(state.inner().download_mods(ids)?)
}

#[tauri::command]
#[specta::specta]
async fn clone_git_mods(state: St<'_>, urls: Vec<String>) -> Cmd<TaskId> {
    Ok(state.inner().clone_git_mods(urls)?)
}

#[tauri::command]
#[specta::specta]
async fn list_backups(state: St<'_>) -> Cmd<Vec<rimsort_core::dto::BackupInfo>> {
    Ok(state.list_backups()?)
}

#[tauri::command]
#[specta::specta]
async fn create_local_copy(state: St<'_>, id: ModId) -> Cmd<TaskId> {
    Ok(state.inner().create_local_copy(id)?)
}

#[tauri::command]
#[specta::specta]
async fn sort_active(state: St<'_>) -> Cmd<SortResultDto> {
    Ok(state.sort_active())
}

#[tauri::command]
#[specta::specta]
async fn save_mods_config(state: St<'_>) -> Cmd<SaveResult> {
    Ok(state.save_mods_config()?)
}

#[tauri::command]
#[specta::specta]
async fn import_modlist(state: St<'_>, path: String) -> Cmd<ImportResult> {
    Ok(state.import_modlist(&path)?)
}

#[tauri::command]
#[specta::specta]
async fn export_modlist(state: St<'_>, path: String, format: ExportFormat) -> Cmd<()> {
    Ok(state.export_modlist(&path, format)?)
}

#[tauri::command]
#[specta::specta]
async fn export_modlist_text(state: St<'_>, format: ExportFormat) -> Cmd<String> {
    Ok(state.export_text(format))
}

#[tauri::command]
#[specta::specta]
async fn read_player_log(state: St<'_>, offset: Option<u32>) -> Cmd<LogChunk> {
    Ok(state.read_player_log(offset)?)
}

#[tauri::command]
#[specta::specta]
async fn search_mods(
    state: St<'_>,
    query: rimsort_core::search::SearchQuery,
) -> Cmd<rimsort_core::search::SearchResult> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.search_mods(&query))
        .await
        .map_err(|e| rimsort_core::Error::Other(e.to_string()))?
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
async fn troubleshoot_preview(
    state: St<'_>,
    fix: rimsort_core::troubleshoot::Fix,
) -> Cmd<Vec<String>> {
    Ok(state.troubleshoot_preview(fix)?)
}

#[tauri::command]
#[specta::specta]
async fn troubleshoot_apply(state: St<'_>, fix: rimsort_core::troubleshoot::Fix) -> Cmd<u32> {
    Ok(state.troubleshoot_apply(fix)?)
}

#[tauri::command]
#[specta::specta]
async fn workshop_matches(
    state: St<'_>,
    package_ids: Vec<String>,
) -> Cmd<Vec<rimsort_core::dto::WorkshopMatch>> {
    Ok(state.workshop_matches(&package_ids))
}

#[tauri::command]
#[specta::specta]
async fn get_todds_options(state: St<'_>) -> Cmd<rimsort_core::todds::ToddsOptions> {
    Ok(state.todds_options())
}

#[tauri::command]
#[specta::specta]
async fn set_todds_options(state: St<'_>, options: rimsort_core::todds::ToddsOptions) -> Cmd<()> {
    Ok(state.set_todds_options(options)?)
}

#[tauri::command]
#[specta::specta]
async fn run_todds(state: St<'_>, options: rimsort_core::todds::ToddsOptions) -> Cmd<TaskId> {
    Ok(state.inner().run_todds(options)?)
}

/// Folders the UI can open in the system file manager.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
enum FolderKind {
    Game,
    Config,
    Local,
    Workshop,
    Data,
    Logs,
}

#[tauri::command]
#[specta::specta]
async fn open_folder(app: AppHandle, state: St<'_>, kind: FolderKind) -> Cmd<()> {
    use tauri_plugin_opener::OpenerExt;
    let view = state.settings_view();
    let inst = view
        .instances
        .iter()
        .find(|i| i.name == view.current_instance);
    let path = match kind {
        FolderKind::Data => rimsort_core::settings::data_dir(),
        FolderKind::Logs => {
            log_dir_of(&app).map_err(|e| rimsort_core::Error::Other(e.to_string()))?
        }
        _ => {
            let inst =
                inst.ok_or_else(|| rimsort_core::Error::Other("No instance configured".into()))?;
            PathBuf::from(match kind {
                FolderKind::Game => &inst.game_folder,
                FolderKind::Config => &inst.config_folder,
                FolderKind::Local => &inst.local_folder,
                _ => &inst.workshop_folder,
            })
        }
    };
    if !path.is_dir() {
        return Err(
            rimsort_core::Error::Other(format!("Folder not found: {}", path.display())).into(),
        );
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| rimsort_core::Error::Other(e.to_string()))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn game_running(state: St<'_>) -> Cmd<bool> {
    Ok(state.game_running())
}

#[tauri::command]
#[specta::specta]
async fn launch_game(state: St<'_>) -> Cmd<()> {
    Ok(state.launch_game()?)
}

#[tauri::command]
#[specta::specta]
fn cancel_task(state: St<'_>, id: TaskId) -> Cmd<()> {
    Ok(state.tasks.cancel(id)?)
}

fn builder() -> Builder<Wry> {
    Builder::<Wry>::new()
        .commands(collect_commands![
            ping,
            log_frontend,
            get_settings,
            save_instance,
            update_options,
            switch_instance,
            create_instance,
            delete_instance,
            autodetect_paths,
            start_scan,
            get_lists,
            get_mod,
            set_active,
            get_validation,
            create_local_copy,
            list_backups,
            clone_git_mods,
            download_mods,
            update_databases,
            update_git_mod,
            set_mod_meta,
            delete_mod,
            get_duplicates,
            get_mod_rules,
            set_user_rule,
            set_ignored,
            get_missing_dependencies,
            sort_active,
            save_mods_config,
            import_modlist,
            export_modlist,
            export_modlist_text,
            read_player_log,
            launch_game,
            game_running,
            open_folder,
            get_todds_options,
            set_todds_options,
            run_todds,
            workshop_matches,
            troubleshoot_preview,
            troubleshoot_apply,
            search_mods,
            cancel_task
        ])
        .events(collect_events![TaskUpdate])
}

/// Debug builds log to `<repo>/debug/logs` (readable by dev tooling); release to the app log dir.
fn log_dir(app: &tauri::App) -> tauri::Result<PathBuf> {
    log_dir_of(app.handle())
}

fn log_dir_of(app: &AppHandle) -> tauri::Result<PathBuf> {
    if cfg!(debug_assertions) {
        Ok(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../debug/logs"
        )))
    } else {
        app.path().app_log_dir()
    }
}

#[cfg(any(debug_assertions, test))]
const BINDINGS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../ui/src/bindings.ts");

fn main() {
    let builder = builder();
    #[cfg(debug_assertions)]
    builder
        .export(Typescript::default(), BINDINGS)
        .expect("export TS bindings");

    tauri::Builder::default()
        // Must be first: a second launch focuses the running window instead of starting another app.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let appender = tracing_appender::rolling::daily(log_dir(app)?, "rimsort.log");
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let default = if cfg!(debug_assertions) {
                "debug"
            } else {
                "info"
            };
            tracing_subscriber::fmt()
                .with_writer(writer)
                .with_ansi(false)
                .with_env_filter(
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| default.into()),
                )
                .init();
            app.manage(guard); // keep flush guard alive for app lifetime
            std::panic::set_hook(Box::new(|info| tracing::error!("panic: {info}")));

            app.manage(AppState::new(TaskManager::new(TauriSink(
                app.handle().clone(),
            ))));
            tracing::info!("rimsort-rs started");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}

#[cfg(test)]
mod tests {
    /// Fails if `ui/src/bindings.ts` is stale vs. the Rust commands/types (run `just bindings` to fix).
    #[test]
    fn bindings_up_to_date() {
        let update = std::env::var_os("UPDATE_BINDINGS").is_some();
        let tmp = if update {
            super::BINDINGS.into()
        } else {
            std::env::temp_dir().join("rimsort-bindings-check.ts")
        };
        super::builder()
            .export(super::Typescript::default(), &tmp)
            .unwrap();
        if update {
            return;
        }
        let fresh = std::fs::read_to_string(&tmp).unwrap();
        let committed = std::fs::read_to_string(super::BINDINGS).unwrap_or_default();
        assert_eq!(
            fresh.replace("\r\n", "\n"),
            committed.replace("\r\n", "\n"),
            "stale bindings.ts"
        );
    }
}
