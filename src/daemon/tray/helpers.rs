#[cfg(feature = "tray")]
use crate::config::Config;
#[cfg(feature = "tray")]
use crate::paths::log_dir;
#[cfg(feature = "tray")]
use crate::session::{clear_session, options_from_config};
#[cfg(feature = "tray")]
use log::{error, info, warn};
#[cfg(feature = "tray")]
use std::env;
#[cfg(feature = "tray")]
use std::fs;
#[cfg(feature = "tray")]
use std::io::ErrorKind;
#[cfg(feature = "tray")]
use std::process::{Command, Stdio};
#[cfg(feature = "tray")]
use std::sync::atomic::Ordering;

#[cfg(feature = "tray")]
use super::WayscriberTray;
#[cfg(feature = "tray")]
use crate::daemon::icons::{decode_tray_icon_png, fallback_tray_icon};

#[cfg(feature = "tray")]
impl WayscriberTray {
    pub(super) fn launch_configurator(&self) {
        let mut command = Command::new(&self.configurator_binary);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(child) => {
                info!(
                    "Launched wayscriber-configurator (binary: {}, pid: {})",
                    self.configurator_binary,
                    child.id()
                );
            }
            Err(err) => {
                let not_found = err.kind() == ErrorKind::NotFound;
                if not_found {
                    error!(
                        "Configurator not found (looked for '{}'). Install 'wayscriber-configurator' (Arch: yay -S wayscriber-configurator; deb/rpm users: grab the wayscriber-configurator package from the release page) or set WAYSCRIBER_CONFIGURATOR to its path.",
                        self.configurator_binary
                    );
                } else {
                    error!(
                        "Failed to launch wayscriber-configurator using '{}': {}",
                        self.configurator_binary, err
                    );
                    error!(
                        "Set WAYSCRIBER_CONFIGURATOR to override the executable path if needed."
                    );
                }
                #[cfg(feature = "dbus")]
                {
                    let body = if not_found {
                        "Install wayscriber-configurator or set WAYSCRIBER_CONFIGURATOR to its path."
                    } else {
                        "Failed to launch configurator; see logs for details."
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

        let mut command = Command::new(exe);
        command
            .arg("--about")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(child) => {
                info!("Launched About window (pid {})", child.id());
            }
            Err(err) => {
                warn!("Failed to launch About window: {}", err);
            }
        }
    }

    pub(super) fn dispatch_overlay_action(&self, action: &str) {
        if let Some(parent) = self.tray_action_path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            warn!(
                "Failed to prepare tray action directory {}: {}",
                parent.display(),
                err
            );
            return;
        }

        if let Err(err) = fs::write(&self.tray_action_path, action) {
            warn!(
                "Failed to write tray action {} to {}: {}",
                action,
                self.tray_action_path.display(),
                err
            );
            return;
        }

        let pid = self.overlay_pid.load(Ordering::Acquire);

        #[cfg(unix)]
        {
            if pid != 0 {
                if unsafe { libc::kill(pid as i32, libc::SIGUSR2) } != 0 {
                    warn!(
                        "Failed to signal overlay process {} for tray action {}: {}",
                        pid,
                        action,
                        std::io::Error::last_os_error()
                    );
                }
            } else {
                // Overlay not running; request it to show so the action can run on startup.
                self.toggle_flag.store(true, Ordering::Release);
            }
        }
        #[cfg(not(unix))]
        {
            if pid == 0 {
                self.toggle_flag.store(true, Ordering::Release);
            } else {
                warn!("Tray overlay actions are only supported on Unix platforms");
            }
        }
    }

    pub(super) fn clear_session_files(&self) {
        match Config::load() {
            Ok(loaded) => {
                let config_dir = match Config::config_directory_from_source(&loaded.source) {
                    Ok(dir) => dir,
                    Err(err) => {
                        warn!("Failed to resolve config directory: {}", err);
                        return;
                    }
                };
                match options_from_config(&loaded.config.session, &config_dir, None) {
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
        let dir = log_dir();
        if let Err(err) = fs::create_dir_all(&dir) {
            warn!("Failed to create log directory {}: {}", dir.display(), err);
            return;
        }

        let mut command = Command::new("xdg-open");
        command.arg(&dir);
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());

        match command.spawn() {
            Ok(child) => info!("Opened log directory via xdg-open (pid {})", child.id()),
            Err(err) => warn!("Failed to open log directory {}: {}", dir.display(), err),
        }
    }

    pub(super) fn open_config_file(&self) {
        let path = match Config::get_config_path() {
            Ok(p) => p,
            Err(err) => {
                warn!("Unable to resolve config path: {}", err);
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
            Ok(child) => info!(
                "Opened config file at {} (pid {})",
                path.display(),
                child.id()
            ),
            Err(err) => warn!("Failed to open config file at {}: {}", path.display(), err),
        }
    }

    pub(super) fn tray_icon_pixmap(&self) -> Vec<ksni::Icon> {
        decode_tray_icon_png().unwrap_or_else(fallback_tray_icon)
    }
}
