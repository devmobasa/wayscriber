use super::{PALETTE_RECENTS_CAP, PaletteRecentsStore};
use crate::domain::Action;
use log::warn;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(250);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

/// Non-blocking event-loop facade for persisted palette recents.
///
/// Requests cross a capacity-one channel to a dedicated writer thread. The
/// worker performs the atomic file and parent-directory syncs, coalesces to the
/// newest accepted snapshot before each write, and retries failed writes with
/// bounded exponential backoff. When the channel is full, the caller retains
/// its dirty flag and retries the newest root-owned snapshot on a later loop.
pub(crate) struct PaletteRecentsWriter {
    requests: Option<SyncSender<Vec<Action>>>,
    pending: Option<Vec<Action>>,
    worker: Option<JoinHandle<()>>,
    persistence_disabled: bool,
}

impl PaletteRecentsWriter {
    pub(crate) fn new(store: PaletteRecentsStore) -> Self {
        if store.path.is_none() {
            return Self {
                requests: None,
                pending: None,
                worker: None,
                persistence_disabled: true,
            };
        }

        let (requests, receiver) = sync_channel(1);
        let worker = thread::Builder::new()
            .name("palette-recents".to_string())
            .spawn(move || run_writer(store, receiver));

        match worker {
            Ok(worker) => Self {
                requests: Some(requests),
                pending: None,
                worker: Some(worker),
                persistence_disabled: false,
            },
            Err(err) => {
                warn!("Failed to start palette recents writer: {err}");
                Self {
                    requests: Some(requests),
                    pending: None,
                    worker: None,
                    persistence_disabled: false,
                }
            }
        }
    }

    /// Queue the latest desired history without performing filesystem work on
    /// the caller. Returns false while the capacity-one channel is occupied,
    /// when the writer could not be started, or after it terminates. The caller
    /// retains its dirty flag and retries its newest snapshot without falling
    /// back to synchronous I/O.
    #[must_use = "a false return means the request was not accepted"]
    pub(crate) fn request(&mut self, recents: &[Action]) -> bool {
        if self.persistence_disabled {
            return true;
        }
        let latest = recents.iter().copied().take(PALETTE_RECENTS_CAP).collect();
        self.pending = Some(latest);
        let Some(requests) = self.requests.as_ref() else {
            return false;
        };
        let Some(pending) = self.pending.take() else {
            return true;
        };
        match requests.try_send(pending) {
            Ok(()) => true,
            Err(TrySendError::Full(pending)) | Err(TrySendError::Disconnected(pending)) => {
                self.pending = Some(pending);
                false
            }
        }
    }
}

impl Drop for PaletteRecentsWriter {
    fn drop(&mut self) {
        if let Some(pending) = self.pending.take()
            && let Some(requests) = self.requests.as_ref()
            && requests.send(pending).is_err()
        {
            warn!("Palette recents writer stopped before accepting its final snapshot");
        }
        self.requests.take();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            warn!("Palette recents writer thread panicked");
        }
    }
}

fn run_writer(mut store: PaletteRecentsStore, receiver: Receiver<Vec<Action>>) {
    run_writer_loop(
        receiver,
        |desired| store.set_recents(desired),
        INITIAL_RETRY_DELAY,
        MAX_RETRY_DELAY,
    );
}

