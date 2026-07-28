use super::super::base::InputState;
use crate::config::Config;
use crate::configurator_destination::{ConfiguratorDestination, configurator_launch_arguments};
use crate::domain::Action;
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
                    Toast::error("Failed to open About (see logs)."),
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
                // reaches the settings, just not the destination.
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

    /// Say once per run that an overlay preference toggle is a current-run
    /// change and the configurator owns the configured default.
    ///
    /// Once, not per toggle: every authored preference the overlay exposes has
    /// the same scope, so repeating it on each checkbox would be noise. The
    /// hint names the shortcut the user actually has for the configurator, and
    /// drops it when the action is unbound.
    pub(crate) fn notify_process_only_preference(&mut self) {
        if self.process_only_preference_notice_shown {
            return;
        }
        self.process_only_preference_notice_shown = true;
        let message = match self.action_binding_primary_label(Action::OpenConfigurator) {
            Some(binding) => {
                format!("Applies to this run — edit defaults in the configurator ({binding}).")
            }
            None => "Applies to this run — edit defaults in the configurator.".to_string(),
        };
        self.push_toast(
            ToastPriority::Info,
            "config-preference",
            Toast::info(message),
        );
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
