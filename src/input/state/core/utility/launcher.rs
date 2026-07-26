use super::super::base::InputState;
use crate::env_vars::CONFIGURATOR_ENV;
use crate::input::state::{Toast, ToastPriority};
use std::ffi::{OsStr, OsString};

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

impl InputState {
    /// Open the About dialog, closing the overlay first.
    ///
    /// The overlay is a layer-shell surface above normal windows, so an About
    /// toplevel opened underneath it would be invisible and unfocusable. Exiting
    /// is what the configurator already does for the same reason; in daemon mode
    /// this just returns to the hidden state, so the cost is one toggle.
    pub(crate) fn launch_about(
        &mut self,
        process_broker: &crate::process_broker::ProcessBrokerHandle,
    ) {
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
            process_broker,
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
                    Toast::error("Failed to open About (see logs)."),
                );
            }
        }
    }

    pub(crate) fn launch_configurator(
        &mut self,
        process_broker: &crate::process_broker::ProcessBrokerHandle,
        config_path: &std::path::Path,
    ) {
        let binary = std::env::var(CONFIGURATOR_ENV)
            .unwrap_or_else(|_| "wayscriber-configurator".to_string());

        match spawn_detached(
            process_broker,
            crate::process_broker::HelperKind::Configurator,
            OsStr::new(&binary),
            &[],
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
                if self.open_config_file_default(process_broker, config_path) {
                    log::info!(
                        "Opened config file with default application because wayscriber-configurator was unavailable"
                    );
                } else {
                    self.push_toast(
                        ToastPriority::Critical,
                        "launcher",
                        Toast::error("Failed to launch configurator (see logs)."),
                    );
                }
            }
        }
    }

    /// Opens the most recent capture directory using the desktop default application.
    pub(crate) fn open_capture_folder(
        &mut self,
        process_broker: &crate::process_broker::ProcessBrokerHandle,
    ) {
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

        let (opener, arguments) = opener_arguments(&folder);
        match spawn_detached(
            process_broker,
            crate::process_broker::HelperKind::DesktopOpen,
            &opener,
            &arguments,
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
                    Toast::error("Failed to open capture folder."),
                );
            }
        }
    }

    /// Opens the primary config file using the desktop default application.
    pub(crate) fn open_config_file_default(
        &mut self,
        process_broker: &crate::process_broker::ProcessBrokerHandle,
        path: &std::path::Path,
    ) -> bool {
        let (opener, arguments) = opener_arguments(path);
        match spawn_detached(
            process_broker,
            crate::process_broker::HelperKind::DesktopOpen,
            &opener,
            &arguments,
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
                    Toast::error("Failed to open config file."),
                );
                false
            }
        }
    }
}
