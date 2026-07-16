use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};

use tokio::sync::mpsc;

use crate::capture::{
    dependencies::CaptureDependencies,
    desktop_backdrop::capture_desktop_backdrop,
    file::FileSaveConfig,
    pipeline::{
        CaptureManagerRequest, CaptureManagerResult, CaptureRequest, deliver_document,
        deliver_image, perform_capture,
    },
    types::{
        CaptureDestination, CaptureError, CaptureOutcome, CaptureStatus, CaptureType,
        DesktopBackdropCaptureRequest, DocumentDeliveryRequest, ImageDeliveryRequest,
        ImageOperationKind,
    },
};

type CompletionNotifier = Arc<dyn Fn() + Send + Sync + 'static>;

/// Monotonic identity for one accepted capture or delivery operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CaptureRequestId(u64);

#[cfg(test)]
impl CaptureRequestId {
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for CaptureRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Synchronous rejection from the bounded capture manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureSubmitError {
    Busy { active_id: CaptureRequestId },
    IdentityExhausted,
    Disconnected,
    Unhealthy { reason: String },
}

impl fmt::Display for CaptureSubmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy { active_id } => {
                write!(formatter, "capture operation {active_id} is still active")
            }
            Self::IdentityExhausted => formatter.write_str("capture operation IDs exhausted"),
            Self::Disconnected => formatter.write_str("capture manager is not running"),
            Self::Unhealthy { reason } => {
                write!(formatter, "capture manager is unhealthy: {reason}")
            }
        }
    }
}

impl std::error::Error for CaptureSubmitError {}

/// Non-blocking observation of the unique capture completion owner.
#[derive(Debug)]
pub enum CapturePoll {
    Idle,
    Pending {
        id: CaptureRequestId,
        operation: ImageOperationKind,
    },
    Ready {
        id: CaptureRequestId,
        operation: ImageOperationKind,
        outcome: CaptureOutcome,
    },
    WorkerFailed {
        active_id: Option<CaptureRequestId>,
        operation: Option<ImageOperationKind>,
        error: String,
    },
}

struct CaptureCommand {
    id: CaptureRequestId,
    request: CaptureManagerRequest,
}

struct CaptureCompletion {
    id: CaptureRequestId,
    outcome: CaptureOutcome,
}

#[derive(Debug, Clone, Copy)]
struct ActiveCapture {
    id: CaptureRequestId,
    operation: ImageOperationKind,
}

/// Unique owner for bounded, identified asynchronous capture operations.
///
/// Production Wayland code installs a completion notifier backed by the shared
/// runtime wake. Other callers may use [`CaptureManager::new`] and poll the
/// manager directly.
pub struct CaptureManager {
    request_tx: Option<mpsc::Sender<CaptureCommand>>,
    completion_rx: Receiver<CaptureCompletion>,
    active: Option<ActiveCapture>,
    next_id: Option<u64>,
    healthy: bool,
    terminal_reported: bool,
    shutdown_requested: Arc<AtomicBool>,
    worker: Option<tokio::task::JoinHandle<()>>,
    status: Arc<tokio::sync::Mutex<CaptureStatus>>,
}

impl CaptureManager {
    /// Creates a manager whose owner polls completions directly.
    pub fn new(runtime_handle: &tokio::runtime::Handle) -> Self {
        Self::with_dependencies_and_notifier(
            runtime_handle,
            CaptureDependencies::default(),
            Arc::new(|| {}),
        )
    }

    /// Creates a manager with custom dependencies for deterministic consumers.
    #[cfg(test)]
    pub(crate) fn with_dependencies(
        runtime_handle: &tokio::runtime::Handle,
        dependencies: CaptureDependencies,
    ) -> Self {
        Self::with_dependencies_and_notifier(runtime_handle, dependencies, Arc::new(|| {}))
    }

    pub(crate) fn with_completion_notifier(
        runtime_handle: &tokio::runtime::Handle,
        notifier: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self::with_dependencies_and_notifier(
            runtime_handle,
            CaptureDependencies::default(),
            Arc::new(notifier),
        )
    }

    #[cfg(test)]
    pub(crate) fn with_dependencies_and_test_notifier(
        runtime_handle: &tokio::runtime::Handle,
        dependencies: CaptureDependencies,
        notifier: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self::with_dependencies_and_notifier(runtime_handle, dependencies, Arc::new(notifier))
    }

