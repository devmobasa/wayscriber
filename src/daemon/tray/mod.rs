mod helpers;
#[cfg(feature = "tray")]
mod ksni;
mod runtime;
#[cfg(feature = "tray")]
mod shortcut_hint_io;

pub(crate) use runtime::{TrayCleanupOwner, TrayRuntime, start_system_tray};

use super::types::{DaemonControlEvent, OverlayActionPublisher, VisibilityPublisher};
#[cfg(all(test, feature = "tray"))]
use super::types::{DaemonEventInbox, daemon_event_channel};
#[cfg(feature = "tray")]
use super::types::{TraySnapshot, TrayStatusPublisher};
#[cfg(feature = "tray")]
use crate::config::TrayIconStyle;

pub(super) struct TrayControl {
    pub(super) visibility: VisibilityPublisher,
    pub(super) action: OverlayActionPublisher,
    pub(super) quit: DaemonControlEvent,
}

#[cfg(feature = "tray")]
struct TraySetup {
    configurator_binary: String,
    session_resume_enabled: bool,
    icon_style: TrayIconStyle,
    snapshot: TraySnapshot,
    status_publisher: TrayStatusPublisher,
    process_broker: crate::process_broker::ProcessBrokerHandle,
    config_store: crate::config::ConfigStore,
    path_resolver: crate::paths::PathResolver,
}

#[cfg(feature = "tray")]
pub(crate) struct WayscriberTray {
    control: TrayControl,
    configurator_binary: String,
    session_resume_enabled: bool,
    icon_style: TrayIconStyle,
    snapshot: TraySnapshot,
    status_publisher: TrayStatusPublisher,
    process_broker: crate::process_broker::ProcessBrokerHandle,
    config_store: crate::config::ConfigStore,
    path_resolver: crate::paths::PathResolver,
}

#[cfg(feature = "tray")]
impl WayscriberTray {
    fn new(control: TrayControl, setup: TraySetup) -> Self {
        Self {
            control,
            configurator_binary: setup.configurator_binary,
            session_resume_enabled: setup.session_resume_enabled,
            icon_style: setup.icon_style,
            snapshot: setup.snapshot,
            status_publisher: setup.status_publisher,
            process_broker: setup.process_broker,
            config_store: setup.config_store,
            path_resolver: setup.path_resolver,
        }
    }

    fn apply_snapshot(&mut self, snapshot: TraySnapshot) {
        self.snapshot = snapshot;
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
        session_resume_enabled: bool,
    ) -> (
        Self,
        DaemonEventInbox,
        crate::backend::wayland::RuntimeWakeSource,
        crate::process_broker::ProcessBrokerOwner,
    ) {
        let wake = crate::backend::wayland::RuntimeWakeSource::new()
            .expect("test creates a daemon tray runtime eventfd");
        let (inbox, mut senders) = daemon_event_channel();
        let process_broker = crate::process_broker::start_for_runtime()
            .expect("test starts its explicit process broker owner");
        let tray = Self::new(
            TrayControl {
                visibility: senders.visibility(
                    wake.try_sender()
                        .expect("test duplicates tray visibility wake ownership"),
                ),
                action: senders
                    .overlay_actions(
                        wake.try_sender()
                            .expect("test duplicates tray action wake ownership"),
                    )
                    .expect("fixture claims its only overlay-action publisher"),
                quit: senders.quit(
                    wake.try_sender()
                        .expect("test duplicates tray quit wake ownership"),
                ),
            },
            TraySetup {
                configurator_binary: "true".into(),
                session_resume_enabled,
                icon_style: TrayIconStyle::Auto,
                snapshot: TraySnapshot::default(),
                status_publisher: senders.tray_status(
                    wake.try_sender()
                        .expect("test duplicates tray status wake ownership"),
                ),
                process_broker: process_broker.handle(),
                config_store: crate::config::ConfigStore::at_path(
                    "/tmp/wayscriber-tray-test-config.toml",
                ),
                path_resolver: crate::paths::PathResolver::from_process_environment(),
            },
        );
        (tray, inbox, wake, process_broker)
    }
}
