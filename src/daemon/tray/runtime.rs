use anyhow::Result;
use log::info;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

#[cfg(feature = "tray")]
use anyhow::anyhow;
#[cfg(feature = "tray")]
use ksni::TrayMethods;
#[cfg(feature = "tray")]
use log::{debug, warn};
#[cfg(feature = "tray")]
use std::sync::atomic::Ordering;
#[cfg(feature = "tray")]
use std::sync::mpsc;
#[cfg(feature = "tray")]
use zbus::{Connection, Proxy};

#[cfg(feature = "tray")]
use crate::config::Config;
#[cfg(feature = "tray")]
use crate::paths::tray_action_file;

#[cfg(feature = "tray")]
use super::super::types::TrayStatusShared;
#[cfg(feature = "tray")]
use super::WayscriberTray;

#[cfg(feature = "tray")]
const TRAY_START_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(feature = "tray")]
fn load_session_resume_enabled_from_config() -> bool {
    match Config::load() {
        Ok(loaded) => {
            let session = loaded.config.session;
            session.persist_transparent
                || session.persist_whiteboard
                || session.persist_blackboard
                || session.persist_history
                || session.restore_tool_state
        }
        Err(err) => {
            warn!(
                "Failed to read config for session resume state; assuming disabled: {}",
                err
            );
            false
        }
    }
}

#[cfg(feature = "tray")]
pub(super) fn update_session_resume_in_config(target_enabled: bool, fallback: bool) -> bool {
    match Config::load() {
        Ok(loaded) => {
            let mut config = loaded.config;
            config.session.persist_transparent = target_enabled;
            config.session.persist_whiteboard = target_enabled;
            config.session.persist_blackboard = target_enabled;
            config.session.persist_history = target_enabled;
            config.session.restore_tool_state = target_enabled;
            if let Err(err) = config.save() {
                warn!(
                    "Failed to write session resume setting to config (desired {}): {}",
                    target_enabled, err
                );
                fallback
            } else {
                target_enabled
            }
        }
        Err(err) => {
            warn!(
                "Failed to load config while toggling session resume (desired {}): {}",
                target_enabled, err
            );
            fallback
        }
    }
}

/// System tray implementation
#[cfg(feature = "tray")]
pub(crate) fn start_system_tray(
    toggle_flag: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
    overlay_pid: Arc<AtomicU32>,
    tray_status: Arc<TrayStatusShared>,
) -> Result<JoinHandle<()>> {
    let configurator_binary = std::env::var("WAYSCRIBER_CONFIGURATOR")
        .unwrap_or_else(|_| "wayscriber-configurator".to_string());
    let session_resume_enabled = load_session_resume_enabled_from_config();

    let tray_quit_flag = quit_flag.clone();
    let tray = WayscriberTray::new(
        toggle_flag,
        tray_quit_flag.clone(),
        configurator_binary,
        session_resume_enabled,
        overlay_pid,
        tray_action_file(),
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
                        if tray_quit_flag.load(Ordering::Acquire) {
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
            quit_flag.store(true, Ordering::Release);
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
    _toggle_flag: Arc<AtomicBool>,
    _quit_flag: Arc<AtomicBool>,
    _overlay_pid: Arc<AtomicU32>,
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
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
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
