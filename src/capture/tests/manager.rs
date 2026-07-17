use std::{
    os::fd::AsRawFd,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use tokio::time::{Duration, sleep};

use crate::capture::{
    DesktopBackdropCaptureRequest, DocumentDeliveryRequest, ImageDeliveryRequest,
    ImageFormatMetadata, ImageOperationKind, RenderedDocument, RenderedImage,
    dependencies::{CaptureDependencies, CaptureFuture, CaptureSource},
    file::FileSaveConfig,
    manager::{CaptureManager, CapturePoll, CaptureSubmitError},
    types::{CaptureDestination, CaptureError, CaptureOutcome, CaptureStatus, CaptureType},
};

use super::fixtures::{MockClipboard, MockSaver, MockSource, create_placeholder_image};

struct PanicSource;

struct GatedSource {
    started: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

impl CaptureSource for PanicSource {
    fn capture(&self, _capture_type: CaptureType) -> CaptureFuture<'_> {
        Box::pin(async { panic!("expected capture worker panic") })
    }
}

impl CaptureSource for GatedSource {
    fn capture(&self, _capture_type: CaptureType) -> CaptureFuture<'_> {
        Box::pin(async move {
            self.started.notify_one();
            self.release.notified().await;
            Ok(vec![1, 2, 3])
        })
    }
}

async fn wait_for_notification(notifications: &AtomicUsize) {
    for _ in 0..100 {
        if notifications.load(Ordering::Acquire) > 0 {
            return;
        }
        sleep(Duration::from_millis(10)).await;
    }
    panic!("capture worker did not publish a completion notification");
}

async fn wait_for_manager_outcome(manager: &mut CaptureManager) -> Option<CaptureOutcome> {
    for _ in 0..100 {
        match manager.poll() {
            CapturePoll::Ready { outcome, .. } => return Some(outcome),
            CapturePoll::WorkerFailed { error, .. } => {
                panic!("capture worker failed while awaiting outcome: {error}")
            }
            CapturePoll::Idle | CapturePoll::Pending { .. } => {}
        }
        sleep(Duration::from_millis(20)).await;
    }
    None
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
async fn test_capture_manager_creation() {
    let manager = CaptureManager::new(&tokio::runtime::Handle::current());
    let status = manager.get_status().await;
    assert_eq!(status, CaptureStatus::Idle);
}

#[tokio::test]
async fn completion_is_observable_when_the_notifier_runs() {
    let deps = CaptureDependencies {
        source: Arc::new(MockSource {
            data: vec![1, 2, 3],
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        }),
        saver: Arc::new(MockSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/notified.png"),
            calls: Arc::new(Mutex::new(0)),
        }),
        clipboard: Arc::new(MockClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
        }),
    };
    let notifications = Arc::new(AtomicUsize::new(0));
    let notified = Arc::clone(&notifications);
    let mut manager = CaptureManager::with_dependencies_and_test_notifier(
        &tokio::runtime::Handle::current(),
        deps,
        move || {
            notified.fetch_add(1, Ordering::Release);
        },
    );
    let accepted = manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .unwrap();

    wait_for_notification(&notifications).await;

    assert!(matches!(
        manager.poll(),
        CapturePoll::Ready { id, .. } if id == accepted
    ));
    assert_eq!(notifications.load(Ordering::Acquire), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn completion_wakes_an_already_blocked_runtime_poll_after_publication() {
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let deps = CaptureDependencies {
        source: Arc::new(GatedSource {
            started: Arc::clone(&started),
            release: Arc::clone(&release),
        }),
        saver: Arc::new(MockSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/gated.png"),
            calls: Arc::new(Mutex::new(0)),
        }),
        clipboard: Arc::new(MockClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
        }),
    };
    let runtime_wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
    let wake_handle = runtime_wake.handle();
    let mut manager = CaptureManager::with_dependencies_and_test_notifier(
        &tokio::runtime::Handle::current(),
        deps,
        move || {
            wake_handle.wake().unwrap();
        },
    );
    let accepted = manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .unwrap();
    started.notified().await;

    let wake_fd = runtime_wake.poll_fd().as_raw_fd();
    let (polling_tx, polling_rx) = std::sync::mpsc::sync_channel(0);
    let poller = std::thread::spawn(move || {
        let mut pollfd = libc::pollfd {
            fd: wake_fd,
            events: libc::POLLIN,
            revents: 0,
        };
        polling_tx.send(()).unwrap();
        // SAFETY: the runtime wake source outlives this bounded poll and owns
        // the descriptor for the entire test.
        assert_eq!(unsafe { libc::poll(&mut pollfd, 1, 1_000) }, 1);
        assert_ne!(pollfd.revents & libc::POLLIN, 0);
    });
    polling_rx.recv().unwrap();
    release.notify_one();
    poller.join().unwrap();
    runtime_wake.drain().unwrap();

    assert!(matches!(
        manager.poll(),
        CapturePoll::Ready { id, .. } if id == accepted
    ));
}

