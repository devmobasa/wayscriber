use anyhow::Result;
#[cfg(feature = "tray")]
use anyhow::anyhow;
#[cfg(feature = "tray")]
use ksni::TrayMethods;
use log::info;
#[cfg(feature = "tray")]
use log::{debug, warn};
#[cfg(feature = "tray")]
use std::path::Path;
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
use crate::config::{Config, ConfigDocument, RuntimeConfigBackup, SessionConfig, TrayIconStyle};
#[cfg(feature = "tray")]
use crate::env_vars::CONFIGURATOR_ENV;

#[cfg(feature = "tray")]
const TRAY_START_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(feature = "tray")]
const STATUS_NOTIFIER_WATCHER_BUS: &str = "org.kde.StatusNotifierWatcher";
#[cfg(feature = "tray")]
const STATUS_NOTIFIER_WATCHER_PATH: &str = "/StatusNotifierWatcher";
/// Attempts allowed for one session-resume toggle. The overlay's writer
/// debounces at 75 ms, so a handful of spaced retries outlives a burst of
/// runtime preference writes without making a menu click feel stuck.
#[cfg(feature = "tray")]
const SESSION_RESUME_WRITE_ATTEMPTS: usize = 4;
#[cfg(feature = "tray")]
const SESSION_RESUME_RETRY_BACKOFF: Duration = Duration::from_millis(150);

#[cfg(feature = "tray")]
fn session_resume_enabled(session: &SessionConfig) -> bool {
    session.persist_transparent
        || session.persist_whiteboard
        || session.persist_blackboard
        || session.persist_history
        || session.restore_tool_state
}

#[cfg(feature = "tray")]
fn load_tray_settings_from_config() -> (bool, TrayIconStyle) {
    match Config::load() {
        Ok(loaded) => {
            let config = loaded.config;
            (
                session_resume_enabled(&config.session),
                config.tray.icon_style,
            )
        }
        Err(err) => {
            warn!(
                "Failed to read config for tray settings; using safe defaults: {}",
                err
            );
            (false, TrayIconStyle::Auto)
        }
    }
}

/// Bring the `session.*` flags the tray's single toggle owns to `target`.
///
/// The menu reads them as one OR'd state, so enabling has to raise every flag
/// and disabling has to clear every one. Only the flags that disagree with the
/// target are assigned: the ones already matching are not part of this toggle
/// and stay exactly as the file authored them. Returns whether anything moved.
#[cfg(feature = "tray")]
fn apply_session_resume(session: &mut SessionConfig, target: bool) -> bool {
    let flags: [&mut bool; 5] = [
        &mut session.persist_transparent,
        &mut session.persist_whiteboard,
        &mut session.persist_blackboard,
        &mut session.persist_history,
        &mut session.restore_tool_state,
    ];
    let mut changed = false;
    for flag in flags {
        if *flag != target {
            *flag = target;
            changed = true;
        }
    }
    changed
}

/// Persist the toggle once against a freshly loaded document.
#[cfg(feature = "tray")]
fn write_session_resume(
    path: &Path,
    target_enabled: bool,
    backup: &mut RuntimeConfigBackup,
) -> Result<()> {
    let document = ConfigDocument::load_from_path(path)?;
    let mut config = document.config().clone();
    if !apply_session_resume(&mut config.session, target_enabled) {
        return Ok(());
    }
    // Taken here so a redundant toggle does not spend the daemon's one
    // snapshot on a file it leaves untouched, and the retry loop below takes
    // it once no matter how many attempts the write needs.
    backup.ensure_snapshot(path);
    document.save(config)?;
    Ok(())
}

