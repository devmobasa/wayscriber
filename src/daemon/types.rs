use crate::tray_action::TrayAction;
#[cfg(feature = "tray")]
use log::warn;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
#[cfg(feature = "tray")]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "tray")]
use std::time::Instant;

use crate::backend::wayland::RuntimeWakeHandle;
#[cfg(test)]
use crate::backend::wayland::RuntimeWakeSource;

trait ControlWake: Send + Sync {
    fn wake(&self) -> io::Result<()>;
}

impl ControlWake for RuntimeWakeHandle {
    fn wake(&self) -> io::Result<()> {
        RuntimeWakeHandle::wake(self)
    }
}

/// A daemon-owned flag whose external producers cannot publish without also
/// waking the control loop that consumes it.
#[derive(Clone)]
pub(super) struct DaemonControlEvent {
    flag: Arc<AtomicBool>,
    wake: Arc<dyn ControlWake>,
}

impl DaemonControlEvent {
    pub(super) fn new(flag: Arc<AtomicBool>, wake: RuntimeWakeHandle) -> Self {
        Self {
            flag,
            wake: Arc::new(wake),
        }
    }

    #[cfg(test)]
    fn with_wake(flag: Arc<AtomicBool>, wake: Arc<dyn ControlWake>) -> Self {
        Self { flag, wake }
    }

    pub(super) fn raise(&self, source: &str) -> io::Result<()> {
        self.flag.store(true, Ordering::Release);
        self.wake
            .wake()
            .map_err(|error| io::Error::new(error.kind(), format!("{source}: {error}")))
    }

    pub(super) fn is_raised(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(super) fn for_test(flag: Arc<AtomicBool>) -> (Self, RuntimeWakeSource) {
        let wake = RuntimeWakeSource::new().unwrap();
        (Self::new(flag, wake.handle()), wake)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct VisibilityIntent {
    pub(super) activation_token: Option<String>,
    pub(super) signal_requested: bool,
}

#[derive(Debug, Default)]
struct VisibilityIntentState {
    activation_token: Option<String>,
    signal_requested: bool,
}

#[derive(Debug, Default)]
pub(super) struct VisibilityIntents {
    state: Mutex<VisibilityIntentState>,
    ready: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(super) struct VisibilityPublisher {
    intents: Arc<VisibilityIntents>,
    event: DaemonControlEvent,
}

impl VisibilityIntents {
    #[cfg(test)]
    pub(super) fn with_ready(ready: Arc<AtomicBool>) -> Self {
        Self {
            state: Mutex::new(VisibilityIntentState::default()),
            ready,
        }
    }

    pub(super) fn publisher(self: &Arc<Self>, wake: RuntimeWakeHandle) -> VisibilityPublisher {
        VisibilityPublisher {
            intents: Arc::clone(self),
            event: DaemonControlEvent::new(Arc::clone(&self.ready), wake),
        }
    }

    pub(super) fn claim(&self) -> Option<VisibilityIntent> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !self.ready.swap(false, Ordering::Acquire) {
            return None;
        }
        Some(VisibilityIntent {
            activation_token: state.activation_token.take(),
            signal_requested: std::mem::take(&mut state.signal_requested),
        })
    }
}

impl VisibilityPublisher {
    pub(super) fn publish(
        &self,
        activation_token: Option<String>,
        signal_requested: bool,
        source: &str,
    ) -> io::Result<()> {
        let mut state = self
            .intents
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if activation_token.is_some() {
            state.activation_token = activation_token;
        }
        state.signal_requested |= signal_requested;
        self.event.raise(source)
    }
}

/// Overlay state for daemon mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Hidden,  // Daemon running, overlay not visible
    Visible, // Overlay active, capturing input
}

#[cfg(feature = "tray")]
#[derive(Debug, Clone)]
pub(crate) struct OverlaySpawnErrorInfo {
    pub(crate) message: String,
    pub(crate) next_retry_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub(crate) struct OverlaySpawnCandidate {
    pub(crate) program: OsString,
    pub(crate) source: &'static str,
}

#[cfg(feature = "tray")]
#[derive(Debug, Default, Clone)]
pub(crate) struct TrayStatus {
    pub(crate) overlay_error: Option<OverlaySpawnErrorInfo>,
    pub(crate) watcher_offline: bool,
    pub(crate) watcher_reason: Option<String>,
}

#[cfg(feature = "tray")]
#[derive(Debug)]
pub(crate) struct TrayStatusShared {
    inner: Mutex<TrayStatus>,
    revision: AtomicU64,
}

#[cfg(feature = "tray")]
impl TrayStatusShared {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(TrayStatus::default()),
            revision: AtomicU64::new(0),
        }
    }

    pub(crate) fn snapshot(&self) -> TrayStatus {
        self.lock_status().clone()
    }

    pub(crate) fn set_overlay_error(&self, error: Option<OverlaySpawnErrorInfo>) {
        {
            let mut status = self.lock_status();
            status.overlay_error = error;
        }
        self.bump_revision();
    }

    pub(crate) fn set_watcher_offline(&self, reason: String) -> bool {
        let was_offline = {
            let mut status = self.lock_status();
            let was_offline = status.watcher_offline;
            status.watcher_offline = true;
            status.watcher_reason = Some(reason);
            was_offline
        };
        self.bump_revision();
        !was_offline
    }

    pub(crate) fn set_watcher_online(&self) -> bool {
        let was_offline = {
            let mut status = self.lock_status();
            let was_offline = status.watcher_offline;
            status.watcher_offline = false;
            status.watcher_reason = None;
            was_offline
        };
        self.bump_revision();
        was_offline
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    fn lock_status(&self) -> std::sync::MutexGuard<'_, TrayStatus> {
        self.inner.lock().unwrap_or_else(|poisoned| {
            warn!("tray status mutex poisoned; recovering");
            poisoned.into_inner()
        })
    }

    fn bump_revision(&self) {
        self.revision.fetch_add(1, Ordering::Release);
    }
}

#[derive(Debug)]
pub struct AlreadyRunningError;

impl std::fmt::Display for AlreadyRunningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "wayscriber daemon is already running")
    }
}

