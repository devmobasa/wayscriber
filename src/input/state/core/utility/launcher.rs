use super::super::base::InputState;
use crate::config::Config;
use crate::configurator_destination::{ConfiguratorDestination, configurator_launch_arguments};
use crate::env_vars::CONFIGURATOR_ENV;
use crate::input::state::{Toast, ToastPriority};
use std::ffi::{OsStr, OsString};

/// Launch a helper from the Wayland callback thread.
///
/// Deliberately the non-blocking spawn: every launcher here runs inside event
/// dispatch, so waiting for the broker transport would stall input and redraw
/// for the length of an unrelated helper. A contended launch is reported and
/// the user retries.
fn spawn_detached(
    kind: crate::process_broker::HelperKind,
    program: &OsStr,
    arguments: &[OsString],
) -> anyhow::Result<crate::process_broker::BrokerChild> {
    crate::process_broker::current()?.try_spawn(
        kind,
        crate::process_broker::HelperLifetime::DetachedAfterExec,
        program,
        arguments,
        Vec::new(),
    )
}

/// Whether a launch failed only because the broker transport was busy.
///
/// Spawn requests give up rather than stall the event thread behind a long
/// helper, so this is a "try again in a moment", not a broken install.
fn launch_deferred_by_busy_broker(error: &anyhow::Error) -> bool {
    format!("{error:#}").contains(crate::process_broker::BROKER_BUSY)
}

/// Message for a failed launch: retryable when the transport was merely busy.
fn launch_failure_message(error: &anyhow::Error, failed: &'static str) -> &'static str {
    if launch_deferred_by_busy_broker(error) {
        "Busy with another task — try again in a moment."
    } else {
        failed
    }
}

impl InputState {
    /// Open the About dialog, closing the overlay first.
    ///
    /// The overlay is a layer-shell surface above normal windows, so an About
    /// toplevel opened underneath it would be invisible and unfocusable. Exiting
    /// is what the configurator already does for the same reason; in daemon mode
    /// this just returns to the hidden state, so the cost is one toggle.
    pub(crate) fn launch_about(&mut self) {
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(err) => {
                log::error!("Failed to resolve the Wayscriber executable for About: {err}");
                self.push_toast(
                    ToastPriority::Critical,
                    "launcher",
                    Toast::error("Could not locate Wayscriber to open About."),
                );
                return;
            }
        };

        match spawn_detached(
            crate::process_broker::HelperKind::About,
            executable.as_os_str(),
            &["--about".into()],
        ) {
            Ok(child) => {
                log::info!("Launched About window (pid {})", child.id());
                self.should_exit = true;
            }
            Err(err) => {
                log::error!("Failed to launch the About window: {err:#}");
                self.push_toast(
                    ToastPriority::Critical,
                    "launcher",
                    Toast::error(launch_failure_message(
                        &err,
                        "Failed to open About (see logs).",
                    )),
                );
            }
        }
    }

    /// Open the configurator, optionally at the screen for the control the user
    /// just used.
    pub(crate) fn launch_configurator(&mut self, destination: Option<ConfiguratorDestination>) {
        let binary = std::env::var(CONFIGURATOR_ENV)
            .unwrap_or_else(|_| "wayscriber-configurator".to_string());
        let arguments = configurator_launch_arguments(destination.as_ref());

        match spawn_detached(
            crate::process_broker::HelperKind::Configurator,
            OsStr::new(&binary),
            &arguments,
        ) {
            Ok(child) => {
                log::info!(
                    "Launched wayscriber-configurator (binary: {binary}, pid: {})",
                    child.id()
                );
                self.should_exit = true;
            }
            Err(err) => {
                log::error!("Failed to launch wayscriber-configurator using '{binary}': {err:#}");
                log::error!("Set {CONFIGURATOR_ENV} to override the executable path if needed.");
                // The fallback hands the file to the desktop's default editor,
                // which has no concept of a configurator screen: the user still
                // reaches the settings, just not the destination. A busy
                // transport is not the configurator being unavailable, though —
                // falling back there would open a text editor for what is really
                // "try again", and would take a second run at the same busy
                // transport to do it.
                if !launch_deferred_by_busy_broker(&err) && self.open_config_file_default() {
                    log::info!(
                        "Opened config file with default application because wayscriber-configurator was unavailable"
                    );
                } else {
                    self.push_toast(
                        ToastPriority::Critical,
                        "launcher",
                        Toast::error(launch_failure_message(
                            &err,
                            "Failed to launch configurator (see logs).",
                        )),
                    );
                }
            }
        }
    }

    /// Opens the most recent capture directory using the desktop default application.
    pub(crate) fn open_capture_folder(&mut self) {
        let Some(path) = self.last_capture_path.clone() else {
            self.push_toast(
                ToastPriority::Info,
                "launcher",
                Toast::warning("No saved capture to open."),
            );
            return;
        };

        let folder = if path.is_dir() {
            path
        } else if let Some(parent) = path.parent() {
            parent.to_path_buf()
        } else {
            self.push_toast(
                ToastPriority::Info,
                "launcher",
                Toast::warning("Capture folder is unavailable."),
            );
            return;
        };

        let invocation = crate::desktop_open::path(&folder);
        match spawn_detached(
            crate::process_broker::HelperKind::DesktopOpen,
            invocation.program(),
            invocation.arguments(),
        ) {
            Ok(child) => {
                log::info!(
                    "Opened capture folder at {} (pid {})",
                    folder.display(),
                    child.id()
                );
                self.should_exit = true;
            }
            Err(err) => {
                log::error!(
                    "Failed to open capture folder at {}: {}",
                    folder.display(),
                    err
                );
                self.push_toast(
                    ToastPriority::Critical,
                    "launcher",
                    Toast::error(launch_failure_message(
                        &err,
                        "Failed to open capture folder.",
                    )),
                );
            }
        }
    }

    /// Opens the primary config file using the desktop default application.
    pub(crate) fn open_config_file_default(&mut self) -> bool {
        let path = match Config::get_config_path() {
            Ok(p) => p,
            Err(err) => {
                log::error!("Unable to resolve config path: {}", err);
                self.push_toast(
                    ToastPriority::Critical,
                    "launcher",
                    Toast::error("Unable to resolve config path."),
                );
                return false;
            }
        };

        let invocation = crate::desktop_open::path(&path);
        match spawn_detached(
            crate::process_broker::HelperKind::DesktopOpen,
            invocation.program(),
            invocation.arguments(),
        ) {
            Ok(child) => {
                log::info!(
                    "Opened config file at {} (pid {})",
                    path.display(),
                    child.id()
                );
                self.should_exit = true;
                true
            }
            Err(err) => {
                log::error!("Failed to open config file at {}: {}", path.display(), err);
                self.push_toast(
                    ToastPriority::Critical,
                    "launcher",
                    Toast::error(launch_failure_message(&err, "Failed to open config file.")),
                );
                false
            }
        }
    }
}
