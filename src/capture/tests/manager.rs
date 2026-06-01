use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use tokio::time::{Duration, sleep};

use crate::capture::{
    DocumentDeliveryRequest, ImageDeliveryRequest, ImageFormatMetadata, ImageOperationKind,
    RenderedDocument, RenderedImage,
    dependencies::CaptureDependencies,
    file::FileSaveConfig,
    manager::CaptureManager,
    types::{CaptureDestination, CaptureError, CaptureOutcome, CaptureStatus, CaptureType},
};

use super::fixtures::{MockClipboard, MockSaver, MockSource};

async fn wait_for_manager_outcome(manager: &CaptureManager) -> Option<CaptureOutcome> {
    for _ in 0..10 {
        if let Some(result) = manager.try_take_result() {
            return Some(result);
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
    let manager =
        CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps.clone());

    manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .unwrap();

    let outcome = wait_for_manager_outcome(&manager).await;

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
    let manager = CaptureManager::with_closed_channel_for_test();
    let err = manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::ClipboardOnly,
            None,
        )
        .expect_err("should fail when channel closed");
    assert!(
        matches!(err, CaptureError::ImageError(ref msg) if msg.contains("not running")),
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
    let manager =
        CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps.clone());

    manager
        .request_capture(
            CaptureType::FullScreen,
            CaptureDestination::FileOnly,
            Some(FileSaveConfig::default()),
        )
        .unwrap();

    let outcome = wait_for_manager_outcome(&manager).await;

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
    let manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

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

    let outcome = wait_for_manager_outcome(&manager).await;

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
    let manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_document_delivery(DocumentDeliveryRequest {
            document: rendered_pdf(b"%PDF-".to_vec()),
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig::default()),
            operation: ImageOperationKind::BoardPdfExport,
        })
        .unwrap();

    let outcome = wait_for_manager_outcome(&manager).await;

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
    let manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_image_delivery(ImageDeliveryRequest {
            image: rendered_png(vec![1, 2, 3]),
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig::default()),
            operation: ImageOperationKind::CanvasExport,
            fallback_format_override: Some(ImageFormatMetadata::png()),
        })
        .unwrap();

    let outcome = wait_for_manager_outcome(&manager).await;

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
    let manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_document_delivery(DocumentDeliveryRequest {
            document: rendered_pdf(b"%PDF-".to_vec()),
            destination: CaptureDestination::FileOnly,
            save_config: Some(FileSaveConfig::default()),
            operation: ImageOperationKind::BoardPdfExport,
        })
        .unwrap();

    let outcome = wait_for_manager_outcome(&manager).await;

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
    let manager = CaptureManager::with_dependencies(&tokio::runtime::Handle::current(), deps);

    manager
        .request_image_delivery(ImageDeliveryRequest {
            image: rendered_png(vec![1, 2, 3]),
            destination: CaptureDestination::ClipboardAndFile,
            save_config: Some(FileSaveConfig::default()),
            operation: ImageOperationKind::CanvasExport,
            fallback_format_override: Some(ImageFormatMetadata::png()),
        })
        .unwrap();

    let outcome = wait_for_manager_outcome(&manager).await;

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
