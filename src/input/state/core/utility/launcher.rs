use super::super::base::{InputState, UiToastKind};
use crate::config::Config;
use std::io::ErrorKind;
use std::process::{Command, Stdio};

impl InputState {
    pub(crate) fn launch_configurator(&mut self) {
        let binary = std::env::var("WAYSCRIBER_CONFIGURATOR")
            .unwrap_or_else(|_| "wayscriber-configurator".to_string());

        match Command::new(&binary)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                log::info!(
                    "Launched wayscriber-configurator (binary: {binary}, pid: {})",
                    child.id()
                );
                self.should_exit = true;
            }
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    log::error!(
                        "Configurator not found (looked for '{binary}'). Install 'wayscriber-configurator' (Arch: yay -S wayscriber-configurator; deb/rpm users: grab the wayscriber-configurator package from the release page) or set WAYSCRIBER_CONFIGURATOR to its path."
                    );
                    self.set_ui_toast(
                        UiToastKind::Warning,
                        format!("Configurator not found: {binary}"),
                    );
                } else {
                    log::error!("Failed to launch wayscriber-configurator using '{binary}': {err}");
                    log::error!(
                        "Set WAYSCRIBER_CONFIGURATOR to override the executable path if needed."
                    );
                    self.set_ui_toast(
                        UiToastKind::Error,
                        "Failed to launch configurator (see logs).",
                    );
                }
            }
        }
    }

    /// Opens the most recent capture directory using the desktop default application.
    pub(crate) fn open_capture_folder(&mut self) {
        let Some(path) = self.last_capture_path.clone() else {
            self.set_ui_toast(UiToastKind::Warning, "No saved capture to open.");
            return;
        };

        let folder = if path.is_dir() {
            path
        } else if let Some(parent) = path.parent() {
            parent.to_path_buf()
        } else {
            self.set_ui_toast(UiToastKind::Warning, "Capture folder is unavailable.");
            return;
        };

        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "xdg-open"
        };

        let mut cmd = Command::new(opener);
        if cfg!(target_os = "windows") {
            cmd.args(["/C", "start", ""]).arg(&folder);
        } else {
            cmd.arg(&folder);
        }

        match cmd.spawn() {
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
                self.set_ui_toast(UiToastKind::Error, "Failed to open capture folder.");
            }
        }
    }

    /// Opens the primary config file using the desktop default application.
    pub(crate) fn open_config_file_default(&mut self) {
        let path = match Config::get_config_path() {
            Ok(p) => p,
            Err(err) => {
                log::error!("Unable to resolve config path: {}", err);
                return;
            }
        };

        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "xdg-open"
        };

        let mut cmd = Command::new(opener);
        if cfg!(target_os = "windows") {
            cmd.args(["/C", "start", ""]).arg(&path);
        } else {
            cmd.arg(&path);
        }

        match cmd.spawn() {
            Ok(child) => {
                log::info!(
                    "Opened config file at {} (pid {})",
                    path.display(),
                    child.id()
                );
                self.should_exit = true;
            }
            Err(err) => {
                log::error!("Failed to open config file at {}: {}", path.display(), err);
            }
        }
    }
}
