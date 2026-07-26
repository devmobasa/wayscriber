use std::fmt;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

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

type EventNotifier = Box<dyn Fn() + Send + 'static>;

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

/// Ordered worker event observed by the thread that owns the capture manager.
#[derive(Debug)]
pub(crate) enum CaptureManagerEvent {
    Idle,
    Pending {
        id: CaptureRequestId,
        operation: ImageOperationKind,
    },
    Status {
        id: CaptureRequestId,
        operation: ImageOperationKind,
        status: CaptureStatus,
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

enum CaptureWorkerEvent {
    Status {
        id: CaptureRequestId,
        operation: ImageOperationKind,
        status: CaptureStatus,
    },
    Completed {
        id: CaptureRequestId,
        operation: ImageOperationKind,
        status: CaptureStatus,
        outcome: CaptureOutcome,
    },
    ExitedUnexpectedly,
}

#[derive(Debug, Clone, Copy)]
struct ActiveCapture {
    id: CaptureRequestId,
    operation: ImageOperationKind,
}

/// Unique owner for bounded, identified asynchronous capture operations.
///
/// Production Wayland code installs an event notifier backed by its runtime
/// wake source. Other callers may use [`CaptureManager::new`] and poll the
/// manager directly.
pub struct CaptureManager {
    request_tx: Option<mpsc::Sender<CaptureCommand>>,
    event_rx: Option<Receiver<CaptureWorkerEvent>>,
    active: Option<ActiveCapture>,
    next_id: Option<u64>,
    healthy: bool,
    terminal_reported: bool,
    status: CaptureStatus,
    worker: Option<tokio::task::JoinHandle<()>>,
}

impl CaptureManager {
    /// Creates a manager whose owner polls completions directly.
    #[cfg(test)]
    pub fn new(runtime_handle: &tokio::runtime::Handle) -> Self {
        Self::with_dependencies_and_notifier(
            runtime_handle,
            CaptureDependencies::default(),
            Box::new(|| {}),
        )
    }

    /// Creates a manager with custom dependencies for deterministic consumers.
    #[cfg(test)]
    pub(crate) fn with_dependencies(
        runtime_handle: &tokio::runtime::Handle,
        dependencies: CaptureDependencies,
    ) -> Self {
        Self::with_dependencies_and_notifier(runtime_handle, dependencies, Box::new(|| {}))
    }

    pub(crate) fn with_event_notifier(
        runtime_handle: &tokio::runtime::Handle,
        process_broker: crate::process_broker::ProcessBrokerHandle,
        notifier: impl Fn() + Send + 'static,
    ) -> Self {
        Self::with_dependencies_and_notifier(
            runtime_handle,
            CaptureDependencies::production(process_broker),
            Box::new(notifier),
        )
    }

    #[cfg(test)]
    pub(crate) fn with_dependencies_and_test_notifier(
        runtime_handle: &tokio::runtime::Handle,
        dependencies: CaptureDependencies,
        notifier: impl Fn() + Send + 'static,
    ) -> Self {
        Self::with_dependencies_and_notifier(runtime_handle, dependencies, Box::new(notifier))
    }

    fn with_dependencies_and_notifier(
        runtime_handle: &tokio::runtime::Handle,
        dependencies: CaptureDependencies,
        notifier: EventNotifier,
    ) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<CaptureCommand>(1);
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let worker = runtime_handle.spawn(run_capture_worker(
            request_rx,
            event_tx,
            dependencies,
            notifier,
        ));

        Self {
            request_tx: Some(request_tx),
            event_rx: Some(event_rx),
            active: None,
            next_id: Some(1),
            healthy: true,
            terminal_reported: false,
            status: CaptureStatus::Idle,
            worker: Some(worker),
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

    /// Polls for a public completion while applying any preceding status events.
    pub fn poll(&mut self) -> CapturePoll {
        loop {
            match self.poll_event() {
                CaptureManagerEvent::Idle => return CapturePoll::Idle,
                CaptureManagerEvent::Pending { id, operation } => {
                    return CapturePoll::Pending { id, operation };
                }
                CaptureManagerEvent::Status { .. } => {}
                CaptureManagerEvent::Ready {
                    id,
                    operation,
                    outcome,
                } => {
                    return CapturePoll::Ready {
                        id,
                        operation,
                        outcome,
                    };
                }
                CaptureManagerEvent::WorkerFailed {
                    active_id,
                    operation,
                    error,
                } => {
                    return CapturePoll::WorkerFailed {
                        active_id,
                        operation,
                        error,
                    };
                }
            }
        }
    }

    /// Polls the next ordered worker event on the manager-owning thread.
    pub(crate) fn poll_event(&mut self) -> CaptureManagerEvent {
        if self.terminal_reported {
            return CaptureManagerEvent::Idle;
        }
        let Some(event_rx) = self.event_rx.as_ref() else {
            return CaptureManagerEvent::Idle;
        };
        match (self.active, event_rx.try_recv()) {
            (None, Err(TryRecvError::Empty)) => CaptureManagerEvent::Idle,
            (Some(active), Err(TryRecvError::Empty)) => CaptureManagerEvent::Pending {
                id: active.id,
                operation: active.operation,
            },
            (active, Err(TryRecvError::Disconnected)) => {
                self.terminal_reported = true;
                self.active = None;
                self.disable_worker();
                CaptureManagerEvent::WorkerFailed {
                    active_id: active.map(|active| active.id),
                    operation: active.map(|active| active.operation),
                    error: "capture worker exited unexpectedly".to_string(),
                }
            }
            (active, Ok(CaptureWorkerEvent::ExitedUnexpectedly)) => {
                self.terminal_reported = true;
                self.active = None;
                self.disable_worker();
                CaptureManagerEvent::WorkerFailed {
                    active_id: active.map(|capture| capture.id),
                    operation: active.map(|capture| capture.operation),
                    error: "capture worker exited unexpectedly".to_string(),
                }
            }
            (
                None,
                Ok(CaptureWorkerEvent::Status {
                    id,
                    operation,
                    status: _,
                }),
            )
            | (
                None,
                Ok(CaptureWorkerEvent::Completed {
                    id,
                    operation,
                    status: _,
                    outcome: _,
                }),
            ) => {
                let reason =
                    format!("capture event {id} ({operation:?}) arrived without an active request");
                self.terminal_reported = true;
                self.disable_worker();
                CaptureManagerEvent::WorkerFailed {
                    active_id: None,
                    operation: None,
                    error: reason,
                }
            }
            (
                Some(active),
                Ok(CaptureWorkerEvent::Status {
                    id,
                    operation,
                    status,
                }),
            ) if id == active.id && operation == active.operation => {
                self.status = status.clone();
                CaptureManagerEvent::Status {
                    id,
                    operation,
                    status,
                }
            }
            (
                Some(active),
                Ok(CaptureWorkerEvent::Completed {
                    id,
                    operation,
                    status,
                    outcome,
                }),
            ) if id == active.id && operation == active.operation => {
                self.active = None;
                self.status = status;
                CaptureManagerEvent::Ready {
                    id,
                    operation,
                    outcome,
                }
            }
            (
                Some(active),
                Ok(CaptureWorkerEvent::Status {
                    id,
                    operation,
                    status: _,
                }),
            )
            | (
                Some(active),
                Ok(CaptureWorkerEvent::Completed {
                    id,
                    operation,
                    status: _,
                    outcome: _,
                }),
            ) => {
                let reason = format!(
                    "capture event identity {id} ({operation:?}), expected {} ({:?})",
                    active.id, active.operation
                );
                self.active = None;
                self.terminal_reported = true;
                self.disable_worker();
                CaptureManagerEvent::WorkerFailed {
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

    /// Returns the last informational status observed while this owner polled.
    ///
    /// Worker-side changes become visible here only after [`Self::poll`] (or the
    /// application runtime's ordered event polling) applies the corresponding
    /// event on the thread that owns this manager.
    pub async fn get_status(&self) -> CaptureStatus {
        self.status.clone()
    }

    /// Stops the owned worker without reporting normal teardown as failure.
    pub fn shutdown(&mut self) {
        self.request_tx.take();
        self.event_rx.take();
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
        self.active = None;
    }

    fn disable_worker(&mut self) {
        self.healthy = false;
        self.request_tx.take();
        self.event_rx.take();
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
    event_tx: Sender<CaptureWorkerEvent>,
    mut dependencies: CaptureDependencies,
    notifier: EventNotifier,
) {
    let mut reporter = CaptureWorkerReporter::new(event_tx, notifier);
    while let Some(command) = request_rx.recv().await {
        log::debug!(
            "Processing capture manager request {}: {:?}",
            command.id,
            command.request
        );
        let operation = command.request.operation();
        if !reporter.publish(CaptureWorkerEvent::Status {
            id: command.id,
            operation,
            status: CaptureStatus::AwaitingPermission,
        }) {
            reporter.finish();
            return;
        }

        let result = match command.request {
            CaptureManagerRequest::Capture(request) => perform_capture(request, &mut dependencies)
                .await
                .map(CaptureManagerResult::Capture),
            CaptureManagerRequest::CaptureDesktopBackdrop(request) => {
                capture_desktop_backdrop(request, &mut dependencies)
                    .await
                    .map(CaptureManagerResult::DesktopBackdrop)
            }
            CaptureManagerRequest::DeliverImage(request) => {
                deliver_image(request, &mut dependencies)
                    .await
                    .map(CaptureManagerResult::Capture)
            }
            CaptureManagerRequest::DeliverDocument(request) => {
                deliver_document(request, &mut dependencies)
                    .await
                    .map(CaptureManagerResult::Capture)
            }
        };
        let (status, outcome) = outcome_and_status(result, operation);
        if !reporter.publish(CaptureWorkerEvent::Completed {
            id: command.id,
            operation,
            status,
            outcome,
        }) {
            reporter.finish();
            return;
        }
    }
    reporter.finish();
}

fn outcome_and_status(
    result: Result<CaptureManagerResult, CaptureError>,
    operation: ImageOperationKind,
) -> (CaptureStatus, CaptureOutcome) {
    match result {
        Ok(CaptureManagerResult::Capture(result)) => {
            log::info!("Image operation successful: {:?}", result.saved_path);
            (CaptureStatus::Success, CaptureOutcome::Success(result))
        }
        Ok(CaptureManagerResult::DesktopBackdrop(result)) => {
            log::info!("Desktop backdrop capture successful");
            (
                CaptureStatus::Success,
                CaptureOutcome::DesktopBackdropSuccess(result),
            )
        }
        Err(CaptureError::Cancelled(reason)) => {
            log::info!("Image operation cancelled: {reason}");
            (
                CaptureStatus::Cancelled(reason.clone()),
                CaptureOutcome::Cancelled { operation, reason },
            )
        }
        Err(error) => {
            let message = operation.format_error(&error);
            log::error!("Image operation failed: {message}");
            (
                CaptureStatus::Failed(message.clone()),
                CaptureOutcome::Failed { operation, message },
            )
        }
    }
}

struct CaptureWorkerReporter {
    event_tx: Option<Sender<CaptureWorkerEvent>>,
    notifier: EventNotifier,
}

impl CaptureWorkerReporter {
    fn new(event_tx: Sender<CaptureWorkerEvent>, notifier: EventNotifier) -> Self {
        Self {
            event_tx: Some(event_tx),
            notifier,
        }
    }

    fn publish(&self, event: CaptureWorkerEvent) -> bool {
        let Some(event_tx) = self.event_tx.as_ref() else {
            return false;
        };
        if event_tx.send(event).is_err() {
            return false;
        }
        (self.notifier)();
        true
    }

    fn finish(&mut self) {
        self.event_tx.take();
    }
}

impl Drop for CaptureWorkerReporter {
    fn drop(&mut self) {
        if let Some(event_tx) = self.event_tx.take()
            && event_tx
                .send(CaptureWorkerEvent::ExitedUnexpectedly)
                .is_ok()
        {
            (self.notifier)();
        }
    }
}

#[cfg(test)]
impl CaptureManager {
    pub(crate) fn with_closed_channel_for_test() -> Self {
        let runtime = tokio::runtime::Runtime::new()
            .expect("fixture creates a Tokio runtime before constructing its manager");
        let mut manager = Self::new(runtime.handle());
        manager.request_tx.take();
        manager.event_rx.take();
        if let Some(worker) = manager.worker.take() {
            worker.abort();
        }
        manager.healthy = true;
        manager
    }

    pub(crate) fn abort_worker_for_test(&mut self) -> bool {
        let Some(worker) = self.worker.take() else {
            return false;
        };
        worker.abort();
        true
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
        Sender<CaptureWorkerEvent>,
    ) {
        let (request_tx, request_rx) = mpsc::channel(1);
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let manager = CaptureManager {
            request_tx: Some(request_tx),
            event_rx: Some(event_rx),
            active: None,
            next_id: Some(1),
            healthy: true,
            terminal_reported: false,
            status: CaptureStatus::Idle,
            worker: None,
        };
        (manager, request_rx, event_tx)
    }

    fn completed(id: CaptureRequestId) -> CaptureWorkerEvent {
        CaptureWorkerEvent::Completed {
            id,
            operation: ImageOperationKind::Screenshot,
            status: CaptureStatus::Cancelled("test completion".to_string()),
            outcome: cancelled(),
        }
    }

    #[test]
    fn accepted_request_is_busy_until_ordered_status_and_completion_events_arrive() {
        let (mut manager, mut request_rx, event_tx) = harness();

        let first = manager
            .try_submit(request())
            .expect("fixture transport accepts its first request");
        assert!(matches!(
            manager.poll_event(),
            CaptureManagerEvent::Pending { id, operation }
                if id == first && operation == ImageOperationKind::Screenshot
        ));
        assert!(matches!(
            manager.try_submit(request()),
            Err(CaptureSubmitError::Busy { active_id }) if active_id == first
        ));
        assert_eq!(
            request_rx
                .try_recv()
                .expect("fixture request channel contains the accepted request")
                .id,
            first
        );

        event_tx
            .send(CaptureWorkerEvent::Status {
                id: first,
                operation: ImageOperationKind::Screenshot,
                status: CaptureStatus::AwaitingPermission,
            })
            .expect("fixture event receiver remains live for the status event");
        assert!(matches!(
            manager.try_submit(request()),
            Err(CaptureSubmitError::Busy { active_id }) if active_id == first
        ));
        assert!(matches!(
            manager.poll_event(),
            CaptureManagerEvent::Status {
                id,
                operation: ImageOperationKind::Screenshot,
                status: CaptureStatus::AwaitingPermission,
            } if id == first
        ));
        assert_eq!(manager.status, CaptureStatus::AwaitingPermission);

        event_tx
            .send(completed(first))
            .expect("fixture event receiver remains live for the completion event");
        assert!(matches!(
            manager.try_submit(request()),
            Err(CaptureSubmitError::Busy { active_id }) if active_id == first
        ));
        assert!(matches!(
            manager.poll_event(),
            CaptureManagerEvent::Ready {
                id,
                operation: ImageOperationKind::Screenshot,
                ..
            } if id == first
        ));
        assert!(matches!(
            &manager.status,
            CaptureStatus::Cancelled(reason) if reason == "test completion"
        ));

        let second = manager
            .try_submit(request())
            .expect("completed fixture request releases the transport for a second request");
        assert!(second > first);
    }

    #[test]
    fn closed_or_impossibly_full_request_transport_never_creates_active_state() {
        let (mut disconnected, request_rx, _event_tx) = harness();
        drop(request_rx);
        assert!(matches!(
            disconnected.try_submit(request()),
            Err(CaptureSubmitError::Disconnected)
        ));
        assert!(disconnected.active.is_none());

        let (mut full, _request_rx, _event_tx) = harness();
        full.request_tx
            .as_ref()
            .expect("fixture harness always installs a request sender")
            .try_send(CaptureCommand {
                id: CaptureRequestId(99),
                request: request(),
            })
            .expect("empty fixture request channel accepts its prefill command");
        assert!(matches!(
            full.try_submit(request()),
            Err(CaptureSubmitError::Unhealthy { reason }) if reason.contains("full")
        ));
        assert!(full.active.is_none());
    }

    #[test]
    fn mismatched_or_unowned_event_is_terminal_and_reported_once() {
        let (mut mismatch, mut request_rx, event_tx) = harness();
        let accepted = mismatch
            .try_submit(request())
            .expect("fixture transport accepts the request used for identity mismatch");
        request_rx
            .try_recv()
            .expect("fixture request channel contains the mismatch request");
        event_tx
            .send(CaptureWorkerEvent::Completed {
                id: CaptureRequestId(accepted.0 + 1),
                operation: ImageOperationKind::Screenshot,
                status: CaptureStatus::Cancelled("test completion".to_string()),
                outcome: cancelled(),
            })
            .expect("fixture event receiver remains live for the mismatched event");
        assert!(matches!(
            mismatch.poll(),
            CapturePoll::WorkerFailed {
                active_id: Some(id),
                operation: Some(ImageOperationKind::Screenshot),
                error,
            } if id == accepted && error.contains("expected")
        ));
        assert!(matches!(mismatch.poll(), CapturePoll::Idle));

        let (mut unowned, _request_rx, event_tx) = harness();
        event_tx
            .send(CaptureWorkerEvent::Status {
                id: CaptureRequestId(7),
                operation: ImageOperationKind::Screenshot,
                status: CaptureStatus::AwaitingPermission,
            })
            .expect("fixture event receiver remains live for the unowned event");
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
    fn buffered_event_after_terminal_shutdown_is_discarded() {
        let (mut manager, _request_rx, event_tx) = harness();
        event_tx
            .send(completed(CaptureRequestId(7)))
            .expect("fixture event receiver remains live before terminal shutdown");
        manager.mark_unhealthy();

        assert!(matches!(manager.poll(), CapturePoll::Idle));
        assert!(matches!(manager.poll(), CapturePoll::Idle));
    }

    #[test]
    fn identity_exhaustion_occurs_only_after_the_last_identity_completes() {
        let (mut manager, mut request_rx, event_tx) = harness();
        manager.next_id = Some(u64::MAX);

        let last = manager
            .try_submit(request())
            .expect("fixture transport accepts the last available request identity");
        assert_eq!(last.0, u64::MAX);
        request_rx
            .try_recv()
            .expect("fixture request channel contains the last-identity request");
        event_tx
            .send(completed(last))
            .expect("fixture event receiver remains live for the last completion");
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
        let (mut active, _request_rx, event_tx) = harness();
        let accepted = active
            .try_submit(request())
            .expect("fixture transport accepts the request used for active disconnect");
        drop(event_tx);
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

        let (mut idle, _request_rx, event_tx) = harness();
        drop(event_tx);
        assert!(matches!(
            idle.poll(),
            CapturePoll::WorkerFailed {
                active_id: None,
                operation: None,
                ..
            }
        ));
        assert!(matches!(idle.poll(), CapturePoll::Idle));

        let (mut shutdown, _request_rx, event_tx) = harness();
        drop(event_tx);
        shutdown.shutdown();
        assert!(matches!(shutdown.poll(), CapturePoll::Idle));
    }

    #[test]
    fn reporter_notifies_after_publication_and_reports_only_unexpected_exit() {
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let (notification_tx, notification_rx) = std::sync::mpsc::channel();
        let mut reporter = CaptureWorkerReporter::new(
            event_tx,
            Box::new(move || {
                let _ = notification_tx.send(());
            }),
        );

        assert!(reporter.publish(CaptureWorkerEvent::Status {
            id: CaptureRequestId(1),
            operation: ImageOperationKind::Screenshot,
            status: CaptureStatus::AwaitingPermission,
        }));
        assert!(matches!(
            event_rx.try_recv(),
            Ok(CaptureWorkerEvent::Status {
                id: CaptureRequestId(1),
                ..
            })
        ));
        assert!(notification_rx.try_recv().is_ok());

        reporter.finish();
        drop(reporter);
        assert!(matches!(
            event_rx.try_recv(),
            Err(TryRecvError::Empty | TryRecvError::Disconnected)
        ));
        assert!(matches!(
            notification_rx.try_recv(),
            Err(TryRecvError::Empty | TryRecvError::Disconnected)
        ));

        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let (notification_tx, notification_rx) = std::sync::mpsc::channel();
        let reporter = CaptureWorkerReporter::new(
            event_tx,
            Box::new(move || {
                let _ = notification_tx.send(());
            }),
        );
        drop(reporter);
        assert!(matches!(
            event_rx.try_recv(),
            Ok(CaptureWorkerEvent::ExitedUnexpectedly)
        ));
        assert!(notification_rx.try_recv().is_ok());
    }
}