    fn with_dependencies_and_notifier(
        runtime_handle: &tokio::runtime::Handle,
        dependencies: CaptureDependencies,
        notifier: CompletionNotifier,
    ) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<CaptureCommand>(1);
        let (completion_tx, completion_rx) = std::sync::mpsc::sync_channel(1);
        let status = Arc::new(tokio::sync::Mutex::new(CaptureStatus::Idle));
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let worker = runtime_handle.spawn(run_capture_worker(
            request_rx,
            completion_tx,
            Arc::clone(&status),
            Arc::new(dependencies),
            notifier,
            Arc::clone(&shutdown_requested),
        ));

        Self {
            request_tx: Some(request_tx),
            completion_rx,
            active: None,
            next_id: Some(1),
            healthy: true,
            terminal_reported: false,
            shutdown_requested,
            worker: Some(worker),
            status,
        }
    }

    pub fn request_capture(
        &mut self,
        capture_type: CaptureType,
        destination: CaptureDestination,
        save_config: Option<FileSaveConfig>,
    ) -> Result<CaptureRequestId, CaptureSubmitError> {
        self.try_submit(CaptureManagerRequest::Capture(CaptureRequest {
            capture_type,
            destination,
            save_config,
        }))
    }

    pub fn request_desktop_backdrop_capture(
        &mut self,
        request: DesktopBackdropCaptureRequest,
    ) -> Result<CaptureRequestId, CaptureSubmitError> {
        self.try_submit(CaptureManagerRequest::CaptureDesktopBackdrop(request))
    }

    pub fn request_image_delivery(
        &mut self,
        request: ImageDeliveryRequest,
    ) -> Result<CaptureRequestId, CaptureSubmitError> {
        self.try_submit(CaptureManagerRequest::DeliverImage(request))
    }

    pub fn request_document_delivery(
        &mut self,
        request: DocumentDeliveryRequest,
    ) -> Result<CaptureRequestId, CaptureSubmitError> {
        self.try_submit(CaptureManagerRequest::DeliverDocument(request))
    }

    fn try_submit(
        &mut self,
        request: CaptureManagerRequest,
    ) -> Result<CaptureRequestId, CaptureSubmitError> {
        if !self.healthy {
            return Err(CaptureSubmitError::Unhealthy {
                reason: "terminal worker or transport failure".to_string(),
            });
        }
        if let Some(active) = self.active {
            return Err(CaptureSubmitError::Busy {
                active_id: active.id,
            });
        }

        let Some(value) = self.next_id else {
            self.disable_worker();
            return Err(CaptureSubmitError::IdentityExhausted);
        };
        self.next_id = value.checked_add(1);
        let id = CaptureRequestId(value);
        let operation = request.operation();
        let command = CaptureCommand { id, request };
        let Some(sender) = self.request_tx.as_ref() else {
            self.disable_worker();
            return Err(CaptureSubmitError::Disconnected);
        };
        match sender.try_send(command) {
            Ok(()) => {
                self.active = Some(ActiveCapture { id, operation });
                Ok(id)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.disable_worker();
                Err(CaptureSubmitError::Disconnected)
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                let reason = "request channel was full without an active capture".to_string();
                self.disable_worker();
                Err(CaptureSubmitError::Unhealthy { reason })
            }
        }
    }

    pub fn poll(&mut self) -> CapturePoll {
        let message = self.completion_rx.try_recv();
        match (self.active, message) {
            (None, Err(TryRecvError::Empty)) => CapturePoll::Idle,
            (Some(active), Err(TryRecvError::Empty)) => CapturePoll::Pending {
                id: active.id,
                operation: active.operation,
            },
            (active, Err(TryRecvError::Disconnected)) => {
                if self.shutdown_requested.load(Ordering::Acquire) || self.terminal_reported {
                    return CapturePoll::Idle;
                }
                self.terminal_reported = true;
                self.healthy = false;
                self.active = None;
                CapturePoll::WorkerFailed {
                    active_id: active.map(|active| active.id),
                    operation: active.map(|active| active.operation),
                    error: "capture worker exited unexpectedly".to_string(),
                }
            }
            (None, Ok(completion)) => {
                let reason = format!(
                    "capture completion {} arrived without an active request",
                    completion.id
                );
                self.terminal_reported = true;
                self.disable_worker();
                CapturePoll::WorkerFailed {
                    active_id: None,
                    operation: None,
                    error: reason,
                }
            }
            (Some(active), Ok(completion)) if completion.id == active.id => {
                self.active = None;
                CapturePoll::Ready {
                    id: active.id,
                    operation: active.operation,
                    outcome: completion.outcome,
                }
            }
            (Some(active), Ok(completion)) => {
                let reason = format!(
                    "capture completion identity {}, expected {}",
                    completion.id, active.id
                );
                self.active = None;
                self.terminal_reported = true;
                self.disable_worker();
                CapturePoll::WorkerFailed {
                    active_id: Some(active.id),
                    operation: Some(active.operation),
                    error: reason,
                }
            }
        }
    }

    /// Disables future submission after an owner-side invariant failure.
    pub(crate) fn mark_unhealthy(&mut self) {
        self.terminal_reported = true;
        self.active = None;
        self.disable_worker();
    }

    /// Gets the current informational capture status.
    pub async fn get_status(&self) -> CaptureStatus {
        self.status.lock().await.clone()
    }

    /// Stops the owned worker without reporting normal teardown as failure.
    pub fn shutdown(&mut self) {
        self.shutdown_requested.store(true, Ordering::Release);
        self.request_tx.take();
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
        self.active = None;
    }

    fn disable_worker(&mut self) {
        self.healthy = false;
        self.shutdown_requested.store(true, Ordering::Release);
        self.request_tx.take();
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
    }
}

