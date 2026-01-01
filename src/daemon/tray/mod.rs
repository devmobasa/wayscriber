mod helpers;
#[cfg(feature = "tray")]
mod ksni;
mod runtime;

pub(crate) use runtime::start_system_tray;

#[cfg(feature = "tray")]
use super::types::TrayStatusShared;
#[cfg(all(feature = "tray", test))]
use crate::paths::tray_action_file;
#[cfg(feature = "tray")]
use std::path::PathBuf;
#[cfg(feature = "tray")]
use std::sync::Arc;
#[cfg(feature = "tray")]
use std::sync::atomic::{AtomicBool, AtomicU32};

#[cfg(feature = "tray")]
pub(crate) struct WayscriberTray {
    toggle_flag: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
    configurator_binary: String,
    session_resume_enabled: bool,
    overlay_pid: Arc<AtomicU32>,
    tray_action_path: PathBuf,
    tray_status: Arc<TrayStatusShared>,
}

#[cfg(feature = "tray")]
impl WayscriberTray {
    fn new(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        configurator_binary: String,
        session_resume_enabled: bool,
        overlay_pid: Arc<AtomicU32>,
        tray_action_path: PathBuf,
        tray_status: Arc<TrayStatusShared>,
    ) -> Self {
        Self {
            toggle_flag,
            quit_flag,
            configurator_binary,
            session_resume_enabled,
            overlay_pid,
            tray_action_path,
            tray_status,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        session_resume_enabled: bool,
    ) -> Self {
        Self::new(
            toggle_flag,
            quit_flag,
            "true".into(),
            session_resume_enabled,
            Arc::new(AtomicU32::new(0)),
            tray_action_file(),
            Arc::new(TrayStatusShared::new()),
        )
    }
}