fn run_writer_loop(
    receiver: Receiver<Vec<Action>>,
    mut persist: impl FnMut(&[Action]) -> bool,
    initial_retry_delay: Duration,
    max_retry_delay: Duration,
) {
    while let Ok(mut desired) = receiver.recv() {
        while let Ok(newer) = receiver.try_recv() {
            desired = newer;
        }

        let mut retry_delay = initial_retry_delay;
        loop {
            if persist(&desired) {
                break;
            }
            match receiver.recv_timeout(retry_delay) {
                Ok(newer) => {
                    desired = newer;
                    while let Ok(newest) = receiver.try_recv() {
                        desired = newest;
                    }
                    retry_delay = initial_retry_delay;
                }
                Err(RecvTimeoutError::Timeout) => {
                    retry_delay = retry_delay.saturating_mul(2).min(max_retry_delay);
                }
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn writer_flushes_the_latest_queued_snapshot_before_drop_returns() {
        let tmp = crate::test_temp::tempdir()
            .expect("fixture creates its private palette-recents directory");
        let path = tmp.path().join("wayscriber").join("palette_recents.toml");
        let store = PaletteRecentsStore::load_from_path(path.clone());
        let mut writer = PaletteRecentsWriter::new(store);

        assert!(writer.request(&[Action::ToggleHelp]));
        let mut latest_accepted = false;
        for _ in 0..100 {
            if writer.request(&[Action::Undo, Action::ToggleHelp]) {
                latest_accepted = true;
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            latest_accepted,
            "fixture worker must accept the latest snapshot within its bounded retry window"
        );
        drop(writer);

        let reloaded = PaletteRecentsStore::load_from_path(path);
        assert_eq!(reloaded.recents(), &[Action::Undo, Action::ToggleHelp]);
    }

    #[test]
    fn writer_without_a_target_accepts_requests_without_starting_a_thread() {
        let mut writer = PaletteRecentsWriter::new(PaletteRecentsStore {
            recents: Vec::new(),
            persisted: true,
            path: None::<PathBuf>,
        });

        assert!(writer.request(&[Action::ToggleHelp]));
    }

    #[test]
    fn full_channel_retains_latest_snapshot_until_shutdown_can_enqueue_it() {
        let (requests, receiver) = sync_channel(1);
        requests
            .send(vec![Action::ToggleHelp])
            .expect("fixture request channel accepts its older queued snapshot");
        let mut writer = PaletteRecentsWriter {
            requests: Some(requests),
            pending: None,
            worker: None,
            persistence_disabled: false,
        };

        assert!(!writer.request(&[Action::Undo, Action::ToggleHelp]));
        let observer = thread::spawn(move || {
            let older = receiver
                .recv()
                .expect("fixture observer receives the older queued snapshot");
            let latest = receiver
                .recv()
                .expect("fixture observer receives the retained shutdown snapshot");
            (older, latest)
        });

        drop(writer);
        let (older, latest) = observer
            .join()
            .expect("fixture observer exits after receiving both owned snapshots");
        assert_eq!(older, vec![Action::ToggleHelp]);
        assert_eq!(latest, vec![Action::Undo, Action::ToggleHelp]);
    }

    #[test]
    fn failed_write_is_replaced_by_newer_snapshot_during_backoff() {
        let (requests, receiver) = sync_channel(1);
        let (attempt_tx, attempt_rx) = std::sync::mpsc::channel();
        let (outcome_tx, outcome_rx) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            run_writer_loop(
                receiver,
                move |desired| {
                    attempt_tx
                        .send(desired.to_vec())
                        .expect("fixture observer remains available for every write attempt");
                    outcome_rx
                        .recv()
                        .expect("fixture supplies one outcome for every observed write attempt")
                },
                Duration::from_secs(60),
                Duration::from_secs(60),
            );
        });

        requests
            .send(vec![Action::ToggleHelp])
            .expect("fixture queues its initial palette snapshot");
        assert_eq!(
            attempt_rx
                .recv()
                .expect("fixture observes the initial write attempt"),
            vec![Action::ToggleHelp]
        );
        outcome_tx
            .send(false)
            .expect("fixture injects the initial write failure");

        requests
            .send(vec![Action::Undo, Action::ToggleHelp])
            .expect("fixture replaces the failed snapshot during backoff");
        assert_eq!(
            attempt_rx
                .recv()
                .expect("fixture observes the replacement write attempt"),
            vec![Action::Undo, Action::ToggleHelp]
        );
        outcome_tx
            .send(true)
            .expect("fixture accepts the replacement write");

        drop(requests);
        worker
            .join()
            .expect("fixture writer exits after its request channel disconnects");
    }

    #[test]
    fn disconnect_during_failed_write_backoff_stops_without_retrying() {
        let (requests, receiver) = sync_channel(1);
        let (attempt_tx, attempt_rx) = std::sync::mpsc::channel();
        let (outcome_tx, outcome_rx) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            run_writer_loop(
                receiver,
                move |desired| {
                    attempt_tx
                        .send(desired.to_vec())
                        .expect("fixture observer remains available for the write attempt");
                    outcome_rx
                        .recv()
                        .expect("fixture supplies the observed write outcome")
                },
                Duration::from_secs(60),
                Duration::from_secs(60),
            );
        });

        requests
            .send(vec![Action::ToggleHelp])
            .expect("fixture queues its failing palette snapshot");
        assert_eq!(
            attempt_rx
                .recv()
                .expect("fixture observes the failing write attempt"),
            vec![Action::ToggleHelp]
        );
        drop(requests);
        outcome_tx
            .send(false)
            .expect("fixture injects failure after disconnecting the request channel");
        worker
            .join()
            .expect("fixture writer exits when backoff observes disconnection");
        assert!(matches!(
            attempt_rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected)
        ));
    }
}
