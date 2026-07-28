use anyhow::Result;
#[cfg(feature = "tray")]
use anyhow::anyhow;
#[cfg(feature = "tray")]
use ksni::TrayMethods;
use log::info;
#[cfg(feature = "tray")]
use log::{debug, warn};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
#[cfg(feature = "tray")]
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;
#[cfg(feature = "tray")]
use std::time::Duration;
#[cfg(feature = "tray")]
use zbus::{Connection, Proxy};

#[cfg(feature = "tray")]
use super::super::types::TrayStatusShared;
use super::super::types::{DaemonControlEvent, OverlayActionPublisher, VisibilityPublisher};
#[cfg(feature = "tray")]
use super::{TrayControl, WayscriberTray};
#[cfg(feature = "tray")]
use crate::config::{Config, TrayIconStyle};
#[cfg(feature = "tray")]
use crate::env_vars::CONFIGURATOR_ENV;

#[cfg(feature = "tray")]
const TRAY_START_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(feature = "tray")]
const STATUS_NOTIFIER_WATCHER_BUS: &str = "org.kde.StatusNotifierWatcher";
#[cfg(feature = "tray")]
const STATUS_NOTIFIER_WATCHER_PATH: &str = "/StatusNotifierWatcher";

/// Read the settings the tray needs to draw itself.
///
/// This is the whole of the tray's interest in `config.toml`, and it is a read:
/// the daemon has no way to change the file, so a setting the user wants to
/// keep is edited in the configurator and picked up the next time the tray
/// starts. A config that cannot be read is not an error worth failing a tray
/// over — the compiled default draws a usable icon.
#[cfg(feature = "tray")]
fn load_tray_settings_from_config() -> TrayIconStyle {
    match Config::load() {
        Ok(loaded) => loaded.config.tray.icon_style,
        Err(err) => {
            warn!(
                "Failed to read config for tray settings; using safe defaults: {}",
                err
            );
            TrayIconStyle::Auto
        }
    }
}

/// System tray implementation
#[cfg(feature = "tray")]
pub(crate) fn start_system_tray(
    visibility: VisibilityPublisher,
    action: OverlayActionPublisher,
    quit: DaemonControlEvent,
    overlay_active: Arc<AtomicBool>,
    tray_status: Arc<TrayStatusShared>,
) -> Result<JoinHandle<()>> {
    let configurator_binary =
        std::env::var(CONFIGURATOR_ENV).unwrap_or_else(|_| "wayscriber-configurator".to_string());
    let icon_style = load_tray_settings_from_config();

    let tray_quit = quit.clone();
    let tray = WayscriberTray::new(
        TrayControl {
            visibility,
            action,
            quit: quit.clone(),
        },
        configurator_binary,
        icon_style,
        overlay_active,
        tray_status.clone(),
    );
    let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();

    info!("Creating tray service...");
    info!("Spawning system tray runtime thread...");

    let ready_thread_tx = ready_tx.clone();
    let tray_thread = thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(e) => {
                warn!("Failed to create Tokio runtime for system tray: {}", e);
                report_tray_readiness(
                    &ready_thread_tx,
                    Err(anyhow!(
                        "Failed to create Tokio runtime for system tray: {e}"
                    )),
                );
                return;
            }
        };

        rt.block_on(async {
            match tray.assume_sni_available(true).spawn().await {
                Ok(handle) => {
                    info!("System tray spawned successfully");
                    report_tray_readiness(&ready_thread_tx, Ok(()));
                    tokio::spawn(log_status_notifier_state());
                    let mut last_revision = tray_status.revision();

                    // Monitor quit flag and shutdown gracefully
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        let revision = tray_status.revision();
                        if revision != last_revision {
                            if handle.update(|_| {}).await.is_none() {
                                warn!("Tray service closed; stopping tray monitor");
                                break;
                            }
                            last_revision = revision;
                        }
                        if tray_quit.is_raised() {
                            info!("Quit signal received - shutting down system tray");
                            let _ = handle.shutdown().await;
                            break;
                        }
                    }
                }
                Err(e) => {
                    warn!("System tray error: {}", e);
                    report_tray_readiness(&ready_thread_tx, Err(anyhow!("System tray error: {e}")));
                }
            }
        });
    });

    drop(ready_tx);

    info!("Waiting for system tray readiness signal...");
    match ready_rx.recv_timeout(TRAY_START_TIMEOUT) {
        Ok(result) => {
            result?;
            info!("System tray thread started");
            Ok(tray_thread)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            warn!("Timed out waiting for system tray to start");
            if let Err(error) = quit.raise("tray startup timeout") {
                warn!("Failed to wake daemon after tray startup timeout: {error}");
            }
            let _ = tray_thread.join();
            Err(anyhow!("Timed out waiting for system tray to start"))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = tray_thread.join();
            Err(anyhow!(
                "System tray thread exited before signaling readiness"
            ))
        }
    }
}