impl std::error::Error for AlreadyRunningError {}

/// Daemon state manager
pub type BackendRunner = dyn Fn(Option<String>) -> anyhow::Result<()> + Send + Sync;

const MAX_OVERLAY_ACTION_INTENTS: usize = 64;

#[derive(Debug, Default)]
struct OverlayActionIntentState {
    queue: VecDeque<TrayAction>,
    in_flight: usize,
}

#[derive(Debug, Default)]
pub(crate) struct OverlayActionIntents {
    state: Mutex<OverlayActionIntentState>,
    ready: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(super) struct OverlayActionPublisher {
    intents: Arc<OverlayActionIntents>,
    event: DaemonControlEvent,
}

#[derive(Debug)]
pub(super) enum OverlayActionPublishError {
    QueueFull,
    Wake(io::Error),
}

impl OverlayActionIntents {
    pub(super) fn publisher(self: &Arc<Self>, wake: RuntimeWakeHandle) -> OverlayActionPublisher {
        OverlayActionPublisher {
            intents: Arc::clone(self),
            event: DaemonControlEvent::new(Arc::clone(&self.ready), wake),
        }
    }

    pub(crate) fn claim_batch(&self) -> Vec<TrayAction> {
        self.claim_batch_with_hook(|| {})
    }

    fn claim_batch_with_hook(&self, after_claim: impl FnOnce()) -> Vec<TrayAction> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !self.ready.swap(false, Ordering::Acquire) {
            return Vec::new();
        }
        after_claim();
        let batch: Vec<_> = state.queue.drain(..).collect();
        state.in_flight += batch.len();
        batch
    }

    pub(crate) fn finish_batch(&self, completed: usize) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(
            completed <= state.in_flight,
            "completed action count exceeds in-flight ownership"
        );
        state.in_flight -= completed;
    }

    #[cfg(test)]
    fn pending_counts(&self) -> (usize, usize) {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (state.queue.len(), state.in_flight)
    }
}

impl OverlayActionPublisher {
    pub(super) fn publish(&self, action: TrayAction) -> Result<(), OverlayActionPublishError> {
        let mut state = self
            .intents
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.queue.len() + state.in_flight >= MAX_OVERLAY_ACTION_INTENTS {
            return Err(OverlayActionPublishError::QueueFull);
        }
        state.queue.push_back(action);
        self.event
            .raise("tray action")
            .map_err(OverlayActionPublishError::Wake)
    }
}

#[cfg(test)]
mod control_tests {
    use super::*;
    use std::sync::Barrier;
    use std::sync::atomic::AtomicUsize;

