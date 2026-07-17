#[cfg(any(feature = "tray", feature = "portal"))]
use log::warn;
use std::ffi::OsString;
use std::sync::Arc;
#[cfg(feature = "tray")]
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
#[cfg(feature = "tray")]
use std::sync::atomic::AtomicU64;
#[cfg(any(feature = "tray", feature = "portal"))]
use std::sync::atomic::Ordering;
#[cfg(feature = "tray")]
use std::time::Instant;

use crate::backend::wayland::RuntimeWakeHandle;
#[cfg(all(test, any(feature = "tray", feature = "portal")))]
use crate::backend::wayland::RuntimeWakeSource;

/// A daemon-owned flag whose external producers cannot publish without also
/// waking the control loop that consumes it.
#[derive(Clone)]
pub(super) struct DaemonControlEvent {
    #[cfg(any(feature = "tray", feature = "portal"))]
    flag: Arc<AtomicBool>,
    #[cfg(any(feature = "tray", feature = "portal"))]
    wake: RuntimeWakeHandle,
}

impl DaemonControlEvent {
    pub(super) fn new(flag: Arc<AtomicBool>, wake: RuntimeWakeHandle) -> Self {
        #[cfg(any(feature = "tray", feature = "portal"))]
        {
            Self { flag, wake }
        }
        #[cfg(not(any(feature = "tray", feature = "portal")))]
        {
            let _ = (flag, wake);
            Self {}
        }
    }

    #[cfg(any(feature = "tray", feature = "portal"))]
    pub(super) fn raise(&self, source: &str) {
        self.flag.store(true, Ordering::Release);
        if let Err(err) = self.wake.wake() {
            warn!("Failed to wake daemon after {source}: {err}");
        }
    }

    #[cfg(feature = "tray")]
    pub(super) fn is_raised(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    #[cfg(all(test, any(feature = "tray", feature = "portal")))]
    pub(super) fn for_test(flag: Arc<AtomicBool>) -> (Self, RuntimeWakeSource) {
        let wake = RuntimeWakeSource::new().unwrap();
        (Self::new(flag, wake.handle()), wake)
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
