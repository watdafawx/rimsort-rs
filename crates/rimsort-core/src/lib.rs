//! Tauri-free core logic for rimsort-rs.

pub mod dbupdate;
pub mod dto;
mod error;
#[cfg(test)]
mod fuzz;
pub mod gitmods;
pub mod launch;
pub mod logs;
pub mod meta;
mod mod_id;
pub mod modlist_io;
pub mod mods;
pub mod modsconfig;
pub mod paths;
pub mod rules;
pub mod search;
pub mod settings;
pub mod sort;
pub mod startup_impact;
mod state;
pub mod steamacf;
pub mod steamcmd;
pub mod steamdb;
mod task;
pub mod todds;
pub mod troubleshoot;
pub mod validate;
pub mod watch;
mod xml;

pub use error::{Error, ErrorDto, Result};
pub use mod_id::ModId;
pub use state::AppState;
pub use task::{TaskCtx, TaskEvent, TaskId, TaskManager, TaskSink};

pub fn ping() -> String {
    "pong from rimsort-core".to_owned()
}
