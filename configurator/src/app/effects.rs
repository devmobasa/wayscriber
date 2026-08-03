//! Side effects the update layer asks for.
//!
//! Handlers stay pure functions of state: they mutate the model and return
//! the effects to run, never touching the GTK main loop or an async runtime
//! themselves. The component maps each effect onto a Relm4 command exactly
//! once ([`super::component`]), so the update layer stays framework-free
//! and handler tests assert on returned effects instead of driving a UI.

use std::path::PathBuf;

use wayscriber::config::{Config, ConfigDocument};

use crate::models::DaemonAction;

/// One asynchronous job a handler wants started. Fields carry everything
/// the job needs, so running it reads nothing back out of the model.
#[derive(Debug)]
pub(crate) enum Effect {
    LoadConfig,
    /// The document leaves the model with the write and comes back with its
    /// result, so exactly one owner holds it at any moment.
    SaveConfig {
        document: Box<ConfigDocument>,
        config: Box<Config>,
    },
    LoadDaemonStatus {
        request_id: u64,
    },
    PerformDaemonAction {
        action: DaemonAction,
        shortcut_input: String,
    },
    LoadSessionCatalog,
    ForgetSessionEntry {
        id: String,
    },
    RenameSessionEntry {
        id: String,
        display_name: String,
    },
    DuplicateSessionEntry {
        id: String,
        target: PathBuf,
    },
    MoveSessionEntry {
        id: String,
        target: PathBuf,
    },
    RevealSessionEntry {
        id: String,
    },
    ClearSessionToolState {
        id: String,
    },
    ClearSessionEntry {
        id: String,
    },
}