impl Drop for CaptureManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

async fn run_capture_worker(
    mut request_rx: mpsc::Receiver<CaptureCommand>,
    completion_tx: SyncSender<CaptureCompletion>,
    status: Arc<tokio::sync::Mutex<CaptureStatus>>,
    dependencies: Arc<CaptureDependencies>,
    notifier: CompletionNotifier,
    shutdown_requested: Arc<AtomicBool>,
) {
    let guard = CaptureWorkerExitGuard::new(completion_tx, notifier, shutdown_requested);
    while let Some(command) = request_rx.recv().await {
        log::debug!(
            "Processing capture manager request {}: {:?}",
            command.id,
            command.request
        );
        let operation = command.request.operation();
        *status.lock().await = CaptureStatus::AwaitingPermission;

        let result = match command.request {
            CaptureManagerRequest::Capture(request) => {
                perform_capture(request, dependencies.clone())
                    .await
                    .map(CaptureManagerResult::Capture)
            }
            CaptureManagerRequest::CaptureDesktopBackdrop(request) => {
                capture_desktop_backdrop(request, dependencies.clone())
                    .await
                    .map(CaptureManagerResult::DesktopBackdrop)
            }
            CaptureManagerRequest::DeliverImage(request) => {
                deliver_image(request, dependencies.clone())
                    .await
                    .map(CaptureManagerResult::Capture)
            }
            CaptureManagerRequest::DeliverDocument(request) => {
                deliver_document(request, dependencies.clone())
                    .await
                    .map(CaptureManagerResult::Capture)
            }
        };
        let outcome = outcome_and_status(result, operation, &status).await;
        if !guard.publish(CaptureCompletion {
            id: command.id,
            outcome,
        }) {
            return;
        }
    }
}

async fn outcome_and_status(
    result: Result<CaptureManagerResult, CaptureError>,
    operation: ImageOperationKind,
    status: &tokio::sync::Mutex<CaptureStatus>,
) -> CaptureOutcome {
    match result {
        Ok(CaptureManagerResult::Capture(result)) => {
            log::info!("Image operation successful: {:?}", result.saved_path);
            *status.lock().await = CaptureStatus::Success;
            CaptureOutcome::Success(result)
        }
        Ok(CaptureManagerResult::DesktopBackdrop(result)) => {
            log::info!("Desktop backdrop capture successful");
            *status.lock().await = CaptureStatus::Success;
            CaptureOutcome::DesktopBackdropSuccess(result)
        }
        Err(CaptureError::Cancelled(reason)) => {
            log::info!("Image operation cancelled: {reason}");
            *status.lock().await = CaptureStatus::Cancelled(reason.clone());
            CaptureOutcome::Cancelled { operation, reason }
        }
        Err(error) => {
            let message = operation.format_error(&error);
            log::error!("Image operation failed: {message}");
            *status.lock().await = CaptureStatus::Failed(message.clone());
            CaptureOutcome::Failed { operation, message }
        }
    }
}

struct CaptureWorkerExitGuard {
    completion_tx: Option<SyncSender<CaptureCompletion>>,
    notifier: CompletionNotifier,
    shutdown_requested: Arc<AtomicBool>,
}

