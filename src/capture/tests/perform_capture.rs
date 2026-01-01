use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::capture::{
    dependencies::CaptureDependencies,
    file::FileSaveConfig,
    pipeline::{CaptureRequest, perform_capture},
    types::{CaptureDestination, CaptureError, CaptureType},
};

use super::fixtures::{MockClipboard, MockSaver, MockSource};

#[tokio::test]
async fn test_perform_capture_clipboard_only_success() {
    let source = MockSource {
        data: vec![1, 2, 3],
        error: Arc::new(Mutex::new(None)),
        captured_types: Arc::new(Mutex::new(Vec::new())),
    };
    let saver = MockSaver {
        should_fail: false,
        path: PathBuf::from("unused.png"),
        calls: Arc::new(Mutex::new(0)),
    };
    let saver_handle = saver.clone();
    let clipboard = MockClipboard {
        should_fail: false,
        calls: Arc::new(Mutex::new(0)),
    };
    let clipboard_handle = clipboard.clone();
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(saver),
        clipboard: Arc::new(clipboard),
    };
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::ClipboardOnly,
        save_config: None,
    };

    let result = perform_capture(request, Arc::new(deps.clone()))
        .await
        .unwrap();
    assert!(result.saved_path.is_none());
    assert!(result.copied_to_clipboard);
    assert_eq!(*clipboard_handle.calls.lock().unwrap(), 1);
    assert_eq!(*saver_handle.calls.lock().unwrap(), 0);
}

#[tokio::test]
async fn test_perform_capture_file_only_success() {
    let source = MockSource {
        data: vec![4, 5, 6],
        error: Arc::new(Mutex::new(None)),
        captured_types: Arc::new(Mutex::new(Vec::new())),
    };
    let saver = MockSaver {
        should_fail: false,
        path: PathBuf::from("/tmp/test.png"),
        calls: Arc::new(Mutex::new(0)),
    };
    let saver_handle = saver.clone();
    let clipboard = MockClipboard {
        should_fail: false,
        calls: Arc::new(Mutex::new(0)),
    };
    let clipboard_handle = clipboard.clone();
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(saver),
        clipboard: Arc::new(clipboard),
    };
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::FileOnly,
        save_config: Some(FileSaveConfig::default()),
    };

    let result = perform_capture(request, Arc::new(deps.clone()))
        .await
        .unwrap();
    assert!(result.saved_path.is_some());
    assert!(!result.copied_to_clipboard);
    assert_eq!(*saver_handle.calls.lock().unwrap(), 1);
    assert_eq!(*clipboard_handle.calls.lock().unwrap(), 0);
}

#[tokio::test]
async fn test_perform_capture_clipboard_failure() {
    let source = MockSource {
        data: vec![7, 8, 9],
        error: Arc::new(Mutex::new(None)),
        captured_types: Arc::new(Mutex::new(Vec::new())),
    };
    let saver = MockSaver {
        should_fail: false,
        path: PathBuf::from("/tmp/a.png"),
        calls: Arc::new(Mutex::new(0)),
    };
    let clipboard = MockClipboard {
        should_fail: true,
        calls: Arc::new(Mutex::new(0)),
    };
    let clipboard_handle = clipboard.clone();
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(saver),
        clipboard: Arc::new(clipboard),
    };
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::ClipboardOnly,
        save_config: None,
    };

    let result = perform_capture(request, Arc::new(deps.clone()))
        .await
        .unwrap();
    assert!(!result.copied_to_clipboard);
    assert_eq!(*clipboard_handle.calls.lock().unwrap(), 1);
}

#[tokio::test]
async fn test_perform_capture_save_failure() {
    let source = MockSource {
        data: vec![10, 11, 12],
        error: Arc::new(Mutex::new(None)),
        captured_types: Arc::new(Mutex::new(Vec::new())),
    };
    let saver = MockSaver {
        should_fail: true,
        path: PathBuf::from("/tmp/should_fail.png"),
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
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::FileOnly,
        save_config: Some(FileSaveConfig::default()),
    };

    let err = perform_capture(request, Arc::new(deps.clone()))
        .await
        .unwrap_err();
    match err {
        CaptureError::SaveError(_) => {}
        other => panic!("expected SaveError, got {:?}", other),
    }
    assert_eq!(*saver_handle.calls.lock().unwrap(), 1);
}

#[tokio::test]
async fn test_perform_capture_clipboard_and_file_success() {
    let source = MockSource {
        data: vec![21, 22, 23],
        error: Arc::new(Mutex::new(None)),
        captured_types: Arc::new(Mutex::new(Vec::new())),
    };
    let saver = MockSaver {
        should_fail: false,
        path: PathBuf::from("/tmp/combined.png"),
        calls: Arc::new(Mutex::new(0)),
    };
    let clipboard = MockClipboard {
        should_fail: false,
        calls: Arc::new(Mutex::new(0)),
    };
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(saver.clone()),
        clipboard: Arc::new(clipboard.clone()),
    };
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::ClipboardAndFile,
        save_config: Some(FileSaveConfig::default()),
    };

    let result = perform_capture(request, Arc::new(deps)).await.unwrap();
    assert!(result.saved_path.is_some());
    assert!(result.copied_to_clipboard);
    assert_eq!(*saver.calls.lock().unwrap(), 1);
    assert_eq!(*clipboard.calls.lock().unwrap(), 1);
}

#[tokio::test]
async fn test_perform_capture_clipboard_and_file_save_failure_still_copies() {
    let source = MockSource {
        data: vec![21, 22, 23],
        error: Arc::new(Mutex::new(None)),
        captured_types: Arc::new(Mutex::new(Vec::new())),
    };
    let saver = MockSaver {
        should_fail: true,
        path: PathBuf::from("/tmp/combined_fail.png"),
        calls: Arc::new(Mutex::new(0)),
    };
    let saver_handle = saver.clone();
    let clipboard = MockClipboard {
        should_fail: false,
        calls: Arc::new(Mutex::new(0)),
    };
    let clipboard_handle = clipboard.clone();
    let deps = CaptureDependencies {
        source: Arc::new(source),
        saver: Arc::new(saver),
        clipboard: Arc::new(clipboard),
    };
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::ClipboardAndFile,
        save_config: Some(FileSaveConfig::default()),
    };

    let result = perform_capture(request, Arc::new(deps)).await.unwrap();
    assert!(result.saved_path.is_none());
    assert!(result.copied_to_clipboard);
    assert_eq!(*saver_handle.calls.lock().unwrap(), 1);
    assert_eq!(*clipboard_handle.calls.lock().unwrap(), 1);
}

#[tokio::test]
async fn perform_capture_propagates_source_error() {
    let source = MockSource {
        data: vec![],
        error: Arc::new(Mutex::new(Some(CaptureError::ImageError(
            "boom".to_string(),
        )))),
        captured_types: Arc::new(Mutex::new(Vec::new())),
    };
    let saver = MockSaver {
        should_fail: false,
        path: PathBuf::from("/tmp/unneeded.png"),
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
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::ClipboardOnly,
        save_config: None,
    };

    let err = perform_capture(request, Arc::new(deps)).await.unwrap_err();
    match err {
        CaptureError::ImageError(msg) => assert!(
            msg.contains("boom"),
            "expected error message to contain 'boom', got: {msg}"
        ),
        other => panic!("expected ImageError, got {other:?}"),
    }
}
