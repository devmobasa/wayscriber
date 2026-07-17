mod helpers;
#[cfg(feature = "tray")]
mod ksni;
mod runtime;
#[cfg(feature = "tray")]
mod shortcut_hint_io;

pub(crate) use runtime::start_system_tray;

#[cfg(feature = "tray")]
use super::types::OverlayActionIntents;
#[cfg(feature = "tray")]
use super::types::TrayStatusShared;
#[cfg(feature = "tray")]
use crate::config::TrayIconStyle;
#[cfg(feature = "tray")]
use std::sync::Arc;
#[cfg(feature = "tray")]
use std::sync::atomic::AtomicBool;

#[cfg(feature = "tray")]
pub(crate) struct WayscriberTray {
    toggle_flag: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
    configurator_binary: String,
    session_resume_enabled: bool,
    icon_style: TrayIconStyle,
    overlay_active: Arc<AtomicBool>,
    action_intents: Arc<OverlayActionIntents>,
    tray_status: Arc<TrayStatusShared>,
    daemon_wake: crate::backend::wayland::RuntimeWakeHandle,
}

#[cfg(feature = "tray")]
impl WayscriberTray {
    #[cfg(test)]
    pub(crate) fn new_for_tests(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        session_resume_enabled: bool,
    ) -> Self {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        Self {
            toggle_flag,
            quit_flag,
            configurator_binary: "true".into(),
            session_resume_enabled,
            icon_style: TrayIconStyle::Auto,
            overlay_active: Arc::new(AtomicBool::new(false)),
            action_intents: Arc::new(OverlayActionIntents::default()),
            tray_status: Arc::new(TrayStatusShared::new()),
            daemon_wake: wake.handle(),
        }
    }
}