impl CaptureWorkerExitGuard {
    fn new(
        completion_tx: SyncSender<CaptureCompletion>,
        notifier: CompletionNotifier,
        shutdown_requested: Arc<AtomicBool>,
    ) -> Self {
        Self {
            completion_tx: Some(completion_tx),
            notifier,
            shutdown_requested,
        }
    }

    fn publish(&self, completion: CaptureCompletion) -> bool {
        let result = self
            .completion_tx
            .as_ref()
            .expect("capture completion sender retained by worker")
            .try_send(completion);
        match result {
            Ok(()) => {
                (self.notifier)();
                true
            }
            Err(TrySendError::Full(_)) => {
                log::error!("Capture worker found an impossible full completion channel");
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }
}

impl Drop for CaptureWorkerExitGuard {
    fn drop(&mut self) {
        // Closing the producer side is the authoritative unexpected-exit state.
        self.completion_tx.take();
        if !self.shutdown_requested.load(Ordering::Acquire) {
            (self.notifier)();
        }
    }
}

#[cfg(test)]
impl CaptureManager {
    pub(crate) fn with_closed_channel_for_test() -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        let mut manager = Self::new(runtime.handle());
        manager.request_tx.take();
        if let Some(worker) = manager.worker.take() {
            worker.abort();
        }
        manager.healthy = true;
        manager.shutdown_requested.store(false, Ordering::Release);
        manager
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    fn request() -> CaptureManagerRequest {
        CaptureManagerRequest::Capture(CaptureRequest {
            capture_type: CaptureType::FullScreen,
            destination: CaptureDestination::ClipboardOnly,
            save_config: None,
        })
    }

    fn cancelled() -> CaptureOutcome {
        CaptureOutcome::Cancelled {
            operation: ImageOperationKind::Screenshot,
            reason: "test completion".to_string(),
        }
    }

    fn harness() -> (
        CaptureManager,
        mpsc::Receiver<CaptureCommand>,
        SyncSender<CaptureCompletion>,
    ) {
        let (request_tx, request_rx) = mpsc::channel(1);
        let (completion_tx, completion_rx) = std::sync::mpsc::sync_channel(1);
        let manager = CaptureManager {
            request_tx: Some(request_tx),
            completion_rx,
            active: None,
            next_id: Some(1),
            healthy: true,
            terminal_reported: false,
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            worker: None,
            status: Arc::new(tokio::sync::Mutex::new(CaptureStatus::Idle)),
        };
        (manager, request_rx, completion_tx)
    }

    #[test]
    fn accepted_request_is_unique_and_busy_until_matching_completion() {
        let (mut manager, mut request_rx, completion_tx) = harness();

        let first = manager.try_submit(request()).unwrap();
        assert!(matches!(
            manager.poll(),
            CapturePoll::Pending { id, operation }
                if id == first && operation == ImageOperationKind::Screenshot
        ));
        assert!(matches!(
            manager.try_submit(request()),
            Err(CaptureSubmitError::Busy { active_id }) if active_id == first
        ));
        assert_eq!(request_rx.try_recv().unwrap().id, first);

        completion_tx
            .try_send(CaptureCompletion {
                id: first,
                outcome: cancelled(),
            })
            .unwrap();
        assert!(matches!(
            manager.try_submit(request()),
            Err(CaptureSubmitError::Busy { active_id }) if active_id == first
        ));
        assert!(matches!(
            manager.poll(),
            CapturePoll::Ready { id, operation, .. }
                if id == first && operation == ImageOperationKind::Screenshot
        ));

        let second = manager.try_submit(request()).unwrap();
        assert!(second > first);
    }

    #[test]
    fn closed_or_impossibly_full_request_transport_never_creates_active_state() {
        let (mut disconnected, request_rx, _completion_tx) = harness();
        drop(request_rx);
        assert!(matches!(
            disconnected.try_submit(request()),
            Err(CaptureSubmitError::Disconnected)
        ));
        assert!(disconnected.active.is_none());

        let (mut full, _request_rx, _completion_tx) = harness();
        full.request_tx
            .as_ref()
            .unwrap()
            .try_send(CaptureCommand {
                id: CaptureRequestId(99),
                request: request(),
            })
            .unwrap();
        assert!(matches!(
            full.try_submit(request()),
            Err(CaptureSubmitError::Unhealthy { reason }) if reason.contains("full")
        ));
        assert!(full.active.is_none());
    }

    #[test]
    fn mismatched_or_unowned_completion_is_terminal_and_reported_once() {
        let (mut mismatch, mut request_rx, completion_tx) = harness();
        let accepted = mismatch.try_submit(request()).unwrap();
        request_rx.try_recv().unwrap();
        completion_tx
            .try_send(CaptureCompletion {
                id: CaptureRequestId(accepted.0 + 1),
                outcome: cancelled(),
            })
            .unwrap();
        assert!(matches!(
            mismatch.poll(),
            CapturePoll::WorkerFailed {
                active_id: Some(id),
                operation: Some(ImageOperationKind::Screenshot),
                error,
            } if id == accepted && error.contains("expected")
        ));
        assert!(matches!(mismatch.poll(), CapturePoll::Idle));

        let (mut unowned, _request_rx, completion_tx) = harness();
        completion_tx
            .try_send(CaptureCompletion {
                id: CaptureRequestId(7),
                outcome: cancelled(),
            })
            .unwrap();
        assert!(matches!(
            unowned.poll(),
            CapturePoll::WorkerFailed {
                active_id: None,
                operation: None,
                error,
            } if error.contains("without an active request")
        ));
        assert!(matches!(unowned.poll(), CapturePoll::Idle));
    }

    #[test]
    fn identity_exhaustion_occurs_only_after_the_last_identity_completes() {
        let (mut manager, mut request_rx, completion_tx) = harness();
        manager.next_id = Some(u64::MAX);

        let last = manager.try_submit(request()).unwrap();
        assert_eq!(last.0, u64::MAX);
        request_rx.try_recv().unwrap();
        completion_tx
            .try_send(CaptureCompletion {
                id: last,
                outcome: cancelled(),
            })
            .unwrap();
        assert!(matches!(manager.poll(), CapturePoll::Ready { id, .. } if id == last));
        assert!(matches!(
            manager.try_submit(request()),
            Err(CaptureSubmitError::IdentityExhausted)
        ));
        assert!(manager.active.is_none());
        assert!(matches!(
            manager.try_submit(request()),
            Err(CaptureSubmitError::Unhealthy { .. })
        ));
    }

    #[test]
    fn active_and_idle_disconnects_are_terminal_but_normal_shutdown_is_silent() {
        let (mut active, _request_rx, completion_tx) = harness();
        let accepted = active.try_submit(request()).unwrap();
        drop(completion_tx);
        assert!(matches!(
            active.poll(),
            CapturePoll::WorkerFailed {
                active_id: Some(id),
                operation: Some(ImageOperationKind::Screenshot),
                ..
            } if id == accepted
        ));
        assert!(matches!(active.poll(), CapturePoll::Idle));
        assert!(matches!(
            active.try_submit(request()),
            Err(CaptureSubmitError::Unhealthy { .. })
        ));

        let (mut idle, _request_rx, completion_tx) = harness();
        drop(completion_tx);
        assert!(matches!(
            idle.poll(),
            CapturePoll::WorkerFailed {
                active_id: None,
                operation: None,
                ..
            }
        ));
        assert!(matches!(idle.poll(), CapturePoll::Idle));

        let (mut shutdown, _request_rx, completion_tx) = harness();
        drop(completion_tx);
        shutdown.shutdown();
        assert!(matches!(shutdown.poll(), CapturePoll::Idle));
    }

    #[test]
    fn completion_channel_is_capacity_one_and_notifier_follows_publication() {
        let (completion_tx, completion_rx) = std::sync::mpsc::sync_channel(1);
        let notifications = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let notified = Arc::clone(&notifications);
        let shutdown = Arc::new(AtomicBool::new(true));
        let guard = CaptureWorkerExitGuard::new(
            completion_tx,
            Arc::new(move || {
                notified.fetch_add(1, Ordering::AcqRel);
            }),
            shutdown,
        );

        assert!(guard.publish(CaptureCompletion {
            id: CaptureRequestId(1),
            outcome: cancelled(),
        }));
        assert_eq!(notifications.load(Ordering::Acquire), 1);
        assert!(!guard.publish(CaptureCompletion {
            id: CaptureRequestId(2),
            outcome: cancelled(),
        }));
        assert_eq!(notifications.load(Ordering::Acquire), 1);
        assert_eq!(completion_rx.try_recv().unwrap().id, CaptureRequestId(1));
    }
}
