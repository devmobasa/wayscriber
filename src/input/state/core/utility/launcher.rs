use super::super::base::InputState;
use crate::config::Config;
use crate::env_vars::CONFIGURATOR_ENV;
use crate::input::state::{Toast, ToastPriority};
use std::ffi::{OsStr, OsString};

fn spawn_detached(
    kind: crate::process_broker::HelperKind,
    program: &OsStr,
    arguments: &[OsString],
) -> anyhow::Result<crate::process_broker::BrokerChild> {
    crate::process_broker::current()?.spawn(
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
    pub(crate) fn launch_configurator(&mut self) {
        let binary = std::env::var(CONFIGURATOR_ENV)
            .unwrap_or_else(|_| "wayscriber-configurator".to_string());

        match spawn_detached(
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
                if self.open_config_file_default() {
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

        let (opener, arguments) = opener_arguments(&folder);
        match spawn_detached(
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

        let (opener, arguments) = opener_arguments(&path);
        match spawn_detached(
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
