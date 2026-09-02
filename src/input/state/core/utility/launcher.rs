use super::super::base::InputState;
use crate::config::Config;
use crate::configurator_destination::ConfiguratorDestination;
use crate::input::state::{HelperLaunchRequest, PendingBackendAction, Toast, ToastPriority};

impl InputState {
    /// Requests the About dialog through the backend-owned helper launcher.
    pub(crate) fn launch_about(&mut self) {
        self.set_pending_backend_action(PendingBackendAction::HelperLaunch(
            HelperLaunchRequest::About,
        ));
    }

    /// Requests the configurator through the backend-owned helper launcher.
    pub(crate) fn launch_configurator(&mut self, destination: Option<ConfiguratorDestination>) {
        self.set_pending_backend_action(PendingBackendAction::HelperLaunch(
            HelperLaunchRequest::Configurator(destination),
        ));
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

        log::info!("Queued capture folder open at {}", folder.display());
        self.set_pending_backend_action(PendingBackendAction::DesktopOpen(
            crate::desktop_open::DesktopOpenRequest::CaptureFolder(folder),
        ));
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

        log::info!("Queued config file open at {}", path.display());
        self.set_pending_backend_action(PendingBackendAction::DesktopOpen(
            crate::desktop_open::DesktopOpenRequest::ConfigFile(path),
        ));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configurator_destination::{ConfiguratorDestination, ConfiguratorScreen};
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn helper_launch_requests_remain_ordered_without_exiting_input_state() {
        let mut state = make_test_input_state();
        let destination = ConfiguratorDestination::new(ConfiguratorScreen::Drawing);

        state.launch_about();
        state.launch_configurator(Some(destination.clone()));

        assert!(!state.should_exit);
        assert_eq!(
            state.take_pending_backend_action(),
            Some(PendingBackendAction::HelperLaunch(
                HelperLaunchRequest::About
            ))
        );
        assert_eq!(
            state.take_pending_backend_action(),
            Some(PendingBackendAction::HelperLaunch(
                HelperLaunchRequest::Configurator(Some(destination))
            ))
        );
        assert!(state.take_pending_backend_action().is_none());
    }
}
