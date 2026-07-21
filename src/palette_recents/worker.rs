use super::{PALETTE_RECENTS_CAP, PaletteRecentsStore};
use crate::domain::Action;
use log::warn;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(250);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

/// Non-blocking event-loop facade for persisted palette recents.
///
/// Requests replace one shared pending snapshot and wake a dedicated writer
/// thread. The worker performs the atomic file and parent-directory syncs, and
/// retries failed writes with bounded exponential backoff. Keeping only the
/// latest pending snapshot bounds memory if storage is slow or unavailable.
pub(crate) struct PaletteRecentsWriter {
    pending: Arc<Mutex<Option<Vec<Action>>>>,
    wake: Option<SyncSender<()>>,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    persistence_disabled: bool,
}

impl PaletteRecentsWriter {
    pub(crate) fn new(store: PaletteRecentsStore) -> Self {
        if store.path.is_none() {
            return Self {
                pending: Arc::new(Mutex::new(None)),
                wake: None,
                shutdown: Arc::new(AtomicBool::new(false)),
                worker: None,
                persistence_disabled: true,
            };
        }

        let pending = Arc::new(Mutex::new(None));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (wake, receiver) = sync_channel(1);
        let worker_pending = Arc::clone(&pending);
        let worker_shutdown = Arc::clone(&shutdown);
        let worker = thread::Builder::new()
            .name("palette-recents".to_string())
            .spawn(move || run_writer(store, worker_pending, worker_shutdown, receiver));

        match worker {
            Ok(worker) => Self {
                pending,
                wake: Some(wake),
                shutdown,
                worker: Some(worker),
                persistence_disabled: false,
            },
            Err(err) => {
                warn!("Failed to start palette recents writer: {err}");
                Self {
                    pending,
                    wake: Some(wake),
                    shutdown,
                    worker: None,
                    persistence_disabled: false,
                }
            }
        }
    }

    /// Queue the latest desired history without performing filesystem work on
    /// the caller. Returns false only when the writer could not be started or
    /// has terminated unexpectedly, allowing the caller to retain its dirty
    /// flag and try again without falling back to synchronous I/O.
    #[must_use = "a false return means the request was not accepted"]
    pub(crate) fn request(&self, recents: &[Action]) -> bool {
        if self.persistence_disabled {
            return true;
        }
        let Some(wake) = self.wake.as_ref() else {
            return false;
        };

        let latest = recents.iter().copied().take(PALETTE_RECENTS_CAP).collect();
        *self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(latest);

        match wake.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => true,
            Err(TrySendError::Disconnected(())) => false,
        }
    }
}

impl Drop for PaletteRecentsWriter {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(wake) = self.wake.take() {
            let _ = wake.try_send(());
            drop(wake);
        }
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            warn!("Palette recents writer thread panicked");
        }
    }
}

fn run_writer(
    mut store: PaletteRecentsStore,
    pending: Arc<Mutex<Option<Vec<Action>>>>,
    shutdown: Arc<AtomicBool>,
    receiver: Receiver<()>,
) {
    let mut retry_delay = INITIAL_RETRY_DELAY;

    while receiver.recv().is_ok() {
        loop {
            let desired = pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
            let Some(desired) = desired else {
                break;
            };

            if store.set_recents(&desired) {
                retry_delay = INITIAL_RETRY_DELAY;
                continue;
            }

            // Preserve a newer request that arrived during the failed write;
            // otherwise restore the failed snapshot for the timed retry.
            let mut queued = pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if queued.is_none() {
                *queued = Some(desired);
            }
            drop(queued);

            if shutdown.load(Ordering::Acquire) {
                return;
            }
            match receiver.recv_timeout(retry_delay) {
                Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            retry_delay = retry_delay.saturating_mul(2).min(MAX_RETRY_DELAY);
        }

        if shutdown.load(Ordering::Acquire) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn writer_flushes_the_latest_queued_snapshot_before_drop_returns() {
        let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
        let path = tmp.path().join("wayscriber").join("palette_recents.toml");
        let store = PaletteRecentsStore::load_from_path(path.clone());
        let writer = PaletteRecentsWriter::new(store);

        assert!(writer.request(&[Action::ToggleHelp]));
        assert!(writer.request(&[Action::Undo, Action::ToggleHelp]));
        drop(writer);

        let reloaded = PaletteRecentsStore::load_from_path(path);
        assert_eq!(reloaded.recents(), &[Action::Undo, Action::ToggleHelp]);
    }

    #[test]
    fn writer_without_a_target_accepts_requests_without_starting_a_thread() {
        let writer = PaletteRecentsWriter::new(PaletteRecentsStore {
            recents: Vec::new(),
            persisted: true,
            path: None::<PathBuf>,
        });

        assert!(writer.request(&[Action::ToggleHelp]));
    }
}
