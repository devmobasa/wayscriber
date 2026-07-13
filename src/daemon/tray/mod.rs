mod helpers;
#[cfg(feature = "tray")]
mod ksni;
mod runtime;
#[cfg(feature = "tray")]
mod shortcut_hint_io;

pub(crate) use runtime::start_system_tray;

#[cfg(feature = "tray")]
use super::types::TrayStatusShared;
#[cfg(feature = "tray")]
use crate::config::TrayIconStyle;
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
    icon_style: TrayIconStyle,
    overlay_pid: Arc<AtomicU32>,
    tray_status: Arc<TrayStatusShared>,
}

#[cfg(feature = "tray")]
impl WayscriberTray {
    fn new(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        configurator_binary: String,
        session_resume_enabled: bool,
        icon_style: TrayIconStyle,
        overlay_pid: Arc<AtomicU32>,
        tray_status: Arc<TrayStatusShared>,
    ) -> Self {
        Self {
            toggle_flag,
            quit_flag,
            configurator_binary,
            session_resume_enabled,
            icon_style,
            overlay_pid,
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
            TrayIconStyle::Auto,
            Arc::new(AtomicU32::new(0)),
            Arc::new(TrayStatusShared::new()),
        )
    }
}
