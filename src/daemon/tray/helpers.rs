#[cfg(feature = "tray")]
use super::WayscriberTray;
#[cfg(feature = "tray")]
use crate::daemon::icons::{decode_tray_icon_png, fallback_tray_icon};
#[cfg(feature = "tray")]
use crate::env_vars::CONFIGURATOR_ENV;
#[cfg(feature = "tray")]
use crate::session::{clear_session, options_from_config};
#[cfg(feature = "tray")]
use crate::tray_action::TrayAction;
#[cfg(feature = "tray")]
use log::{error, info, warn};
#[cfg(feature = "tray")]
use std::env;
#[cfg(feature = "tray")]
use std::ffi::{OsStr, OsString};
#[cfg(feature = "tray")]
use std::fs;

#[cfg(feature = "tray")]
fn spawn_detached(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    kind: crate::process_broker::HelperKind,
    program: &OsStr,
    arguments: &[OsString],
) -> anyhow::Result<crate::process_broker::BrokerChild> {
    process_broker.spawn(
        kind,
        crate::process_broker::HelperLifetime::DetachedAfterExec,
        program,
        arguments,
        Vec::new(),
    )
}

#[cfg(feature = "tray")]
fn opener_arguments(path: &std::path::Path) -> (OsString, Vec<OsString>) {
    if cfg!(target_os = "macos") {
        ("open".into(), vec![path.as_os_str().into()])
    } else if cfg!(target_os = "windows") {
        (
            "cmd".into(),
            vec![
                "/C".into(),
                "start".into(),
                "".into(),
                path.as_os_str().into(),
            ],
        )
    } else {
        ("xdg-open".into(), vec![path.as_os_str().into()])
    }
}

#[cfg(feature = "tray")]
impl WayscriberTray {
    pub(super) fn launch_configurator(&self) {
        match spawn_detached(
            &self.process_broker,
            crate::process_broker::HelperKind::Configurator,
            OsStr::new(&self.configurator_binary),
            &[],
        ) {
            Ok(child) => {
                info!(
                    "Launched wayscriber-configurator (binary: {}, pid: {})",
                    self.configurator_binary,
                    child.id()
                );
            }
            Err(err) => {
                error!(
                    "Failed to launch wayscriber-configurator using '{}': {err:#}",
                    self.configurator_binary
                );
                error!("Set {CONFIGURATOR_ENV} to override the executable path if needed.");
                let opened_config = self.open_config_file();
                #[cfg(feature = "dbus")]
                {
                    let body = if opened_config {
                        "Configurator not found; opened config.toml with the default application."
                    } else {
                        "Configurator not found, and config.toml could not be opened."
                    };
                    match tokio::runtime::Handle::try_current() {
                        Ok(handle) => crate::notification::send_notification_async(
                            &handle,
                            "Configurator unavailable".to_string(),
                            body.to_string(),
                            Some("dialog-error".to_string()),
                        ),
                        Err(_) => {
                            if let Ok(rt) = tokio::runtime::Runtime::new() {
                                let _ = rt.block_on(crate::notification::send_notification(
                                    "Configurator unavailable",
                                    body,
                                    Some("dialog-error"),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn launch_about(&self) {
        let exe = match env::current_exe() {
            Ok(path) => path,
            Err(err) => {
                warn!(
                    "Failed to resolve current executable for About window: {}",
                    err
                );
                return;
            }
        };

        match spawn_detached(
            &self.process_broker,
            crate::process_broker::HelperKind::About,
            exe.as_os_str(),
            &["--about".into()],
        ) {
            Ok(child) => {
                info!("Launched About window (pid {})", child.id());
            }
            Err(err) => {
                warn!("Failed to launch About window: {}", err);
            }
        }
    }

    /// Open the update instructions the watcher recorded. The URL comes from
    /// `update_check`, which only ever hands out validated wayscriber.com links.
    pub(super) fn open_update_instructions(&self, url: &str) {
        match spawn_detached(
            &self.process_broker,
            crate::process_broker::HelperKind::DesktopOpen,
            OsStr::new("xdg-open"),
            &[OsString::from(url)],
        ) {
            Ok(child) => info!("Opened update instructions {url} (pid {})", child.id()),
            Err(err) => warn!("Failed to open update instructions {url}: {err}"),
        }
    }

    pub(super) fn dispatch_overlay_action(&mut self, action: TrayAction) {
        let action_str = action.as_str();
        // Tray producers carry only an action intent. The daemon controller
        // resolves the current child generation and owns any signal decision.
        match self.control.action.publish(action) {
            Ok(()) => {}
            Err(super::super::types::OverlayActionPublishError::QueueFull) => {
                warn!(
                    "Failed to queue tray action {}: daemon intent queue is full",
                    action_str
                );
            }
            Err(super::super::types::OverlayActionPublishError::Wake(error)) => {
                warn!("Queued tray action {action_str}, but failed to wake daemon: {error}");
            }
            Err(super::super::types::OverlayActionPublishError::Disconnected) => {
                warn!("Failed to queue tray action {action_str}: daemon owner disconnected");
            }
            Err(super::super::types::OverlayActionPublishError::InvalidCapacity) => {
                warn!("Failed to queue tray action {action_str}: invalid capacity release");
            }
        }
    }

    pub(super) fn clear_session_files(&self) {
        match self.config_store.load() {
            Ok(loaded) => {
                let config_dir = match self.config_store.config_directory() {
                    Ok(dir) => dir,
                    Err(err) => {
                        warn!("Failed to resolve config directory: {}", err);
                        return;
                    }
                };
                match options_from_config(
                    &loaded.config.session,
                    &config_dir,
                    None,
                    &self.path_resolver,
                ) {
                    Ok(opts) => match clear_session(&opts) {
                        Ok(outcome) => {
                            info!("Cleared session files: {:?}", outcome);
                        }
                        Err(err) => warn!("Failed to clear session files: {}", err),
                    },
                    Err(err) => warn!("Failed to build session options: {}", err),
                }
            }
            Err(err) => warn!("Failed to load config for clearing session: {}", err),
        }
    }

    pub(super) fn open_log_folder(&self) {
        let dir = match self.path_resolver.log_dir() {
            Ok(dir) => dir,
            Err(error) => {
                warn!("Unable to resolve log directory: {error}");
                return;
            }
        };
        if let Err(err) = fs::create_dir_all(&dir) {
            warn!("Failed to create log directory {}: {}", dir.display(), err);
            return;
        }

        match spawn_detached(
            &self.process_broker,
            crate::process_broker::HelperKind::DesktopOpen,
            OsStr::new("xdg-open"),
            &[dir.as_os_str().into()],
        ) {
            Ok(child) => info!("Opened log directory via xdg-open (pid {})", child.id()),
            Err(err) => warn!("Failed to open log directory {}: {}", dir.display(), err),
        }
    }

    pub(super) fn open_config_file(&self) -> bool {
        let path = self.config_store.config_path();

        let (opener, arguments) = opener_arguments(path);
        match spawn_detached(
            &self.process_broker,
            crate::process_broker::HelperKind::DesktopOpen,
            &opener,
            &arguments,
        ) {
            Ok(child) => {
                info!(
                    "Opened config file at {} (pid {})",
                    path.display(),
                    child.id()
                );
                true
            }
            Err(err) => {
                warn!("Failed to open config file at {}: {}", path.display(), err);
                false
            }
        }
    }

    pub(super) fn tray_icon_pixmap(&self) -> Vec<ksni::Icon> {
        decode_tray_icon_png().unwrap_or_else(fallback_tray_icon)
    }
}
