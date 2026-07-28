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
    icon_style: TrayIconStyle,
    overlay_active: Arc<AtomicBool>,
    tray_status: Arc<TrayStatusShared>,
}

#[cfg(feature = "tray")]
impl WayscriberTray {
    fn new(
        control: TrayControl,
        configurator_binary: String,
        icon_style: TrayIconStyle,
        overlay_active: Arc<AtomicBool>,
        tray_status: Arc<TrayStatusShared>,
    ) -> Self {
        Self {
            control,
            configurator_binary,
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
    pub(crate) fn new_for_tests(toggle_flag: Arc<AtomicBool>, quit_flag: Arc<AtomicBool>) -> Self {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        Self::new_for_tests_with_wake(toggle_flag, quit_flag, wake.handle())
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests_with_wake(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        control_wake: crate::backend::wayland::RuntimeWakeHandle,
    ) -> Self {
        Self::for_tests(toggle_flag, quit_flag, control_wake, "true".into())
    }

    /// A tray whose configurator "binary" is a program the test owns, so what a
    /// menu item asks the configurator to open can be observed rather than
    /// assumed.
    #[cfg(test)]
    pub(crate) fn new_for_tests_with_configurator(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        configurator_binary: String,
    ) -> Self {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        Self::for_tests(toggle_flag, quit_flag, wake.handle(), configurator_binary)
    }

    #[cfg(test)]
    fn for_tests(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        control_wake: crate::backend::wayland::RuntimeWakeHandle,
        configurator_binary: String,
    ) -> Self {
        let visibility_intents = Arc::new(VisibilityIntents::with_ready(toggle_flag));
        let action_intents = Arc::new(OverlayActionIntents::default());
        Self::new(
            TrayControl {
                visibility: visibility_intents.publisher(control_wake.clone()),
                action: action_intents.publisher(control_wake.clone()),
                quit: DaemonControlEvent::new(quit_flag, control_wake),
            },
            configurator_binary,
            TrayIconStyle::Auto,
            Arc::new(AtomicBool::new(false)),
            Arc::new(TrayStatusShared::new()),
        )
    }
}
