use anyhow::{Context, Result, anyhow};
#[cfg(feature = "tray")]
use ksni::TrayMethods;
use log::{debug, info, warn};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
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

const TRAY_START_TIMEOUT: Duration = Duration::from_secs(5);
const TRAY_STOP_TIMEOUT: Duration = Duration::from_secs(1);
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
) -> Result<TrayRuntime> {
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
    info!("Creating tray service...");
    info!("Spawning system tray runtime thread...");

    let runtime = TrayRuntime::start(
        move |worker| {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(e) => {
                    warn!("Failed to create Tokio runtime for system tray: {}", e);
                    worker.report_readiness(Err(anyhow!(
                        "Failed to create Tokio runtime for system tray: {e}"
                    )));
                    return;
                }
            };

            rt.block_on(async {
                let spawn = tray.assume_sni_available(true).spawn();
                tokio::pin!(spawn);
                let handle = tokio::select! {
                    result = &mut spawn => match result {
                        Ok(handle) => handle,
                        Err(e) => {
                            warn!("System tray error: {}", e);
                            worker.report_readiness(Err(anyhow!("System tray error: {e}")));
                            return;
                        }
                    },
                    () = worker.cancelled() => {
                        debug!("System tray startup cancelled");
                        return;
                    }
                };

                info!("System tray spawned successfully");
                worker.report_readiness(Ok(()));
                tokio::spawn(log_status_notifier_state());
                let mut last_revision = tray_status.revision();

                // Monitor daemon shutdown and tray-local cancellation gracefully.
                loop {
                    tokio::select! {
                        () = worker.cancelled() => {
                            info!("Tray-local stop requested - shutting down system tray");
                            let _ = handle.shutdown().await;
                            break;
                        }
                        () = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
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
                }
            });
        },
        TRAY_START_TIMEOUT,
        TRAY_STOP_TIMEOUT,
    )?;

    info!("System tray thread started");
    Ok(runtime)
}

#[cfg(not(feature = "tray"))]
pub(crate) fn start_system_tray(
    _visibility: VisibilityPublisher,
    _action: OverlayActionPublisher,
    _quit: DaemonControlEvent,
    _overlay_active: Arc<AtomicBool>,
    _tray_status: (),
) -> Result<TrayRuntime> {
    info!("Tray feature disabled; skipping system tray startup");
    TrayRuntime::start(
        |worker| {
            debug_assert!(!worker.is_cancelled());
            worker.report_readiness(Ok(()));
        },
        TRAY_START_TIMEOUT,
        TRAY_STOP_TIMEOUT,
    )
}

struct TrayWorker {
    readiness: mpsc::Sender<Result<()>>,
    cancelled: Arc<AtomicBool>,
}

