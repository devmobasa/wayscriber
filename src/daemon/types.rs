use std::ffi::OsString;
use std::time::Instant;

#[cfg(feature = "tray")]
use std::sync::Mutex;
#[cfg(feature = "tray")]
use std::sync::atomic::{AtomicU64, Ordering};

/// Overlay state for daemon mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Hidden,  // Daemon running, overlay not visible
    Visible, // Overlay active, capturing input
}

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
        self.inner.lock().unwrap().clone()
    }

    pub(crate) fn set_overlay_error(&self, error: Option<OverlaySpawnErrorInfo>) {
        let mut status = self.inner.lock().unwrap();
        status.overlay_error = error;
        self.bump_revision();
    }

    pub(crate) fn set_watcher_offline(&self, reason: String) -> bool {
        let mut status = self.inner.lock().unwrap();
        let was_offline = status.watcher_offline;
        status.watcher_offline = true;
        status.watcher_reason = Some(reason);
        self.bump_revision();
        !was_offline
    }

    pub(crate) fn set_watcher_online(&self) -> bool {
        let mut status = self.inner.lock().unwrap();
        let was_offline = status.watcher_offline;
        status.watcher_offline = false;
        status.watcher_reason = None;
        self.bump_revision();
        was_offline
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
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
