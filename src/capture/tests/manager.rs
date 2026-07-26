use std::{
    io::{Read, Write},
    os::fd::AsRawFd,
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
};

use tokio::{
    sync::oneshot,
    time::{Duration, sleep},
};

use crate::capture::{
    DesktopBackdropCaptureRequest, DocumentDeliveryRequest, ImageDeliveryRequest,
    ImageFormatMetadata, ImageOperationKind, RenderedDocument, RenderedImage,
    dependencies::{CaptureDependencies, CaptureFuture, CaptureSource},
    file::FileSaveConfig,
    manager::{CaptureManager, CaptureManagerEvent, CapturePoll, CaptureSubmitError},
    types::{
        CaptureDestination, CaptureError, CaptureOutcome, CaptureResult, CaptureStatus, CaptureType,
    },
};

use super::fixtures::{MockClipboard, MockSaver, MockSource, create_placeholder_image};

struct CaptureGate {
    started_tx: oneshot::Sender<()>,
    release_rx: oneshot::Receiver<()>,
}

struct GatedSource {
    gate: Option<CaptureGate>,
}

impl GatedSource {
    fn fixture() -> (Self, oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        (
            Self {
                gate: Some(CaptureGate {
                    started_tx,
                    release_rx,
                }),
            },
            started_rx,
            release_tx,
        )
    }
}

impl CaptureSource for GatedSource {
    fn capture(&mut self, _capture_type: CaptureType) -> CaptureFuture<'_> {
        let Some(gate) = self.gate.take() else {
            return Box::pin(async {
                Err(CaptureError::ImageError(
                    "gated test source received a second capture request".to_string(),
                ))
            });
        };
        Box::pin(async move {
            gate.started_tx.send(()).map_err(|()| {
                CaptureError::ImageError("gated test observer was dropped".to_string())
            })?;
            gate.release_rx.await.map_err(|_| {
                CaptureError::ImageError("gated test release was dropped".to_string())
            })?;
            Ok(vec![1, 2, 3])
        })
    }
}

