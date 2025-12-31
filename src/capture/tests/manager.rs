use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use tokio::time::{Duration, sleep};

use crate::capture::{
    dependencies::CaptureDependencies,
    file::FileSaveConfig,
    manager::CaptureManager,
    types::{CaptureDestination, CaptureError, CaptureOutcome, CaptureStatus, CaptureType},
};

use super::fixtures::{MockClipboard, MockSaver, MockSource};

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

    // Wait for background thread to finish
    let mut outcome = None;
    for _ in 0..10 {
        if let Some(result) = manager.try_take_result() {
            outcome = Some(result);
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

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

    // wait for failure outcome
    let mut outcome = None;
    for _ in 0..10 {
        if let Some(result) = manager.try_take_result() {
            outcome = Some(result);
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    match outcome {
        Some(CaptureOutcome::Failed(msg)) => {
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
