#[cfg(feature = "tray")]
use super::WayscriberTray;
#[cfg(feature = "tray")]
use crate::about_window;
#[cfg(feature = "tray")]
use crate::config::Config;
#[cfg(feature = "tray")]
use crate::configurator_destination::{ConfiguratorDestination, configurator_launch_arguments};
#[cfg(feature = "tray")]
use crate::daemon::icons::{decode_tray_icon_png, fallback_tray_icon};
#[cfg(feature = "tray")]
use crate::desktop_open;
#[cfg(feature = "tray")]
use crate::env_vars::CONFIGURATOR_ENV;
#[cfg(feature = "tray")]
use crate::i18n::tr;
#[cfg(feature = "tray")]
use crate::notification;
#[cfg(feature = "tray")]
use crate::paths::log_dir;
#[cfg(feature = "tray")]
use crate::process_broker;
#[cfg(feature = "tray")]
use crate::process_broker::BrokerChild;
#[cfg(feature = "tray")]
use crate::process_broker::HelperKind;
#[cfg(feature = "tray")]
use crate::process_broker::HelperLifetime;
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
use tokio::runtime::Handle;
#[cfg(feature = "tray")]
use tokio::runtime::Runtime;

#[cfg(feature = "tray")]
fn spawn_detached(
    kind: HelperKind,
    program: &OsStr,
    arguments: &[OsString],
) -> anyhow::Result<BrokerChild> {
    process_broker::current()?.spawn(
        kind,
        HelperLifetime::DetachedAfterExec,
        program,
        arguments,
        Vec::new(),
    )
}

/// Notification body when the configurator binary cannot be launched.
#[cfg(all(feature = "tray", feature = "dbus"))]
fn configurator_fallback_body(opened_config: bool) -> &'static str {
    if opened_config {
        tr("Configurator not found; opened config.toml with the default application.")
    } else {
        tr("Configurator not found, and config.toml could not be opened.")
    }
}

#[cfg(feature = "tray")]
impl WayscriberTray {
    /// Open the configurator, optionally at the screen for the menu item used.
    pub(super) fn launch_configurator(&self, destination: Option<&ConfiguratorDestination>) {
        let arguments = configurator_launch_arguments(destination);
        match spawn_detached(
            HelperKind::Configurator,
            OsStr::new(&self.configurator_binary),
            &arguments,
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
                // The fallback hands the file to the desktop's default editor,
                // which has no concept of a configurator screen: the user still
                // reaches the settings, just not the destination.
                let opened_config = self.open_config_file();
                #[cfg(feature = "dbus")]
                {
                    let body = configurator_fallback_body(opened_config);
                    match Handle::try_current() {
                        Ok(handle) => notification::send_notification_async(
                            &handle,
                            tr("Configurator unavailable").to_string(),
                            body.to_string(),
                            Some("dialog-error".to_string()),
                        ),
                        Err(_) => {
                            if let Ok(rt) = Runtime::new() {
                                let _ = rt.block_on(notification::send_notification(
                                    tr("Configurator unavailable"),
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
                warn!("Failed to resolve current executable for About window: {err}");
                return;
            }
        };

        match spawn_detached(
            HelperKind::About,
            exe.as_os_str(),
            &about_window::launch_arguments(),
        ) {
            Ok(child) => {
                info!("Launched About window (pid {})", child.id());
            }
            Err(err) => {
                warn!("Failed to launch About window: {err}");
            }
        }
    }

    /// Open the update instructions the watcher recorded. The URL comes from
    /// `update_check`, which only ever hands out validated wayscriber.com links.
    pub(super) fn open_update_instructions(&self, url: &str) {
        let invocation = match desktop_open::trusted_url(url) {
            Ok(invocation) => invocation,
            Err(err) => {
                warn!("Refused to open update instructions {url:?}: {err:#}");
                return;
            }
        };
        match desktop_open::open_in_background(invocation) {
            Ok(_worker) => info!("Opening update instructions"),
            Err(err) => warn!("Failed to open update instructions {url}: {err}"),
        }
    }

    pub(super) fn dispatch_overlay_action(&self, action: TrayAction) {
        let action_str = action.as_str();
        // Tray producers carry only an action intent. The daemon controller
        // resolves the current child generation and owns any signal decision.
        match self.control.action.publish(action) {
            Ok(()) => {}
            Err(super::super::types::OverlayActionPublishError::QueueFull) => {
                warn!("Failed to queue tray action {action_str}: daemon intent queue is full");
            }
            Err(super::super::types::OverlayActionPublishError::Wake(error)) => {
                warn!("Queued tray action {action_str}, but failed to wake daemon: {error}");
            }
        }
    }

    pub(super) fn clear_session_files(&self) {
        match Config::load() {
            Ok(loaded) => {
                let config_dir = match Config::config_directory_from_source(&loaded.source) {
                    Ok(dir) => dir,
                    Err(err) => {
                        warn!("Failed to resolve config directory: {err}");
                        return;
                    }
                };
                match options_from_config(&loaded.config.session, &config_dir, None) {
                    Ok(opts) => match clear_session(&opts) {
                        Ok(outcome) => {
                            info!("Cleared session files: {outcome:?}");
                        }
                        Err(err) => warn!("Failed to clear session files: {err}"),
                    },
                    Err(err) => warn!("Failed to build session options: {err}"),
                }
            }
            Err(err) => warn!("Failed to load config for clearing session: {err}"),
        }
    }

    pub(super) fn open_log_folder(&self) {
        let dir = log_dir();
        if let Err(err) = fs::create_dir_all(&dir) {
            warn!("Failed to create log directory {}: {}", dir.display(), err);
            return;
        }

        let invocation = desktop_open::path(&dir);
        match desktop_open::open_in_background(invocation) {
            Ok(_worker) => info!("Opening log directory via desktop integration"),
            Err(err) => warn!("Failed to open log directory {}: {}", dir.display(), err),
        }
    }

    pub(super) fn open_config_file(&self) -> bool {
        let path = match Config::config_path() {
            Ok(p) => p,
            Err(err) => {
                warn!("Unable to resolve config path: {err}");
                return false;
            }
        };

        let invocation = desktop_open::path(&path);
        match desktop_open::open_in_background(invocation) {
            Ok(_worker) => {
                info!("Opening config file at {}", path.display());
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