impl TrayWorker {
    fn report_readiness(&self, result: Result<()>) {
        if let Err(err) = self.readiness.send(result) {
            debug!(
                "System tray readiness receiver dropped before signal could be delivered: {}",
                err
            );
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    #[cfg(feature = "tray")]
    async fn cancelled(&self) {
        while !self.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

struct ThreadCompletion(mpsc::Sender<()>);

impl Drop for ThreadCompletion {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayShutdownOutcome {
    Joined,
    Panicked,
    TimedOut,
}

/// Daemon-side owner of the tray thread and its tray-local stop signal.
pub(crate) struct TrayRuntime {
    cancelled: Arc<AtomicBool>,
    completion: mpsc::Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

impl TrayRuntime {
    fn start(
        worker: impl FnOnce(TrayWorker) + Send + 'static,
        startup_timeout: Duration,
        shutdown_timeout: Duration,
    ) -> Result<Self> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let (readiness_tx, readiness_rx) = mpsc::channel();
        let (completion_tx, completion_rx) = mpsc::channel();
        let worker_cancelled = Arc::clone(&cancelled);
        let thread = thread::Builder::new()
            .name("wayscriber-tray".to_string())
            .spawn(move || {
                let _completion = ThreadCompletion(completion_tx);
                worker(TrayWorker {
                    readiness: readiness_tx,
                    cancelled: worker_cancelled,
                });
            })
            .context("Failed to spawn system tray runtime thread")?;
        let mut runtime = Self {
            cancelled,
            completion: completion_rx,
            thread: Some(thread),
        };

        info!("Waiting for system tray readiness signal...");
        match readiness_rx.recv_timeout(startup_timeout) {
            Ok(Ok(())) => Ok(runtime),
            Ok(Err(error)) => {
                runtime.finish_after_startup_failure(shutdown_timeout);
                Err(error)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                warn!("Timed out waiting for system tray to start");
                runtime.finish_after_startup_failure(shutdown_timeout);
                Err(anyhow!("Timed out waiting for system tray to start"))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                runtime.finish_after_startup_failure(shutdown_timeout);
                Err(anyhow!(
                    "System tray thread exited before signaling readiness"
                ))
            }
        }
    }

    fn finish_after_startup_failure(&mut self, timeout: Duration) {
        match self.stop_within(timeout) {
            TrayShutdownOutcome::Joined => {}
            TrayShutdownOutcome::Panicked => {
                warn!("System tray thread panicked while startup failure was being contained");
            }
            TrayShutdownOutcome::TimedOut => {
                warn!(
                    "System tray thread did not stop within {:?} after startup failure; detaching it",
                    timeout
                );
            }
        }
    }

    fn stop_within(&mut self, timeout: Duration) -> TrayShutdownOutcome {
        self.cancelled.store(true, Ordering::Release);
        let Some(handle) = self.thread.as_ref() else {
            return TrayShutdownOutcome::Joined;
        };
        let deadline = Instant::now() + timeout;
        let _ = self
            .completion
            .recv_timeout(deadline.saturating_duration_since(Instant::now()));
        while !handle.is_finished() && Instant::now() < deadline {
            thread::yield_now();
        }
        if !handle.is_finished() {
            let _ = self.thread.take();
            return TrayShutdownOutcome::TimedOut;
        }
        match self
            .thread
            .take()
            .expect("finished tray thread handle remains owned")
            .join()
        {
            Ok(()) => TrayShutdownOutcome::Joined,
            Err(_) => TrayShutdownOutcome::Panicked,
        }
    }

    pub(crate) fn shutdown(mut self) {
        match self.stop_within(TRAY_STOP_TIMEOUT) {
            TrayShutdownOutcome::Joined => info!("System tray thread joined"),
            TrayShutdownOutcome::Panicked => warn!("System tray thread panicked"),
            TrayShutdownOutcome::TimedOut => warn!(
                "System tray thread did not stop within {:?}; detaching it",
                TRAY_STOP_TIMEOUT
            ),
        }
    }
}

impl Drop for TrayRuntime {
    fn drop(&mut self) {
        if self.thread.is_some() {
            let outcome = self.stop_within(TRAY_STOP_TIMEOUT);
            if outcome != TrayShutdownOutcome::Joined {
                warn!("System tray thread drop ended with {outcome:?}");
            }
        }
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
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    fn write_config(config_root: &Path, contents: &str) -> PathBuf {
        let directory = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&directory).expect("the test config directory should be creatable");
        let path = directory.join("config.toml");
        fs::write(&path, contents).expect("the test config should be written");
        path
    }

    #[test]
    fn tray_startup_timeout_is_bounded_when_worker_stalls() {
        let (release_tx, release_rx) = mpsc::channel();
        let started = Instant::now();

        let result = TrayRuntime::start(
            move |_worker| {
                let _ = release_rx.recv();
            },
            Duration::from_millis(10),
            Duration::from_millis(10),
        );

        assert!(result.is_err());
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "tray timeout must remain bounded"
        );
        release_tx.send(()).unwrap();
    }

    #[test]
    fn late_tray_readiness_observes_local_cancellation() {
        let cancellation_seen = Arc::new(AtomicBool::new(false));
        let worker_cancellation_seen = Arc::clone(&cancellation_seen);

        let result = TrayRuntime::start(
            move |worker| {
                while !worker.is_cancelled() {
                    thread::yield_now();
                }
                worker_cancellation_seen.store(true, Ordering::Release);
                worker.report_readiness(Ok(()));
            },
            Duration::from_millis(10),
            Duration::from_secs(1),
        );

        assert!(result.is_err());
        assert!(cancellation_seen.load(Ordering::Acquire));
    }

    #[test]
    fn tray_runtime_shutdown_cancels_and_joins_the_worker() {
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = Arc::clone(&stopped);
        let mut runtime = TrayRuntime::start(
            move |worker| {
                worker.report_readiness(Ok(()));
                while !worker.is_cancelled() {
                    thread::yield_now();
                }
                worker_stopped.store(true, Ordering::Release);
            },
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .unwrap();

        assert_eq!(
            runtime.stop_within(Duration::from_secs(1)),
            TrayShutdownOutcome::Joined
        );
        assert!(stopped.load(Ordering::Acquire));
    }

    #[test]
    fn tray_runtime_shutdown_reports_worker_panic() {
        let mut runtime = TrayRuntime::start(
            |worker| {
                worker.report_readiness(Ok(()));
                panic!("intentional tray runtime test panic");
            },
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .unwrap();

        assert_eq!(
            runtime.stop_within(Duration::from_secs(1)),
            TrayShutdownOutcome::Panicked
        );
    }

    #[test]
    fn tray_runtime_shutdown_detaches_a_stuck_worker_at_the_deadline() {
        let (release_tx, release_rx) = mpsc::channel();
        let mut runtime = TrayRuntime::start(
            move |worker| {
                worker.report_readiness(Ok(()));
                let _ = release_rx.recv();
            },
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .unwrap();
        let started = Instant::now();

        assert_eq!(
            runtime.stop_within(Duration::from_millis(5)),
            TrayShutdownOutcome::TimedOut
        );
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "tray shutdown must remain bounded"
        );
        release_tx.send(()).unwrap();
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