#[tokio::test]
async fn worker_panic_wakes_and_reports_the_active_operation_once() {
    let deps = CaptureDependencies {
        source: Arc::new(PanicSource),
        saver: Arc::new(MockSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/unreachable.png"),
            calls: Arc::new(Mutex::new(0)),
        }),
        clipboard: Arc::new(MockClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
        }),
    };
    let notifications = Arc::new(AtomicUsize::new(0));
    let notified = Arc::clone(&notifications);
    let mut manager = CaptureManager::with_dependencies_and_test_notifier(
        &tokio::runtime::Handle::current(),
        deps,
        move || {
            notified.fetch_add(1, Ordering::Release);
        },
    );
    let accepted = manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .unwrap();

    wait_for_notification(&notifications).await;

    assert!(matches!(
        manager.poll(),
        CapturePoll::WorkerFailed {
            active_id: Some(id),
            operation: Some(ImageOperationKind::Screenshot),
            ..
        } if id == accepted
    ));
    assert!(matches!(manager.poll(), CapturePoll::Idle));
    assert_eq!(notifications.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn normal_shutdown_does_not_publish_a_failure_notification() {
    let notifications = Arc::new(AtomicUsize::new(0));
    let notified = Arc::clone(&notifications);
    let mut manager = CaptureManager::with_dependencies_and_test_notifier(
        &tokio::runtime::Handle::current(),
        CaptureDependencies::default(),
        move || {
            notified.fetch_add(1, Ordering::Release);
        },
    );

    manager.shutdown();
    tokio::task::yield_now().await;

    assert_eq!(notifications.load(Ordering::Acquire), 0);
    assert!(matches!(manager.poll(), CapturePoll::Idle));
}

#[tokio::test]
async fn test_capture_manager_with_dependencies() {
    let clipboard_calls = Arc::new(Mutex::new(0));
    let source = MockSource {
        data: vec![13, 14, 15],
        error: Arc::new(Mutex::new(None)),
        captured_types: Arc::new(Mutex::new(Vec::new())),
    };
    let saver = MockSaver {
        should_fail: false,
        path: PathBuf::from("/tmp/manager.png"),
        calls: Arc::new(Mutex::new(0)),
    };
    let clipboard = MockClipboard {
        should_fail: false,
        calls: clipboard_calls.clone(),
    };
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(saver),
        clipboard: Arc::new(clipboard),
    };
    let mut manager =
        CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps.clone());

    manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .unwrap();

    let outcome = wait_for_manager_outcome(&mut manager).await;

    match outcome {
        Some(CaptureOutcome::Success(result)) => {
            assert!(result.saved_path.is_none());
            assert!(result.copied_to_clipboard);
        }
        other => panic!("Expected success outcome, got {:?}", other),
    }
    assert_eq!(*clipboard_calls.lock().unwrap(), 1);
    assert_eq!(manager.get_status().await, CaptureStatus::Success);
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
        .expect_err("should fail when channel closed");
    assert!(
        matches!(err, CaptureSubmitError::Disconnected),
        "unexpected error variant: {err:?}"
    );
}

#[tokio::test]
async fn capture_manager_records_failure_status() {
    let source = MockSource {
        data: vec![99],
        error: Arc::new(Mutex::new(None)),
        captured_types: Arc::new(Mutex::new(Vec::new())),
    };
    let saver = MockSaver {
        should_fail: true,
        path: PathBuf::from("/tmp/fail.png"),
        calls: Arc::new(Mutex::new(0)),
    };
    let clipboard = MockClipboard {
        should_fail: false,
        calls: Arc::new(Mutex::new(0)),
    };
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(saver),
        clipboard: Arc::new(clipboard),
    };
    let mut manager =
        CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps.clone());

    manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::FileOnly,
            Some(FileSaveConfig::default()),
        )
        .unwrap();

    let outcome = wait_for_manager_outcome(&mut manager).await;

    match outcome {
        Some(CaptureOutcome::Failed { message: msg, .. }) => {
            assert!(
                msg.contains("save failed"),
                "unexpected failure message: {msg}"
            );
        }
        other => panic!("Expected failure outcome, got {other:?}"),
    }

    assert!(matches!(
        manager.get_status().await,
        CaptureStatus::Failed(_)
    ));
}