async fn wait_for_notifications(
    notifications: &Receiver<()>,
    expected: usize,
) -> Result<(), String> {
    let mut observed = 0;
    for _ in 0..100 {
        loop {
            match notifications.try_recv() {
                Ok(()) => {
                    observed += 1;
                    if observed >= expected {
                        return Ok(());
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err(format!(
                        "notification fixture disconnected after {observed} of {expected} events"
                    ));
                }
            }
        }
        sleep(Duration::from_millis(10)).await;
    }
    Err(format!(
        "capture worker published {observed} of {expected} expected event notifications"
    ))
}

async fn wait_for_manager_completion(
    manager: &mut CaptureManager,
) -> Result<(CaptureStatus, CaptureOutcome), String> {
    for _ in 0..100 {
        match manager.poll_event() {
            CaptureManagerEvent::Ready { outcome, .. } => {
                return Ok((manager.get_status().await, outcome));
            }
            CaptureManagerEvent::WorkerFailed { error, .. } => {
                return Err(format!(
                    "capture worker failed while awaiting its fixture outcome: {error}"
                ));
            }
            CaptureManagerEvent::Idle
            | CaptureManagerEvent::Pending { .. }
            | CaptureManagerEvent::Status { .. } => {}
        }
        sleep(Duration::from_millis(20)).await;
    }
    Err("capture fixture did not complete within its bounded polling window".to_string())
}

fn successful_result(
    completion: (CaptureStatus, CaptureOutcome),
    fixture_invariant: &str,
) -> CaptureResult {
    match completion {
        (CaptureStatus::Success, CaptureOutcome::Success(result)) => Some(result),
        _ => None,
    }
    .expect(fixture_invariant)
}

fn failed_result(
    completion: (CaptureStatus, CaptureOutcome),
    fixture_invariant: &str,
) -> (String, ImageOperationKind, String) {
    match completion {
        (CaptureStatus::Failed(status), CaptureOutcome::Failed { operation, message }) => {
            Some((status, operation, message))
        }
        _ => None,
    }
    .expect(fixture_invariant)
}

fn rendered_png(bytes: Vec<u8>) -> RenderedImage {
    RenderedImage {
        bytes,
        format: ImageFormatMetadata::png(),
        width: 1,
        height: 1,
    }
}

fn rendered_pdf(bytes: Vec<u8>) -> RenderedDocument {
    RenderedDocument {
        bytes,
        extension: "pdf".to_string(),
        mime_type: "application/pdf".to_string(),
    }
}

#[tokio::test]
async fn new_capture_manager_has_no_pending_worker_event() {
    let mut manager = CaptureManager::new(&tokio::runtime::Handle::current());
    assert!(matches!(manager.poll(), CapturePoll::Idle));
    assert_eq!(manager.get_status().await, CaptureStatus::Idle);
}

#[tokio::test]
async fn coalesced_status_and_completion_are_observable_in_event_order() {
    let deps = CaptureDependencies {
        source: Box::new(MockSource::succeeding(vec![1, 2, 3])),
        saver: Box::new(MockSaver::succeeding("/tmp/notified.png")),
        clipboard: Box::new(MockClipboard::succeeding()),
    };
    let (notification_tx, notification_rx) = mpsc::channel();
    let mut manager = CaptureManager::with_dependencies_and_test_notifier(
        &tokio::runtime::Handle::current(),
        deps,
        move || {
            let _ = notification_tx.send(());
        },
    );
    let accepted = manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .expect("fixture manager accepts its first capture request");

    // A non-semaphore eventfd may coalesce both notifications into one wake.
    // Both already-published events must still drain in worker order.
    wait_for_notifications(&notification_rx, 2)
        .await
        .expect("fixture pipeline publishes one status and one completion notification");

    assert!(matches!(
        manager.poll_event(),
        CaptureManagerEvent::Status {
            id,
            operation: ImageOperationKind::Screenshot,
            status: CaptureStatus::AwaitingPermission,
        } if id == accepted
    ));
    assert_eq!(
        manager.get_status().await,
        CaptureStatus::AwaitingPermission
    );
    assert!(matches!(
        manager.poll_event(),
        CaptureManagerEvent::Ready { id, .. } if id == accepted
    ));
    assert_eq!(manager.get_status().await, CaptureStatus::Success);
    assert!(matches!(
        notification_rx.try_recv(),
        Err(TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn public_poll_projects_status_events_and_retains_the_last_observed_status() {
    let dependencies = CaptureDependencies {
        source: Box::new(MockSource::succeeding(vec![1, 2, 3])),
        saver: Box::new(MockSaver::succeeding("/tmp/public-poll.png")),
        clipboard: Box::new(MockClipboard::succeeding()),
    };
    let (notification_tx, notification_rx) = mpsc::channel();
    let mut manager = CaptureManager::with_dependencies_and_test_notifier(
        &tokio::runtime::Handle::current(),
        dependencies,
        move || {
            let _ = notification_tx.send(());
        },
    );
    let accepted = manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .expect("fixture manager accepts its public-poll request");

    wait_for_notifications(&notification_rx, 2)
        .await
        .expect("fixture pipeline publishes status before public-poll completion");

    let projected_completion = match manager.poll() {
        CapturePoll::Ready {
            id,
            operation: ImageOperationKind::Screenshot,
            outcome: CaptureOutcome::Success(_),
        } => id == accepted,
        CapturePoll::Idle
        | CapturePoll::Pending { .. }
        | CapturePoll::Ready { .. }
        | CapturePoll::WorkerFailed { .. } => false,
    };
    assert!(
        projected_completion,
        "fixture public poll projects queued status events before completion"
    );
    assert_eq!(manager.get_status().await, CaptureStatus::Success);
}

#[tokio::test(flavor = "multi_thread")]
async fn completion_wakes_an_already_blocked_runtime_poll_after_publication() {
    let (source, started_rx, release_tx) = GatedSource::fixture();
    let deps = CaptureDependencies {
        source: Box::new(source),
        saver: Box::new(MockSaver::succeeding("/tmp/gated.png")),
        clipboard: Box::new(MockClipboard::succeeding()),
    };
    let (wake_receiver, wake_sender) =
        UnixStream::pair().expect("fixture creates a connected wake socket pair");
    let mut manager = CaptureManager::with_dependencies_and_test_notifier(
        &tokio::runtime::Handle::current(),
        deps,
        move || {
            let mut writer = &wake_sender;
            writer
                .write_all(&[1])
                .expect("fixture wake socket remains open while the worker can notify");
        },
    );
    let accepted = manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .expect("fixture manager accepts its gated capture request");
    started_rx
        .await
        .expect("fixture source announces that its single capture started");
    assert!(matches!(
        manager.poll_event(),
        CaptureManagerEvent::Status {
            id,
            status: CaptureStatus::AwaitingPermission,
            ..
        } if id == accepted
    ));
    let mut status_wake = [0_u8; 1];
    let mut wake_reader = &wake_receiver;
    wake_reader
        .read_exact(&mut status_wake)
        .expect("fixture reads the published status wake from its connected socket");

    let wake_fd = wake_receiver.as_raw_fd();
    let (polling_tx, polling_rx) = mpsc::channel();
    let poller = std::thread::spawn(move || {
        let mut pollfd = libc::pollfd {
            fd: wake_fd,
            events: libc::POLLIN,
            revents: 0,
        };
        polling_tx
            .send(())
            .expect("fixture poll coordinator remains alive until polling starts");
        // SAFETY: the wake receiver outlives this bounded poll and owns
        // the descriptor for the entire test.
        assert_eq!(unsafe { libc::poll(&mut pollfd, 1, 1_000) }, 1);
        assert_ne!(pollfd.revents & libc::POLLIN, 0);
    });
    polling_rx
        .recv()
        .expect("fixture poll thread announces entry into its bounded poll");
    release_tx
        .send(())
        .expect("fixture gated source remains active until capture is released");
    poller
        .join()
        .expect("fixture poll thread completes without an assertion failure");
    let mut completion_wake = [0_u8; 1];
    wake_reader
        .read_exact(&mut completion_wake)
        .expect("fixture reads the published completion wake from its connected socket");

    assert!(matches!(
        manager.poll_event(),
        CaptureManagerEvent::Ready { id, .. } if id == accepted
    ));
}

#[tokio::test]
async fn unexpected_worker_exit_wakes_and_reports_the_active_operation_once() {
    let (source, started_rx, _release_tx) = GatedSource::fixture();
    let deps = CaptureDependencies {
        source: Box::new(source),
        saver: Box::new(MockSaver::succeeding("/tmp/unreachable.png")),
        clipboard: Box::new(MockClipboard::succeeding()),
    };
    let (notification_tx, notification_rx) = mpsc::channel();
    let mut manager = CaptureManager::with_dependencies_and_test_notifier(
        &tokio::runtime::Handle::current(),
        deps,
        move || {
            let _ = notification_tx.send(());
        },
    );
    let accepted = manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .expect("fixture manager accepts its gated capture request");
    started_rx
        .await
        .expect("fixture source announces that its single capture started");
    wait_for_notifications(&notification_rx, 1)
        .await
        .expect("fixture worker publishes its initial status notification");

    assert!(matches!(
        manager.poll_event(),
        CaptureManagerEvent::Status {
            id,
            status: CaptureStatus::AwaitingPermission,
            ..
        } if id == accepted
    ));
    assert!(manager.abort_worker_for_test());
    wait_for_notifications(&notification_rx, 1)
        .await
        .expect("aborted fixture worker publishes one unexpected-exit notification");
    assert!(matches!(
        manager.poll_event(),
        CaptureManagerEvent::WorkerFailed {
            active_id: Some(id),
            operation: Some(ImageOperationKind::Screenshot),
            ..
        } if id == accepted
    ));
    assert!(matches!(manager.poll_event(), CaptureManagerEvent::Idle));
    assert!(matches!(
        notification_rx.try_recv(),
        Err(TryRecvError::Empty | TryRecvError::Disconnected)
    ));
}

#[tokio::test]
async fn normal_shutdown_does_not_publish_a_failure_notification() {
    let (notification_tx, notification_rx) = mpsc::channel();
    let mut manager = CaptureManager::with_dependencies_and_test_notifier(
        &tokio::runtime::Handle::current(),
        CaptureDependencies::default(),
        move || {
            let _ = notification_tx.send(());
        },
    );

    manager.shutdown();
    tokio::task::yield_now().await;

    assert!(matches!(
        notification_rx.try_recv(),
        Err(TryRecvError::Empty | TryRecvError::Disconnected)
    ));
    assert!(matches!(manager.poll(), CapturePoll::Idle));

    let (source, started_rx, _release_tx) = GatedSource::fixture();
    let dependencies = CaptureDependencies {
        source: Box::new(source),
        saver: Box::new(MockSaver::succeeding("/tmp/shutdown.png")),
        clipboard: Box::new(MockClipboard::succeeding()),
    };
    let (notification_tx, notification_rx) = mpsc::channel();
    let mut manager = CaptureManager::with_dependencies_and_test_notifier(
        &tokio::runtime::Handle::current(),
        dependencies,
        move || {
            let _ = notification_tx.send(());
        },
    );
    let accepted = manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .expect("fixture manager accepts its gated shutdown request");
    started_rx
        .await
        .expect("fixture source announces that its single capture started");
    wait_for_notifications(&notification_rx, 1)
        .await
        .expect("fixture worker publishes its initial status notification");
    assert!(matches!(
        manager.poll_event(),
        CaptureManagerEvent::Status {
            id,
            status: CaptureStatus::AwaitingPermission,
            ..
        } if id == accepted
    ));

    manager.shutdown();
    sleep(Duration::from_millis(20)).await;

    assert!(matches!(
        notification_rx.try_recv(),
        Err(TryRecvError::Empty | TryRecvError::Disconnected)
    ));
    assert!(matches!(manager.poll(), CapturePoll::Idle));
}

#[tokio::test]
async fn test_capture_manager_with_dependencies() {
    let (clipboard, copied_image_rx) = MockClipboard::recording(false);
    let deps = CaptureDependencies {
        source: Box::new(MockSource::succeeding(vec![13, 14, 15])),
        saver: Box::new(MockSaver::succeeding("/tmp/manager.png")),
        clipboard: Box::new(clipboard),
    };
    let mut manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .expect("fixture manager accepts its first clipboard capture");

    let completion = wait_for_manager_completion(&mut manager)
        .await
        .expect("fixture dependencies keep the manager worker healthy");
    let result = successful_result(
        completion,
        "successful fixture capture yields a successful capture result",
    );
    assert!(result.saved_path.is_none());
    assert!(result.copied_to_clipboard);
    assert_eq!(
        copied_image_rx
            .try_recv()
            .expect("fixture clipboard reports the manager-backed copy before returning"),
        vec![13, 14, 15]
    );
}

#[test]
fn request_capture_returns_error_when_channel_closed() {
    let mut manager = CaptureManager::with_closed_channel_for_test();
    let err = manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .expect_err("fixture closes the manager request channel before submission");
    assert!(
        matches!(err, CaptureSubmitError::Disconnected),
        "unexpected error variant: {err:?}"
    );
}

#[tokio::test]
async fn capture_manager_records_failure_status() {
    let deps = CaptureDependencies {
        source: Box::new(MockSource::succeeding(vec![99])),
        saver: Box::new(MockSaver::failing("/tmp/fail.png")),
        clipboard: Box::new(MockClipboard::succeeding()),
    };
    let mut manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::FileOnly,
            Some(FileSaveConfig::default()),
        )
        .expect("fixture manager accepts its first file capture");

    let completion = wait_for_manager_completion(&mut manager)
        .await
        .expect("fixture failure remains an operation outcome, not a worker failure");
    let (status, operation, message) = failed_result(
        completion,
        "failing saver fixture yields a failed screenshot outcome",
    );
    assert_eq!(operation, ImageOperationKind::Screenshot);
    assert!(
        message.contains("save failed"),
        "unexpected failure message: {message}"
    );
    assert_eq!(status, message);
}

#[tokio::test]
async fn capture_manager_preserves_user_cancellation_as_a_terminal_outcome() {
    let deps = CaptureDependencies {
        source: Box::new(MockSource::failing(CaptureError::Cancelled(
            "user dismissed portal".to_string(),
        ))),
        saver: Box::new(MockSaver::succeeding("/tmp/cancelled.png")),
        clipboard: Box::new(MockClipboard::succeeding()),
    };
    let mut manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);
    manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .expect("fixture manager accepts its cancellable capture");

    assert!(matches!(
        wait_for_manager_completion(&mut manager)
            .await
            .expect("fixture cancellation remains an operation outcome"),
        (
            CaptureStatus::Cancelled(status_reason),
            CaptureOutcome::Cancelled {
                operation: ImageOperationKind::Screenshot,
                reason,
            },
        ) if reason == "user dismissed portal" && status_reason == reason
    ));
}

#[tokio::test]
async fn desktop_backdrop_completion_releases_the_manager_for_pdf_delivery() {
    let deps = CaptureDependencies {
        source: Box::new(MockSource::succeeding(create_placeholder_image())),
        saver: Box::new(MockSaver::succeeding("/tmp/after-backdrop.pdf")),
        clipboard: Box::new(MockClipboard::succeeding()),
    };
    let mut manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);
    let backdrop_id = manager
        .request_desktop_backdrop_capture(DesktopBackdropCaptureRequest {
            logical_width: 100,
            logical_height: 100,
            scale: 1,
            geometry: None,
            operation: ImageOperationKind::BoardPdfExport,
        })
        .expect("fixture manager accepts its desktop backdrop capture");

    assert!(matches!(
        wait_for_manager_completion(&mut manager)
            .await
            .expect("fixture backdrop capture keeps the manager worker healthy"),
        (
            CaptureStatus::Success,
            CaptureOutcome::DesktopBackdropSuccess(backdrop)
        ) if backdrop.width == 100 && backdrop.height == 100
    ));

    let document_id = manager
        .request_document_delivery(DocumentDeliveryRequest {
            document: rendered_pdf(b"%PDF-".to_vec()),
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig::default()),
            operation: ImageOperationKind::BoardPdfExport,
        })
        .expect("completed backdrop fixture releases the manager for PDF delivery");
    assert!(document_id > backdrop_id);
    assert!(matches!(
        wait_for_manager_completion(&mut manager)
            .await
            .expect("fixture PDF delivery keeps the manager worker healthy"),
        (CaptureStatus::Success, CaptureOutcome::Success(result))
            if result.operation == ImageOperationKind::BoardPdfExport
    ));
}

#[tokio::test]
async fn request_image_delivery_queues_manager_backed_path() {
    let (source, captured_type_rx) = MockSource::recording(vec![99]);
    let (saver, saved_config_rx) = MockSaver::recording(false, "/tmp/canvas-delivery.png");
    let deps = CaptureDependencies {
        source: Box::new(source),
        saver: Box::new(saver),
        clipboard: Box::new(MockClipboard::succeeding()),
    };
    let mut manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_image_delivery(ImageDeliveryRequest {
            image: RenderedImage {
                bytes: vec![1, 2, 3],
                format: ImageFormatMetadata::png(),
                width: 1,
                height: 1,
            },
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig {
                format: "jpg".to_string(),
                ..FileSaveConfig::default()
            }),
            operation: ImageOperationKind::CanvasExport,
            fallback_format_override: Some(ImageFormatMetadata::png()),
        })
        .expect("fixture manager accepts its first image delivery");

    let completion = wait_for_manager_completion(&mut manager)
        .await
        .expect("fixture image delivery keeps the manager worker healthy");
    let result = successful_result(
        completion,
        "successful image-delivery fixture yields a successful capture result",
    );
    assert_eq!(result.operation, ImageOperationKind::CanvasExport);
    assert_eq!(result.image_data, vec![1, 2, 3]);
    assert_eq!(
        result.saved_path,
        Some(PathBuf::from("/tmp/canvas-delivery.png"))
    );
    saved_config_rx
        .try_recv()
        .expect("fixture saver reports the manager-backed image delivery");
    assert!(matches!(
        captured_type_rx.try_recv(),
        Err(TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn request_document_delivery_reports_board_pdf_success() {
    let (source, captured_type_rx) = MockSource::recording(vec![99]);
    let (saver, saved_config_rx) = MockSaver::recording(false, "/tmp/board.pdf");
    let deps = CaptureDependencies {
        source: Box::new(source),
        saver: Box::new(saver),
        clipboard: Box::new(MockClipboard::succeeding()),
    };
    let mut manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_document_delivery(DocumentDeliveryRequest {
            document: rendered_pdf(b"%PDF-".to_vec()),
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig::default()),
            operation: ImageOperationKind::BoardPdfExport,
        })
        .expect("fixture manager accepts its first PDF delivery");

    let completion = wait_for_manager_completion(&mut manager)
        .await
        .expect("fixture PDF delivery keeps the manager worker healthy");
    let result = successful_result(
        completion,
        "successful PDF fixture yields a successful capture result",
    );
    assert_eq!(result.operation, ImageOperationKind::BoardPdfExport);
    assert_eq!(result.image_data, b"%PDF-".to_vec());
    assert_eq!(result.saved_path, Some(PathBuf::from("/tmp/board.pdf")));
    assert!(!result.copied_to_clipboard);
    saved_config_rx
        .try_recv()
        .expect("fixture saver reports the manager-backed PDF delivery");
    assert!(matches!(
        captured_type_rx.try_recv(),
        Err(TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn request_image_delivery_records_canvas_save_failure() {
    let (source, captured_type_rx) = MockSource::recording(vec![99]);
    let (saver, saved_config_rx) = MockSaver::recording(true, "/tmp/canvas-delivery.png");
    let deps = CaptureDependencies {
        source: Box::new(source),
        saver: Box::new(saver),
        clipboard: Box::new(MockClipboard::succeeding()),
    };
    let mut manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_image_delivery(ImageDeliveryRequest {
            image: rendered_png(vec![1, 2, 3]),
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig::default()),
            operation: ImageOperationKind::CanvasExport,
            fallback_format_override: Some(ImageFormatMetadata::png()),
        })
        .expect("fixture manager accepts its failing canvas delivery");

    let completion = wait_for_manager_completion(&mut manager)
        .await
        .expect("fixture canvas failure remains an operation outcome");
    let (status, operation, message) = failed_result(
        completion,
        "failing canvas saver fixture yields a failed canvas outcome",
    );
    assert_eq!(operation, ImageOperationKind::CanvasExport);
    assert!(
        message.contains("Failed to save canvas export"),
        "unexpected failure message: {message}"
    );
    assert!(
        !message.to_lowercase().contains("screenshot"),
        "canvas export failure should not mention screenshot: {message}"
    );
    assert_eq!(status, message);
    saved_config_rx
        .try_recv()
        .expect("fixture saver reports the failed canvas save");
    assert!(matches!(
        captured_type_rx.try_recv(),
        Err(TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn request_document_delivery_records_board_pdf_save_failure() {
    let (source, captured_type_rx) = MockSource::recording(vec![99]);
    let (saver, saved_config_rx) = MockSaver::recording(true, "/tmp/board.pdf");
    let deps = CaptureDependencies {
        source: Box::new(source),
        saver: Box::new(saver),
        clipboard: Box::new(MockClipboard::succeeding()),
    };
    let mut manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_document_delivery(DocumentDeliveryRequest {
            document: rendered_pdf(b"%PDF-".to_vec()),
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig::default()),
            operation: ImageOperationKind::BoardPdfExport,
        })
        .expect("fixture manager accepts its failing PDF delivery");

    let completion = wait_for_manager_completion(&mut manager)
        .await
        .expect("fixture PDF failure remains an operation outcome");
    let (status, operation, message) = failed_result(
        completion,
        "failing PDF saver fixture yields a failed PDF outcome",
    );
    assert_eq!(operation, ImageOperationKind::BoardPdfExport);
    assert!(
        message.contains("Failed to save board PDF export"),
        "unexpected failure message: {message}"
    );
    assert_eq!(status, message);
    saved_config_rx
        .try_recv()
        .expect("fixture saver reports the failed PDF save");
    assert!(matches!(
        captured_type_rx.try_recv(),
        Err(TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn request_image_delivery_preserves_clipboard_success_when_file_fails() {
    let (source, captured_type_rx) = MockSource::recording(vec![99]);
    let (clipboard, copied_image_rx) = MockClipboard::recording(false);
    let deps = CaptureDependencies {
        source: Box::new(source),
        saver: Box::new(MockSaver::failing("/tmp/canvas-delivery.png")),
        clipboard: Box::new(clipboard),
    };
    let mut manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_image_delivery(ImageDeliveryRequest {
            image: rendered_png(vec![1, 2, 3]),
            destination: CaptureDestination::ClipboardAndFile,
            save_config: Some(FileSaveConfig::default()),
            operation: ImageOperationKind::CanvasExport,
            fallback_format_override: Some(ImageFormatMetadata::png()),
        })
        .expect("fixture manager accepts its partial-success image delivery");

    let completion = wait_for_manager_completion(&mut manager)
        .await
        .expect("fixture partial success remains an operation outcome");
    let result = successful_result(
        completion,
        "clipboard-success fixture yields a successful capture result",
    );
    assert_eq!(result.operation, ImageOperationKind::CanvasExport);
    assert!(result.saved_path.is_none());
    assert!(result.copied_to_clipboard);
    assert_eq!(
        copied_image_rx
            .try_recv()
            .expect("fixture clipboard reports the partial-success image copy"),
        vec![1, 2, 3]
    );
    assert!(matches!(
        captured_type_rx.try_recv(),
        Err(TryRecvError::Empty)
    ));
}
