//! Backend-owned launch of the About window and configurator.
//!
//! Input records semantic intent only. The backend resolves executables and
//! talks to the process broker because both are runtime side effects.

use std::env;
use std::ffi::{OsStr, OsString};

use crate::process_broker::{BrokerChild, BrokerErrorKind, HelperKind, HelperLifetime};
use crate::{about_window, process_broker};

use super::WaylandState;
use crate::configurator_destination::{ConfiguratorDestination, configurator_launch_arguments};
use crate::env_vars::CONFIGURATOR_ENV;
use crate::i18n::tr;
use crate::input::state::{HelperLaunchRequest, Toast, ToastPriority};

fn spawn_detached(
    kind: HelperKind,
    program: &OsStr,
    arguments: &[OsString],
) -> anyhow::Result<BrokerChild> {
    process_broker::current()?.try_spawn(
        kind,
        HelperLifetime::DetachedAfterExec,
        program,
        arguments,
        Vec::new(),
    )
}

fn launch_deferred_by_busy_broker(error: &anyhow::Error) -> bool {
    process_broker::error_kind(error) == Some(BrokerErrorKind::Busy)
}

fn launch_failure_message(error: &anyhow::Error, failed: &'static str) -> &'static str {
    if launch_deferred_by_busy_broker(error) {
        tr("Busy with another task. Try again in a moment.")
    } else {
        failed
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn handle_helper_launch(
        &mut self,
        request: HelperLaunchRequest,
    ) {
        match request {
            HelperLaunchRequest::About => self.launch_about_helper(),
            HelperLaunchRequest::Configurator(destination) => {
                self.launch_configurator_helper(destination.as_ref());
            }
        }
    }

    fn launch_about_helper(&mut self) {
        let executable = match env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                log::error!("Failed to resolve the Wayscriber executable for About: {error}");
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    "launcher",
                    Toast::error(tr("Could not locate Wayscriber to open About.")),
                );
                return;
            }
        };

        match spawn_detached(
            HelperKind::About,
            executable.as_os_str(),
            &about_window::launch_arguments(),
        ) {
            Ok(child) => {
                log::info!("Launched About window (pid {})", child.id());
                self.input_state.should_exit = true;
            }
            Err(error) => {
                log::error!("Failed to launch the About window: {error:#}");
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    "launcher",
                    Toast::error(launch_failure_message(
                        &error,
                        tr("Failed to open About (see logs)."),
                    )),
                );
            }
        }
    }

    fn launch_configurator_helper(&mut self, destination: Option<&ConfiguratorDestination>) {
        let binary =
            env::var(CONFIGURATOR_ENV).unwrap_or_else(|_| "wayscriber-configurator".to_string());
        let arguments = configurator_launch_arguments(destination);

        match spawn_detached(HelperKind::Configurator, OsStr::new(&binary), &arguments) {
            Ok(child) => {
                log::info!(
                    "Launched wayscriber-configurator (binary: {binary}, pid: {})",
                    child.id()
                );
                self.input_state.should_exit = true;
            }
            Err(error) => {
                log::error!("Failed to launch wayscriber-configurator using '{binary}': {error:#}");
                log::error!("Set {CONFIGURATOR_ENV} to override the executable path if needed.");
                if !launch_deferred_by_busy_broker(&error)
                    && self.input_state.open_config_file_default()
                {
                    log::info!(
                        "Queued config file for the default application because wayscriber-configurator was unavailable"
                    );
                } else {
                    self.input_state.push_toast(
                        ToastPriority::Critical,
                        "launcher",
                        Toast::error(launch_failure_message(
                            &error,
                            tr("Failed to launch configurator (see logs)."),
                        )),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_launch_failure_keeps_specific_notice() {
        let error = anyhow::anyhow!(
            "unrelated diagnostic: process broker is busy running another helper; No such file"
        )
        .context("launch context");

        assert!(!launch_deferred_by_busy_broker(&error));
        assert_eq!(
            launch_failure_message(&error, "Failed to open About (see logs)."),
            "Failed to open About (see logs)."
        );
    }
}