#[tokio::test]
async fn capture_manager_preserves_user_cancellation_as_a_terminal_outcome() {
    let source = MockSource {
        data: Vec::new(),
        error: Arc::new(Mutex::new(Some(CaptureError::Cancelled(
            "user dismissed portal".to_string(),
        )))),
        captured_types: Arc::new(Mutex::new(Vec::new())),
    };
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(MockSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/cancelled.png"),
            calls: Arc::new(Mutex::new(0)),
        }),
        clipboard: Arc::new(MockClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
        }),
    };
    let mut manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);
    manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .unwrap();

    assert!(matches!(
        wait_for_manager_outcome(&mut manager).await,
        Some(CaptureOutcome::Cancelled {
            operation: ImageOperationKind::Screenshot,
            reason,
        }) if reason == "user dismissed portal"
    ));
    assert!(matches!(
        manager.get_status().await,
        CaptureStatus::Cancelled(reason) if reason == "user dismissed portal"
    ));
}

#[tokio::test]
async fn desktop_backdrop_completion_releases_the_manager_for_pdf_delivery() {
    let source = MockSource {
        data: create_placeholder_image(),
        error: Arc::new(Mutex::new(None)),
        captured_types: Arc::new(Mutex::new(Vec::new())),
    };
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(MockSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/after-backdrop.pdf"),
            calls: Arc::new(Mutex::new(0)),
        }),
        clipboard: Arc::new(MockClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
        }),
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
        .unwrap();

    assert!(matches!(
        wait_for_manager_outcome(&mut manager).await,
        Some(CaptureOutcome::DesktopBackdropSuccess(backdrop))
            if backdrop.width == 100 && backdrop.height == 100
    ));

    let document_id = manager
        .request_document_delivery(DocumentDeliveryRequest {
            document: rendered_pdf(b"%PDF-".to_vec()),
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig::default()),
            operation: ImageOperationKind::BoardPdfExport,
        })
        .unwrap();
    assert!(document_id > backdrop_id);
    assert!(matches!(
        wait_for_manager_outcome(&mut manager).await,
        Some(CaptureOutcome::Success(result))
            if result.operation == ImageOperationKind::BoardPdfExport
    ));
}

#[tokio::test]
async fn request_image_delivery_queues_manager_backed_path() {
    let captured_types = Arc::new(Mutex::new(Vec::new()));
    let source = MockSource {
        data: vec![99],
        error: Arc::new(Mutex::new(None)),
        captured_types: captured_types.clone(),
    };
    let saver = MockSaver {
        should_fail: false,
        path: PathBuf::from("/tmp/canvas-delivery.png"),
        calls: Arc::new(Mutex::new(0)),
    };
    let saver_handle = saver.clone();
    let clipboard = MockClipboard {
        should_fail: false,
        calls: Arc::new(Mutex::new(0)),
    };
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(saver),
        clipboard: Arc::new(clipboard),
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
        .unwrap();

    let outcome = wait_for_manager_outcome(&mut manager).await;

    match outcome {
        Some(CaptureOutcome::Success(result)) => {
            assert_eq!(result.operation, ImageOperationKind::CanvasExport);
            assert_eq!(result.image_data, vec![1, 2, 3]);
            assert_eq!(
                result.saved_path,
                Some(PathBuf::from("/tmp/canvas-delivery.png"))
            );
        }
        other => panic!("Expected success outcome, got {other:?}"),
    }
    assert_eq!(*saver_handle.calls.lock().unwrap(), 1);
    assert!(captured_types.lock().unwrap().is_empty());
    assert_eq!(manager.get_status().await, CaptureStatus::Success);
}

#[tokio::test]
async fn request_document_delivery_reports_board_pdf_success() {
    let captured_types = Arc::new(Mutex::new(Vec::new()));
    let source = MockSource {
        data: vec![99],
        error: Arc::new(Mutex::new(None)),
        captured_types: captured_types.clone(),
    };
    let saver = MockSaver {
        should_fail: false,
        path: PathBuf::from("/tmp/board.pdf"),
        calls: Arc::new(Mutex::new(0)),
    };
    let saver_handle = saver.clone();
    let clipboard = MockClipboard {
        should_fail: false,
        calls: Arc::new(Mutex::new(0)),
    };
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(saver),
        clipboard: Arc::new(clipboard),
    };
    let mut manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_document_delivery(DocumentDeliveryRequest {
            document: rendered_pdf(b"%PDF-".to_vec()),
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig::default()),
            operation: ImageOperationKind::BoardPdfExport,
        })
        .unwrap();

    let outcome = wait_for_manager_outcome(&mut manager).await;

    match outcome {
        Some(CaptureOutcome::Success(result)) => {
            assert_eq!(result.operation, ImageOperationKind::BoardPdfExport);
            assert_eq!(result.image_data, b"%PDF-".to_vec());
            assert_eq!(result.saved_path, Some(PathBuf::from("/tmp/board.pdf")));
            assert!(!result.copied_to_clipboard);
        }
        other => panic!("Expected success outcome, got {other:?}"),
    }
    assert_eq!(*saver_handle.calls.lock().unwrap(), 1);
    assert!(captured_types.lock().unwrap().is_empty());
    assert_eq!(manager.get_status().await, CaptureStatus::Success);
}

