mod helpers;
#[cfg(feature = "tray")]
mod ksni;
mod runtime;
#[cfg(feature = "tray")]
mod shortcut_hint_io;

pub(crate) use runtime::start_system_tray;

#[cfg(feature = "tray")]
use super::types::{DaemonControlEvent, TrayStatusShared};
#[cfg(feature = "tray")]
use crate::config::TrayIconStyle;
#[cfg(feature = "tray")]
use std::sync::Arc;
#[cfg(all(test, feature = "tray"))]
use std::sync::atomic::AtomicBool;
#[cfg(feature = "tray")]
use std::sync::atomic::AtomicU32;

#[cfg(feature = "tray")]
struct TrayControl {
    toggle: DaemonControlEvent,
    quit: DaemonControlEvent,
}

#[cfg(feature = "tray")]
pub(crate) struct WayscriberTray {
    control: TrayControl,
    configurator_binary: String,
    session_resume_enabled: bool,
    icon_style: TrayIconStyle,
    overlay_pid: Arc<AtomicU32>,
    tray_status: Arc<TrayStatusShared>,
}

#[cfg(feature = "tray")]
impl WayscriberTray {
    fn new(
        control: TrayControl,
        configurator_binary: String,
        session_resume_enabled: bool,
        icon_style: TrayIconStyle,
        overlay_pid: Arc<AtomicU32>,
        tray_status: Arc<TrayStatusShared>,
    ) -> Self {
        Self {
            control,
            configurator_binary,
            session_resume_enabled,
            icon_style,
            overlay_pid,
            tray_status,
        }
    }

    fn request_toggle(&self) {
        self.control.toggle.raise("tray toggle");
    }

    fn request_quit(&self) {
        self.control.quit.raise("tray quit");
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        session_resume_enabled: bool,
    ) -> Self {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        Self::new_for_tests_with_wake(
            toggle_flag,
            quit_flag,
            session_resume_enabled,
            wake.handle(),
        )
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests_with_wake(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        session_resume_enabled: bool,
        control_wake: crate::backend::wayland::RuntimeWakeHandle,
    ) -> Self {
        Self::new(
            TrayControl {
                toggle: DaemonControlEvent::new(toggle_flag, control_wake.clone()),
                quit: DaemonControlEvent::new(quit_flag, control_wake),
            },
            "true".into(),
            session_resume_enabled,
            TrayIconStyle::Auto,
            Arc::new(AtomicU32::new(0)),
            Arc::new(TrayStatusShared::new()),
        )
    }
}