/// Retry a config write a bounded number of times, pausing between attempts.
///
/// The tray writes from the daemon process, so it cannot share the overlay's
/// background writer and its retry queue. An overlay write that lands between
/// this load and its save trips the document's revision guard; reloading and
/// trying again is what keeps a user-initiated toggle from silently doing
/// nothing. The caller passes a fresh-load closure, so every attempt starts
/// from the document as it is now.
#[cfg(feature = "tray")]
fn persist_with_retry(
    attempts: usize,
    backoff: Duration,
    mut write: impl FnMut() -> Result<()>,
) -> Result<()> {
    let mut last_error = None;
    for attempt in 1..=attempts {
        match write() {
            Ok(()) => return Ok(()),
            Err(error) => {
                if attempt < attempts {
                    debug!(
                        "Config write attempt {}/{} failed; retrying: {:#}",
                        attempt, attempts, error
                    );
                    thread::sleep(backoff);
                }
                last_error = Some(error);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("Config write was never attempted")))
}

/// The daemon is its own process, so it carries its own backup guard: the
/// overlay's writer cannot take a snapshot on its behalf, and the tray struct
/// outlives every menu click, which makes it the daemon-lifetime owner.
#[cfg(feature = "tray")]
pub(super) fn update_session_resume_in_config(
    backup: &mut RuntimeConfigBackup,
    target_enabled: bool,
    fallback: bool,
) -> bool {
    let path = match Config::get_config_path() {
        Ok(path) => path,
        Err(err) => {
            warn!(
                "Failed to locate config while toggling session resume (desired {}): {:#}",
                target_enabled, err
            );
            return fallback;
        }
    };

    match persist_with_retry(
        SESSION_RESUME_WRITE_ATTEMPTS,
        SESSION_RESUME_RETRY_BACKOFF,
        || write_session_resume(&path, target_enabled, backup),
    ) {
        Ok(()) => target_enabled,
        Err(err) => {
            warn!(
                "Failed to write session resume setting to config after {} attempts (desired {}): {:#}",
                SESSION_RESUME_WRITE_ATTEMPTS, target_enabled, err
            );
            fallback
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
    let (session_resume_enabled, icon_style) = load_tray_settings_from_config();

    let tray_quit = quit.clone();
    let tray = WayscriberTray::new(
        TrayControl {
            visibility,
            action,
            quit: quit.clone(),
        },
        configurator_binary,
        session_resume_enabled,
        icon_style,
        overlay_active,
        tray_status.clone(),
        RuntimeConfigBackup::new(),
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
    use std::cell::Cell;
    use std::fs;

    fn write_config(path: &Path, contents: &str) {
        fs::write(path, contents).expect("test config should be written");
    }

    /// Keeps the safety-net copy inside the test's temp directory instead of
    /// the developer's real XDG state directory.
    fn test_backup(path: &Path) -> RuntimeConfigBackup {
        RuntimeConfigBackup::with_directory(backup_dir(path))
    }

    fn backup_dir(path: &Path) -> std::path::PathBuf {
        path.parent()
            .expect("a test config path has a parent")
            .join("config-backups")
    }

    fn backup_contents(path: &Path) -> Vec<String> {
        let directory = backup_dir(path);
        if !directory.exists() {
            return Vec::new();
        }
        let mut entries = fs::read_dir(&directory)
            .expect("backup directory should be listable")
            .filter_map(Result::ok)
            .map(|entry| fs::read_to_string(entry.path()).expect("snapshot should be readable"))
            .collect::<Vec<_>>();
        entries.sort();
        entries
    }

    #[test]
    fn enabling_raises_every_flag_the_menu_reads() {
        let mut session = Config::default().session;
        session.persist_transparent = false;
        session.persist_whiteboard = false;
        session.persist_blackboard = false;
        session.persist_history = false;
        session.restore_tool_state = false;

        assert!(apply_session_resume(&mut session, true));
        assert!(session_resume_enabled(&session));
        assert!(session.persist_transparent);
        assert!(session.persist_whiteboard);
        assert!(session.persist_blackboard);
        assert!(session.persist_history);
        assert!(session.restore_tool_state);
    }

    #[test]
    fn a_toggle_that_changes_nothing_reports_no_edit() {
        let mut session = Config::default().session;
        apply_session_resume(&mut session, true);

        assert!(!apply_session_resume(&mut session, true));
    }

    /// The toggle owns the flags it flips and nothing else: a file that already
    /// disables four of the five only has the fifth rewritten.
    #[test]
    fn disabling_writes_only_the_flags_that_were_on() {
        let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
        let path = temp.path().join("config.toml");
        let original = "# session notes\n[session]\npersist_transparent = false\npersist_whiteboard = false\npersist_blackboard = false\npersist_history = true\nrestore_tool_state = false\nautosave_idle_ms = 900\n";
        write_config(&path, original);

        write_session_resume(&path, false, &mut test_backup(&path))
            .expect("the toggle should persist");

        let written = fs::read_to_string(&path).expect("config should be readable");
        assert!(written.contains("# session notes"));
        assert!(written.contains("persist_history = false"));
        assert!(written.contains("autosave_idle_ms = 900"));
        let reloaded = ConfigDocument::load_from_path(&path).expect("saved config should parse");
        assert!(!session_resume_enabled(&reloaded.config().session));
        // The daemon writes without the overlay's writer, so it has to take
        // its own pre-write copy.
        assert_eq!(backup_contents(&path), vec![original.to_string()]);
    }

    /// One copy per daemon process, taken from the file the user authored:
    /// a second toggle must not overwrite it with the first toggle's output.
    #[test]
    fn a_second_toggle_reuses_the_daemons_single_snapshot() {
        let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
        let path = temp.path().join("config.toml");
        let original = "[session]\npersist_transparent = true\npersist_whiteboard = true\npersist_blackboard = true\npersist_history = true\nrestore_tool_state = true\n";
        write_config(&path, original);
        let mut backup = test_backup(&path);

        write_session_resume(&path, false, &mut backup).expect("the first toggle should persist");
        write_session_resume(&path, true, &mut backup).expect("the second toggle should persist");

        assert_eq!(backup_contents(&path), vec![original.to_string()]);
    }

    /// A snapshot that cannot be taken is a logged warning, not a lost click.
    #[test]
    fn an_unusable_backup_directory_does_not_block_the_toggle() {
        let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
        let path = temp.path().join("config.toml");
        write_config(
            &path,
            "[session]\npersist_transparent = true\npersist_whiteboard = true\npersist_blackboard = true\npersist_history = true\nrestore_tool_state = true\n",
        );
        // A regular file where the backup directory belongs.
        write_config(&backup_dir(&path), "not a directory\n");
        let mut backup = RuntimeConfigBackup::with_directory(backup_dir(&path));

        write_session_resume(&path, false, &mut backup)
            .expect("a failed snapshot must not fail the toggle");

        let reloaded = ConfigDocument::load_from_path(&path).expect("saved config should parse");
        assert!(!session_resume_enabled(&reloaded.config().session));
    }

    /// Nothing to change means nothing to write, so a redundant toggle cannot
    /// lose a race with a concurrent overlay write it did not need to win.
    #[test]
    fn a_redundant_toggle_leaves_the_file_untouched() {
        let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
        let path = temp.path().join("config.toml");
        let original = "[session]\npersist_transparent = false\npersist_whiteboard = false\npersist_blackboard = false\npersist_history = false\nrestore_tool_state = false\n";
        write_config(&path, original);

        write_session_resume(&path, false, &mut test_backup(&path))
            .expect("a redundant toggle should succeed");

        assert_eq!(
            fs::read_to_string(&path).expect("config should be readable"),
            original
        );
        // Nothing was written, so the daemon's one snapshot is still unspent
        // for the toggle that does change something.
        assert!(backup_contents(&path).is_empty());
    }

    #[test]
    fn a_write_that_succeeds_first_is_not_retried() {
        let attempts = Cell::new(0);
        persist_with_retry(4, Duration::ZERO, || {
            attempts.set(attempts.get() + 1);
            Ok(())
        })
        .expect("the first attempt should settle the write");

        assert_eq!(attempts.get(), 1);
    }

    /// A concurrent writer trips the document's revision guard. The toggle has
    /// to reload and try again instead of reporting the click as lost.
    #[test]
    fn a_conflicting_write_is_retried_until_it_lands() {
        let attempts = Cell::new(0);
        persist_with_retry(4, Duration::ZERO, || {
            attempts.set(attempts.get() + 1);
            if attempts.get() < 3 {
                return Err(anyhow!(
                    "Configuration changed on disk. Reload before saving."
                ));
            }
            Ok(())
        })
        .expect("the third attempt should land");

        assert_eq!(attempts.get(), 3);
    }

    #[test]
    fn a_persistently_conflicting_write_gives_up_with_the_last_error() {
        let attempts = Cell::new(0);
        let error = persist_with_retry(3, Duration::ZERO, || {
            attempts.set(attempts.get() + 1);
            Err(anyhow!("attempt {} lost the race", attempts.get()))
        })
        .expect_err("an always-conflicting write should fail");

        assert_eq!(attempts.get(), 3);
        assert_eq!(error.to_string(), "attempt 3 lost the race");
    }
}