    #[derive(Default)]
    struct CountingWake {
        calls: AtomicUsize,
        fail: AtomicBool,
    }

    impl ControlWake for CountingWake {
        fn wake(&self) -> io::Result<()> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if self.fail.load(Ordering::Relaxed) {
                Err(io::Error::other("injected wake failure"))
            } else {
                Ok(())
            }
        }
    }

    fn action_publisher(
        intents: &Arc<OverlayActionIntents>,
        wake: Arc<CountingWake>,
    ) -> OverlayActionPublisher {
        OverlayActionPublisher {
            intents: Arc::clone(intents),
            event: DaemonControlEvent::with_wake(Arc::clone(&intents.ready), wake),
        }
    }

    #[test]
    fn action_published_after_batch_claim_remains_pending() {
        let intents = Arc::new(OverlayActionIntents::default());
        let wake = Arc::new(CountingWake::default());
        let publisher = action_publisher(&intents, wake);
        publisher.publish(TrayAction::ToggleFreeze).unwrap();

        let claim_barrier = Arc::new(Barrier::new(2));
        let release_barrier = Arc::new(Barrier::new(2));
        let claimant_intents = Arc::clone(&intents);
        let claimant_claim = Arc::clone(&claim_barrier);
        let claimant_release = Arc::clone(&release_barrier);
        let claimant = std::thread::spawn(move || {
            claimant_intents.claim_batch_with_hook(|| {
                claimant_claim.wait();
                claimant_release.wait();
            })
        });

        claim_barrier.wait();
        let late_publisher = publisher.clone();
        let late = std::thread::spawn(move || {
            late_publisher.publish(TrayAction::ToggleHelp).unwrap();
        });
        release_barrier.wait();

        let first = claimant.join().unwrap();
        late.join().unwrap();
        assert_eq!(first, [TrayAction::ToggleFreeze]);
        intents.finish_batch(first.len());

        let second = intents.claim_batch();
        assert_eq!(second, [TrayAction::ToggleHelp]);
        intents.finish_batch(second.len());
    }

    #[test]
    fn queue_full_rejection_does_not_publish_another_wake() {
        let intents = Arc::new(OverlayActionIntents::default());
        let wake = Arc::new(CountingWake::default());
        let publisher = action_publisher(&intents, Arc::clone(&wake));
        for _ in 0..MAX_OVERLAY_ACTION_INTENTS {
            publisher.publish(TrayAction::ToggleHelp).unwrap();
        }
        let wake_count = wake.calls.load(Ordering::Relaxed);

        assert!(matches!(
            publisher.publish(TrayAction::ToggleFreeze),
            Err(OverlayActionPublishError::QueueFull)
        ));
        assert_eq!(wake.calls.load(Ordering::Relaxed), wake_count);
        assert!(intents.ready.load(Ordering::Acquire));
    }

    #[test]
    fn wake_failure_keeps_action_and_readiness_pending() {
        let intents = Arc::new(OverlayActionIntents::default());
        let wake = Arc::new(CountingWake::default());
        wake.fail.store(true, Ordering::Relaxed);
        let publisher = action_publisher(&intents, wake);

        assert!(matches!(
            publisher.publish(TrayAction::ToggleFreeze),
            Err(OverlayActionPublishError::Wake(_))
        ));
        assert!(intents.ready.load(Ordering::Acquire));
        assert_eq!(intents.pending_counts(), (1, 0));

        let batch = intents.claim_batch();
        assert_eq!(batch, [TrayAction::ToggleFreeze]);
        intents.finish_batch(batch.len());
    }

    #[test]
    fn visibility_metadata_merges_and_claims_as_one_snapshot() {
        let intents = Arc::new(VisibilityIntents::default());
        let wake = Arc::new(CountingWake::default());
        let publisher = VisibilityPublisher {
            intents: Arc::clone(&intents),
            event: DaemonControlEvent::with_wake(Arc::clone(&intents.ready), wake),
        };

        publisher
            .publish(Some("first".into()), false, "first publication")
            .unwrap();
        publisher.publish(None, true, "signal publication").unwrap();
        publisher
            .publish(Some("latest".into()), false, "replacement publication")
            .unwrap();

        assert_eq!(
            intents.claim(),
            Some(VisibilityIntent {
                activation_token: Some("latest".into()),
                signal_requested: true,
            })
        );
        assert_eq!(intents.claim(), None);
    }
}