#[cfg(not(feature = "tray"))]
pub(crate) fn start_system_tray(
    _visibility: VisibilityPublisher,
    _action: OverlayActionPublisher,
    _quit: DaemonControlEvent,
    _overlay_active: Arc<AtomicBool>,
    _tray_status: (),
) -> Result<JoinHandle<()>> {
    info!("Tray feature disabled; skipping system tray startup");
    Ok(thread::spawn(|| ()))
}

#[cfg(feature = "tray")]
fn report_tray_readiness(tx: &mpsc::Sender<Result<()>>, result: Result<()>) {
    if let Err(err) = tx.send(result) {
        debug!(
            "System tray readiness receiver dropped before signal could be delivered: {}",
            err
        );
    }
}

#[cfg(feature = "tray")]
async fn log_status_notifier_state() {
    let conn = match Connection::session().await {
        Ok(conn) => conn,
        Err(err) => {
            warn!(
                "Failed to connect to session D-Bus for tray diagnostics: {}",
                err
            );
            return;
        }
    };

    let proxy = match Proxy::new(
        &conn,
        STATUS_NOTIFIER_WATCHER_BUS,
        STATUS_NOTIFIER_WATCHER_PATH,
        STATUS_NOTIFIER_WATCHER_BUS,
    )
    .await
    {
        Ok(proxy) => proxy,
        Err(err) => {
            warn!("StatusNotifierWatcher unavailable (no tray host?): {}", err);
            return;
        }
    };

    let host_registered: bool = match proxy.get_property("IsStatusNotifierHostRegistered").await {
        Ok(value) => value,
        Err(err) => {
            warn!("Failed to query tray host registration: {}", err);
            return;
        }
    };

    let items: Vec<String> = match proxy.get_property("RegisteredStatusNotifierItems").await {
        Ok(value) => value,
        Err(err) => {
            warn!("Failed to query registered tray items: {}", err);
            return;
        }
    };

    info!(
        "StatusNotifierWatcher ready: host_registered={}, registered_items={}",
        host_registered,
        items.len()
    );
}

#[cfg(all(test, feature = "tray"))]
mod tests {
    use super::*;
    use crate::config::PRIMARY_CONFIG_DIR;
    use crate::config::test_helpers::{ConfigFileSnapshot, with_temp_config_home};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn write_config(config_root: &Path, contents: &str) -> PathBuf {
        let directory = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&directory).expect("the test config directory should be creatable");
        let path = directory.join("config.toml");
        fs::write(&path, contents).expect("the test config should be written");
        path
    }

    /// The icon style the tray draws with is authored in the file, so the tray
    /// has to read it rather than assume the default.
    #[test]
    fn tray_settings_come_from_the_configured_icon_style() {
        with_temp_config_home(|config_root| {
            write_config(config_root, "[tray]\nicon_style = \"colored\"\n");

            assert_eq!(load_tray_settings_from_config(), TrayIconStyle::Colored);
        });
    }

    /// A file that says nothing about the tray is not a broken file: the tray
    /// draws with the compiled default and still leaves the file alone.
    #[test]
    fn tray_settings_fall_back_to_the_default_without_a_config() {
        with_temp_config_home(|_config_root| {
            assert_eq!(load_tray_settings_from_config(), TrayIconStyle::Auto);
        });
    }

    /// Reading is now the whole of the tray's relationship with `config.toml`,
    /// and a read may not leave a trace: no rewrite, no touch, no chmod, and no
    /// backup next to the file the user authored.
    #[test]
    fn loading_tray_settings_leaves_the_config_untouched() {
        with_temp_config_home(|config_root| {
            let path = write_config(
                config_root,
                "# tray notes\n[tray]\nicon_style = \"symbolic\"\n\n[session]\npersist_history = true\n",
            );
            let snapshot = ConfigFileSnapshot::capture(&path);

            assert_eq!(load_tray_settings_from_config(), TrayIconStyle::Symbolic);

            snapshot.assert_unchanged("load_tray_settings_from_config");
        });
    }
}
