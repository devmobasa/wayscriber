mod helpers;
#[cfg(feature = "tray")]
mod ksni;
mod runtime;
#[cfg(feature = "tray")]
mod shortcut_hint_io;

pub(crate) use runtime::start_system_tray;

#[cfg(feature = "tray")]
use super::types::{
    DaemonControlEvent, OverlayActionPublisher, TrayStatusShared, VisibilityPublisher,
};
#[cfg(all(test, feature = "tray"))]
use super::types::{OverlayActionIntents, VisibilityIntents};
#[cfg(feature = "tray")]
use crate::config::TrayIconStyle;
#[cfg(feature = "tray")]
use std::sync::Arc;
#[cfg(feature = "tray")]
use std::sync::atomic::AtomicBool;

#[cfg(feature = "tray")]
struct TrayControl {
    visibility: VisibilityPublisher,
    action: OverlayActionPublisher,
    quit: DaemonControlEvent,
}

#[cfg(feature = "tray")]
pub(crate) struct WayscriberTray {
    control: TrayControl,
    configurator_binary: String,
    session_resume_enabled: bool,
    icon_style: TrayIconStyle,
    overlay_active: Arc<AtomicBool>,
    tray_status: Arc<TrayStatusShared>,
}

#[cfg(feature = "tray")]
impl WayscriberTray {
    fn new(
        control: TrayControl,
        configurator_binary: String,
        session_resume_enabled: bool,
        icon_style: TrayIconStyle,
        overlay_active: Arc<AtomicBool>,
        tray_status: Arc<TrayStatusShared>,
    ) -> Self {
        Self {
            control,
            configurator_binary,
            session_resume_enabled,
            icon_style,
            overlay_active,
            tray_status,
        }
    }

    fn request_toggle(&self) {
        if let Err(error) = self.control.visibility.publish(None, false, "tray toggle") {
            log::warn!("Failed to wake daemon for tray toggle: {error}");
        }
    }

    fn request_quit(&self) {
        if let Err(error) = self.control.quit.raise("tray quit") {
            log::warn!("Failed to wake daemon for tray shutdown: {error}");
        }
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
        let visibility_intents = Arc::new(VisibilityIntents::with_ready(toggle_flag));
        let action_intents = Arc::new(OverlayActionIntents::default());
        Self::new(
            TrayControl {
                visibility: visibility_intents.publisher(control_wake.clone()),
                action: action_intents.publisher(control_wake.clone()),
                quit: DaemonControlEvent::new(quit_flag, control_wake),
            },
            "true".into(),
            session_resume_enabled,
            TrayIconStyle::Auto,
            Arc::new(AtomicBool::new(false)),
            Arc::new(TrayStatusShared::new()),
        )
    }
}
