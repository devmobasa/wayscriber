//! Capacity-one completion transport for screen text recognition.
//!
//! Modelled on the clipboard controller: one identified request at a time, a
//! worker thread that always publishes a terminal message, and an event-loop
//! wake so the answer is never left waiting for unrelated input. Capacity one
//! is deliberate — a queued request would recognize a screen region the user
//! has already moved on from.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};

use crate::backend::wayland::RuntimeWakeHandle;

use super::{
    OcrOutcome, OcrRequest, OcrTextPublisher, TesseractRecognizer, TextRecognizer, WlCopyPublisher,
    run_request,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct OcrRequestId(u64);

impl OcrRequestId {
    /// A request identity for tests that drive consumers of a completion
    /// without running the controller that would mint one.
    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for OcrRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OcrSubmitError {
    /// Another recognition is still running.
    Busy {
        active_id: OcrRequestId,
    },
    IdentityExhausted,
    Unhealthy,
    SpawnFailed {
        reason: String,
    },
}

impl fmt::Display for OcrSubmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy { active_id } => {
                write!(formatter, "OCR request {active_id} is still active")
            }
            Self::IdentityExhausted => formatter.write_str("OCR request IDs exhausted"),
            Self::Unhealthy => formatter.write_str("OCR controller is unhealthy"),
            Self::SpawnFailed { reason } => {
                write!(formatter, "failed to spawn OCR worker: {reason}")
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum OcrPoll {
    Idle,
    /// A request is running. The identity stays with the controller until it
    /// completes, so callers only need to know that OCR is busy.
    Pending,
    Ready {
        id: OcrRequestId,
        outcome: OcrOutcome,
    },
    /// The worker panicked or exited without publishing an outcome.
    WorkerLost {
        id: OcrRequestId,
        reason: String,
    },
}

enum WorkerMessage {
    Ready {
        id: OcrRequestId,
        outcome: OcrOutcome,
    },
    Panicked {
        id: OcrRequestId,
        reason: String,
    },
}

struct ActiveRequest {
    id: OcrRequestId,
    receiver: Receiver<WorkerMessage>,
}

pub(crate) struct OcrController {
    next_id: Option<u64>,
    runtime_wake: RuntimeWakeHandle,
    active: Option<ActiveRequest>,
    healthy: bool,
}

impl OcrController {
    pub(crate) fn new(runtime_wake: RuntimeWakeHandle) -> Self {
        Self {
            next_id: Some(1),
            runtime_wake,
            active: None,
            healthy: true,
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Submit a request to the production Tesseract/`wl-copy` adapters.
    pub(crate) fn try_submit(
        &mut self,
        request: OcrRequest,
    ) -> Result<OcrRequestId, OcrSubmitError> {
        self.try_submit_with(request, TesseractRecognizer, WlCopyPublisher)
    }

    pub(crate) fn try_submit_with(
        &mut self,
        request: OcrRequest,
        recognizer: impl TextRecognizer + Send + 'static,
        publisher: impl OcrTextPublisher + Send + 'static,
    ) -> Result<OcrRequestId, OcrSubmitError> {
        self.try_submit_with_spawner(request, recognizer, publisher, |job| {
            std::thread::Builder::new()
                .name("wayscriber-ocr".to_string())
                .spawn(job)
                .map(drop)
        })
    }

    fn try_submit_with_spawner(
        &mut self,
        request: OcrRequest,
        recognizer: impl TextRecognizer + Send + 'static,
        publisher: impl OcrTextPublisher + Send + 'static,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<()>,
    ) -> Result<OcrRequestId, OcrSubmitError> {
        if !self.healthy {
            return Err(OcrSubmitError::Unhealthy);
        }
        if let Some(active) = &self.active {
            return Err(OcrSubmitError::Busy {
                active_id: active.id,
            });
        }
        let value = self.next_id.ok_or(OcrSubmitError::IdentityExhausted)?;
        let id = OcrRequestId(value);

        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let runtime_wake = self.runtime_wake.clone();
        let job = Box::new(move || {
            let guard = WorkerExitGuard::new(id, sender, runtime_wake);
            let message = match catch_unwind(AssertUnwindSafe(|| {
                run_request(request, &recognizer, &publisher)
            })) {
                Ok(outcome) => WorkerMessage::Ready { id, outcome },
                Err(payload) => WorkerMessage::Panicked {
                    id,
                    reason: panic_reason(&payload),
                },
            };
            guard.publish(message);
        });
        if let Err(err) = spawn(job) {
            return Err(OcrSubmitError::SpawnFailed {
                reason: err.to_string(),
            });
        }

        self.next_id = value.checked_add(1);
        self.active = Some(ActiveRequest { id, receiver });
        Ok(id)
    }

    pub(crate) fn poll(&mut self) -> OcrPoll {
        let Some(active) = self.active.as_ref() else {
            return OcrPoll::Idle;
        };
        let active_id = active.id;
        match active.receiver.try_recv() {
            Err(TryRecvError::Empty) => OcrPoll::Pending,
            Err(TryRecvError::Disconnected) => {
                self.active = None;
                OcrPoll::WorkerLost {
                    id: active_id,
                    reason: "OCR worker exited without an outcome".to_string(),
                }
            }
            Ok(WorkerMessage::Ready { id, outcome }) if id == active_id => {
                self.active = None;
                OcrPoll::Ready { id, outcome }
            }
            Ok(WorkerMessage::Panicked { id, reason }) if id == active_id => {
                self.active = None;
                OcrPoll::WorkerLost { id, reason }
            }
            Ok(WorkerMessage::Ready { id, .. } | WorkerMessage::Panicked { id, .. }) => {
                self.healthy = false;
                self.active = None;
                OcrPoll::WorkerLost {
                    id: active_id,
                    reason: format!(
                        "OCR worker reported request identity {id}, expected {active_id}"
                    ),
                }
            }
        }
    }
}

fn panic_reason(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "OCR worker panicked with a non-string payload".to_string()
    }
}

struct WorkerExitGuard {
    id: OcrRequestId,
    sender: Option<SyncSender<WorkerMessage>>,
    runtime_wake: RuntimeWakeHandle,
    terminal_published: bool,
}

impl WorkerExitGuard {
    fn new(
        id: OcrRequestId,
        sender: SyncSender<WorkerMessage>,
        runtime_wake: RuntimeWakeHandle,
    ) -> Self {
        Self {
            id,
            sender: Some(sender),
            runtime_wake,
            terminal_published: false,
        }
    }

    fn publish(mut self, message: WorkerMessage) {
        let sender = self
            .sender
            .take()
            .expect("OCR worker still holds its sender until publish");
        let result = sender.try_send(message);
        self.terminal_published = true;
        match result {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(_)) => {
                log::error!("OCR worker {} found an impossible full channel", self.id);
            }
        }
        if let Err(err) = self.runtime_wake.wake() {
            log::error!("Failed to wake runtime for OCR request {}: {err}", self.id);
        }
    }
}

impl Drop for WorkerExitGuard {
    fn drop(&mut self) {
        if self.terminal_published {
            return;
        }
        // Closing the worker side is the terminal publication for a disconnect.
        self.sender.take();
        if let Err(err) = self.runtime_wake.wake() {
            log::error!(
                "Failed to wake runtime for disconnected OCR request {}: {err}",
                self.id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;
    use crate::ocr::{OcrFailure, OcrLanguages, OcrSuccess, RecognizedOutput, RecognizedText};
    use crate::screen_pixels::PackedArgb32;

    struct FakeRecognizer {
        outcome: fn() -> Result<RecognizedOutput, OcrFailure>,
    }

    impl TextRecognizer for FakeRecognizer {
        fn recognize(
            &self,
            _png: &[u8],
            _languages: &OcrLanguages,
        ) -> Result<RecognizedOutput, OcrFailure> {
            (self.outcome)()
        }
    }

    struct BlockingRecognizer {
        started: mpsc::Sender<()>,
        release: std::sync::Mutex<mpsc::Receiver<()>>,
    }

    impl TextRecognizer for BlockingRecognizer {
        fn recognize(
            &self,
            _png: &[u8],
            _languages: &OcrLanguages,
        ) -> Result<RecognizedOutput, OcrFailure> {
            self.started.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            Ok(RecognizedOutput {
                text: RecognizedText::trimmed("done"),
                replaced_invalid_utf8: false,
            })
        }
    }

    struct PanickingRecognizer;

    impl TextRecognizer for PanickingRecognizer {
        fn recognize(
            &self,
            _png: &[u8],
            _languages: &OcrLanguages,
        ) -> Result<RecognizedOutput, OcrFailure> {
            panic!("expected OCR worker panic");
        }
    }

    struct NoopPublisher;

    impl OcrTextPublisher for NoopPublisher {
        fn publish(&self, _text: &str) -> Result<(), OcrFailure> {
            Ok(())
        }
    }

    fn request() -> OcrRequest {
        OcrRequest {
            pixels: PackedArgb32::new(2, 2, 8, vec![0xFF; 16]).unwrap(),
            languages: OcrLanguages::from_validated("eng".to_string()),
        }
    }

    fn controller() -> (RuntimeWakeSource, OcrController) {
        let wake = RuntimeWakeSource::new().unwrap();
        let controller = OcrController::new(wake.handle());
        (wake, controller)
    }

    fn wait_for_wake(wake: &RuntimeWakeSource) {
        assert!(
            wake.wait_readable(Some(Duration::from_secs(5))).unwrap(),
            "OCR completion did not wake the event loop"
        );
    }

    #[test]
    fn one_request_runs_off_thread_and_wakes_the_event_loop() {
        let (wake, mut controller) = controller();
        let id = controller
            .try_submit_with(
                request(),
                FakeRecognizer {
                    outcome: || {
                        Ok(RecognizedOutput {
                            text: RecognizedText::trimmed("copied text"),
                            replaced_invalid_utf8: false,
                        })
                    },
                },
                NoopPublisher,
            )
            .unwrap();

        wait_for_wake(&wake);
        let poll = controller.poll();
        assert!(matches!(
            poll,
            OcrPoll::Ready {
                id: ready,
                outcome: Ok(OcrSuccess::Copied {
                    character_count: 11,
                    replaced_invalid_utf8: false,
                }),
            } if ready == id
        ));
        assert!(!controller.is_active());
    }

    #[test]
    fn a_second_request_while_active_is_rejected_rather_than_queued() {
        let (_wake, mut controller) = controller();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let first = controller
            .try_submit_with(
                request(),
                BlockingRecognizer {
                    started: started_tx,
                    release: std::sync::Mutex::new(release_rx),
                },
                NoopPublisher,
            )
            .unwrap();
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        assert!(matches!(controller.poll(), OcrPoll::Pending));
        let error = controller
            .try_submit_with(
                request(),
                FakeRecognizer {
                    outcome: || panic!("busy submission must not run"),
                },
                NoopPublisher,
            )
            .unwrap_err();
        assert_eq!(error, OcrSubmitError::Busy { active_id: first });

        release_tx.send(()).unwrap();
    }

    #[test]
    fn engine_failures_reach_the_event_loop_as_typed_outcomes() {
        let (wake, mut controller) = controller();
        controller
            .try_submit_with(
                request(),
                FakeRecognizer {
                    outcome: || Err(OcrFailure::TimedOut),
                },
                NoopPublisher,
            )
            .unwrap();

        wait_for_wake(&wake);
        assert!(matches!(
            controller.poll(),
            OcrPoll::Ready {
                outcome: Err(OcrFailure::TimedOut),
                ..
            }
        ));
    }

    #[test]
    fn worker_panic_is_reported_without_poisoning_the_controller() {
        let (wake, mut controller) = controller();
        let id = controller
            .try_submit_with(request(), PanickingRecognizer, NoopPublisher)
            .unwrap();

        wait_for_wake(&wake);
        assert!(matches!(
            controller.poll(),
            OcrPoll::WorkerLost { id: lost, reason } if lost == id && reason.contains("expected OCR worker panic")
        ));
        assert!(
            controller
                .try_submit_with(
                    request(),
                    FakeRecognizer {
                        outcome: || {
                            Ok(RecognizedOutput {
                                text: RecognizedText::trimmed("after panic"),
                                replaced_invalid_utf8: false,
                            })
                        },
                    },
                    NoopPublisher,
                )
                .is_ok()
        );
    }

    #[test]
    fn a_worker_that_exits_without_publishing_is_reported_as_lost() {
        let (wake, mut controller) = controller();
        let id = OcrRequestId(9);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        controller.active = Some(ActiveRequest { id, receiver });

        drop(WorkerExitGuard::new(id, sender, wake.handle()));

        wait_for_wake(&wake);
        assert!(matches!(
            controller.poll(),
            OcrPoll::WorkerLost { id: lost, .. } if lost == id
        ));
    }

    #[test]
    fn spawn_failure_leaves_no_active_request() {
        let (_wake, mut controller) = controller();
        let error = controller
            .try_submit_with_spawner(
                request(),
                FakeRecognizer {
                    outcome: || Err(OcrFailure::EngineFailed),
                },
                NoopPublisher,
                |_job| Err(std::io::Error::other("injected spawn failure")),
            )
            .unwrap_err();

        assert_eq!(
            error,
            OcrSubmitError::SpawnFailed {
                reason: "injected spawn failure".to_string(),
            }
        );
        assert!(!controller.is_active());
    }

    #[test]
    fn identity_is_never_reused_and_exhaustion_is_reported() {
        let (wake, mut controller) = controller();
        controller.next_id = Some(u64::MAX);
        let id = controller
            .try_submit_with(
                request(),
                FakeRecognizer {
                    outcome: || {
                        Ok(RecognizedOutput {
                            text: RecognizedText::trimmed("x"),
                            replaced_invalid_utf8: false,
                        })
                    },
                },
                NoopPublisher,
            )
            .unwrap();
        assert_eq!(id, OcrRequestId(u64::MAX));

        wait_for_wake(&wake);
        assert!(matches!(controller.poll(), OcrPoll::Ready { .. }));
        assert_eq!(
            controller
                .try_submit_with(
                    request(),
                    FakeRecognizer {
                        outcome: || Err(OcrFailure::EngineFailed),
                    },
                    NoopPublisher,
                )
                .unwrap_err(),
            OcrSubmitError::IdentityExhausted
        );
    }
}