#[tokio::test]
async fn request_image_delivery_records_canvas_save_failure() {
    let captured_types = Arc::new(Mutex::new(Vec::new()));
    let source = MockSource {
        data: vec![99],
        error: Arc::new(Mutex::new(None)),
        captured_types: captured_types.clone(),
    };
    let saver = MockSaver {
        should_fail: true,
        path: PathBuf::from("/tmp/canvas-delivery.png"),
        calls: Arc::new(Mutex::new(0)),
    };
    let saver_handle = saver.clone();
    let clipboard = MockClipboard {
        should_fail: false,
        calls: Arc::new(Mutex::new(0)),
    };
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(saver),
        clipboard: Arc::new(clipboard),
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
        .unwrap();

    let outcome = wait_for_manager_outcome(&mut manager).await;

    match outcome {
        Some(CaptureOutcome::Failed { operation, message }) => {
            assert_eq!(operation, ImageOperationKind::CanvasExport);
            assert!(
                message.contains("Failed to save canvas export"),
                "unexpected failure message: {message}"
            );
            assert!(
                !message.to_lowercase().contains("screenshot"),
                "canvas export failure should not mention screenshot: {message}"
            );
        }
        other => panic!("Expected failure outcome, got {other:?}"),
    }
    assert_eq!(*saver_handle.calls.lock().unwrap(), 1);
    assert!(captured_types.lock().unwrap().is_empty());
    assert!(matches!(
        manager.get_status().await,
        CaptureStatus::Failed(ref message)
            if message.contains("Failed to save canvas export")
                && !message.to_lowercase().contains("screenshot")
    ));
}

#[tokio::test]
async fn request_document_delivery_records_board_pdf_save_failure() {
    let captured_types = Arc::new(Mutex::new(Vec::new()));
    let source = MockSource {
        data: vec![99],
        error: Arc::new(Mutex::new(None)),
        captured_types: captured_types.clone(),
    };
    let saver = MockSaver {
        should_fail: true,
        path: PathBuf::from("/tmp/board.pdf"),
        calls: Arc::new(Mutex::new(0)),
    };
    let saver_handle = saver.clone();
    let clipboard = MockClipboard {
        should_fail: false,
        calls: Arc::new(Mutex::new(0)),
    };
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(saver),
        clipboard: Arc::new(clipboard),
    };
    let mut manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_document_delivery(DocumentDeliveryRequest {
            document: rendered_pdf(b"%PDF-".to_vec()),
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig::default()),
            operation: ImageOperationKind::BoardPdfExport,
        })
        .unwrap();

    let outcome = wait_for_manager_outcome(&mut manager).await;

    match outcome {
        Some(CaptureOutcome::Failed { operation, message }) => {
            assert_eq!(operation, ImageOperationKind::BoardPdfExport);
            assert!(
                message.contains("Failed to save board PDF export"),
                "unexpected failure message: {message}"
            );
        }
        other => panic!("Expected failure outcome, got {other:?}"),
    }
    assert_eq!(*saver_handle.calls.lock().unwrap(), 1);
    assert!(captured_types.lock().unwrap().is_empty());
    assert!(matches!(
        manager.get_status().await,
        CaptureStatus::Failed(ref message)
            if message.contains("Failed to save board PDF export")
    ));
}

#[tokio::test]
async fn request_image_delivery_preserves_clipboard_success_when_file_fails() {
    let captured_types = Arc::new(Mutex::new(Vec::new()));
    let source = MockSource {
        data: vec![99],
        error: Arc::new(Mutex::new(None)),
        captured_types: captured_types.clone(),
    };
    let saver = MockSaver {
        should_fail: true,
        path: PathBuf::from("/tmp/canvas-delivery.png"),
        calls: Arc::new(Mutex::new(0)),
    };
    let clipboard_calls = Arc::new(Mutex::new(0));
    let clipboard = MockClipboard {
        should_fail: false,
        calls: clipboard_calls.clone(),
    };
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(saver),
        clipboard: Arc::new(clipboard),
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
        .unwrap();

    let outcome = wait_for_manager_outcome(&mut manager).await;

    match outcome {
        Some(CaptureOutcome::Success(result)) => {
            assert_eq!(result.operation, ImageOperationKind::CanvasExport);
            assert!(result.saved_path.is_none());
            assert!(result.copied_to_clipboard);
        }
        other => panic!("Expected success outcome, got {other:?}"),
    }
    assert_eq!(*clipboard_calls.lock().unwrap(), 1);
    assert!(captured_types.lock().unwrap().is_empty());
    assert_eq!(manager.get_status().await, CaptureStatus::Success);
}
