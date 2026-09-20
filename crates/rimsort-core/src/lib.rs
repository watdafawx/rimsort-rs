//! Tauri-free core logic for rimsort-rs.

pub mod dto;
mod error;
pub mod launch;
pub mod logs;
mod mod_id;
pub mod modlist_io;
pub mod mods;
pub mod modsconfig;
pub mod paths;
pub mod rules;
pub mod settings;
pub mod sort;
mod state;
mod task;
pub mod validate;
mod xml;

pub use error::{Error, ErrorDto, Result};
pub use mod_id::ModId;
pub use state::AppState;
pub use task::{TaskCtx, TaskEvent, TaskId, TaskManager, TaskSink};

pub fn ping() -> String {
    "pong from rimsort-core".to_owned()
}
