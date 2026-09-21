//! Types crossing the IPC boundary (all exported to TypeScript).

use crate::{ModId, mods::ModType, paths::PathCheck};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InstanceDto {
    pub name: String,
    pub game_folder: String,
    pub config_folder: String,
    pub local_folder: String,
    pub workshop_folder: String,
    pub run_args: String,
    /// Launch through the Steam client instead of the game executable.
    pub launch_via_steam: bool,
}

/// A not-installed package resolved to a Workshop item via the Steam database.
#[derive(Debug, Clone, Serialize, Type)]
pub struct WorkshopMatch {
    pub package_id: String,
    pub workshop_id: String,
    pub name: String,
}

/// Global toggles that affect scanning, sorting and validation.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OptionsDto {
    /// Treat declared `modDependencies` as implicit loadAfter rules when sorting.
    pub dependencies_as_load_after: bool,
    /// Let a dependency's alternative package ids satisfy it.
    pub use_alternative_ids: bool,
    /// Prefer `*ByVersion` About.xml entries for the running game version (applied on rescan).
    pub prefer_versioned: bool,
    /// Sort by name (dependencies first) instead of topologically — RimSort's deprecated mode.
    pub alphabetical_sort: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct SettingsView {
    pub current_instance: String,
    pub instances: Vec<InstanceDto>,
    pub sorting_algorithm: String,
    pub options: OptionsDto,
    /// Set when settings were corrupt and reset this launch.
    pub warning: Option<String>,
    pub checks: Vec<PathCheck>,
    pub game_version: String,
}

/// Compact list row; full detail is fetched with `get_mod`.
#[derive(Debug, Clone, Serialize, Type)]
pub struct ModRow {
    pub id: ModId,
    pub name: String,
    pub authors: String,
    pub package_id: String,
    pub mod_type: ModType,
    pub valid: bool,
    pub unsupported_version: bool,
    pub published_file_id: Option<String>,
    /// Folder modification time, Unix seconds (0 when unknown).
    pub modified: u32,
    /// User-chosen color (`#rrggbb`).
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub has_note: bool,
    /// Ships compiled code (`Assemblies/*.dll`); otherwise it is XML/texture content only.
    pub csharp: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ListsView {
    pub active: Vec<ModRow>,
    pub inactive: Vec<ModRow>,
    /// Package ids in `ModsConfig.xml` that aren't installed.
    pub missing: Vec<String>,
    pub game_version: String,
    pub scan_ms: u32,
    pub duplicate_package_ids: u32,
    /// Entries loaded from the community / user rules databases (0 = not found).
    pub community_rules: u32,
    pub user_rules: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ModDetail {
    pub id: ModId,
    pub name: String,
    pub package_id: String,
    pub authors: Vec<String>,
    pub description: String,
    pub path: String,
    pub url: String,
    pub mod_version: String,
    pub supported_versions: Vec<String>,
    pub mod_type: ModType,
    pub valid: bool,
    pub invalid_reason: Option<String>,
    pub published_file_id: Option<String>,
    pub dependencies: Vec<String>,
    pub load_after: Vec<String>,
    pub load_before: Vec<String>,
    pub incompatible_with: Vec<String>,
    /// Absolute path of `About/Preview.png` when it exists.
    pub preview: Option<String>,
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub note: String,
    /// When the installed Workshop version was published (unix seconds), if Steam's manifest knows.
    pub workshop_updated: Option<u32>,
    /// Load time attributed to this mod by the "Loading Progress" mod (milliseconds), if a report exists.
    /// When the mod folder first appeared on this machine (unix seconds).
    pub added: Option<u32>,
    pub startup_ms: Option<f32>,
    pub startup_off_thread_ms: Option<f32>,
}

/// Personal color/tags/note for a mod.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct MetaDto {
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct SortResultDto {
    pub ok: bool,
    /// Cycles that block sorting when `ok` is false.
    pub cycles: Vec<CycleDto>,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct CycleDto {
    /// Package ids in the cycle.
    pub members: Vec<String>,
    /// The rules forming it, with their source.
    pub rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct SaveResult {
    pub path: String,
    pub backup: Option<String>,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ImportResult {
    /// Entries resolved to installed mods and now active.
    pub imported: u32,
    /// Package ids in the file that aren't installed.
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Type)]
pub enum ExportFormat {
    /// RimWorld `ModsConfig.xml` layout.
    Xml,
    /// RimSort JSON (`version`, `activeMods`, `knownExpansions`).
    Json,
    /// One package id per line.
    PackageIds,
    /// Human-readable `Name [package.id][url]` report.
    Report,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct LogChunk {
    pub path: String,
    pub text: String,
    /// Pass back as `offset` on the next read.
    pub offset: u32,
    /// Replace the displayed log instead of appending.
    pub reset: bool,
}

/// User-editable rules for one mod (stored in `userRules.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct UserRuleDto {
    pub load_after: Vec<String>,
    pub load_before: Vec<String>,
    pub load_top: bool,
    pub load_bottom: bool,
}

/// All rule sources for one mod, for the rule editor.
#[derive(Debug, Clone, Serialize, Type)]
pub struct ModRulesView {
    pub package_id: String,
    pub name: String,
    pub about_load_after: Vec<String>,
    pub about_load_before: Vec<String>,
    pub community_load_after: Vec<String>,
    pub community_load_before: Vec<String>,
    pub community_top: bool,
    pub community_bottom: bool,
    pub user: UserRuleDto,
    /// Warnings suppressed for this mod (`ignore.json`).
    pub ignored: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct BackupInfo {
    pub path: String,
    pub unix: u32,
    pub count: u32,
}

/// One installed copy of a package that exists in several places.
#[derive(Debug, Clone, Serialize, Type)]
pub struct DupCopy {
    pub id: ModId,
    pub path: String,
    pub mod_type: ModType,
    pub published_file_id: Option<String>,
    pub mod_version: String,
    /// This copy is the one currently in the active list.
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct DupGroup {
    pub package_id: String,
    pub name: String,
    pub copies: Vec<DupCopy>,
}
