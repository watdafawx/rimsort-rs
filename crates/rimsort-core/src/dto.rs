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
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct SettingsView {
    pub current_instance: String,
    pub instances: Vec<InstanceDto>,
    pub sorting_algorithm: String,
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
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct SortResultDto {
    pub ok: bool,
    /// Package-id cycles when `ok` is false.
    pub cycles: Vec<Vec<String>>,
    pub changed: bool,
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
