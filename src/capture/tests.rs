use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use tokio::time::{Duration, sleep};

use super::{
    dependencies::{CaptureClipboard, CaptureDependencies, CaptureFileSaver, CaptureSource},
    file::FileSaveConfig,
    manager::CaptureManager,
    pipeline::{CaptureRequest, perform_capture},
    types::{CaptureDestination, CaptureError, CaptureOutcome, CaptureStatus, CaptureType},
};

#[derive(Clone)]
struct MockSource {
    data: Vec<u8>,
    error: Arc<Mutex<Option<CaptureError>>>,
    captured_types: Arc<Mutex<Vec<CaptureType>>>,
}

#[async_trait]
impl CaptureSource for MockSource {
    async fn capture(&self, capture_type: CaptureType) -> Result<Vec<u8>, CaptureError> {
        self.captured_types.lock().unwrap().push(capture_type);
        if let Some(err) = self.error.lock().unwrap().take() {
            Err(err)
        } else {
            Ok(self.data.clone())
        }
    }
}

#[derive(Clone)]
struct MockSaver {
    pub should_fail: bool,
    pub path: PathBuf,
    pub calls: Arc<Mutex<usize>>,
}

impl CaptureFileSaver for MockSaver {
    fn save(&self, _image_data: &[u8], _config: &FileSaveConfig) -> Result<PathBuf, CaptureError> {
        *self.calls.lock().unwrap() += 1;
        if self.should_fail {
            Err(CaptureError::SaveError(std::io::Error::new(
                std::io::ErrorKind::Other,
                "save failed",
            )))
        } else {
            Ok(self.path.clone())
        }
    }
}

#[derive(Clone)]
struct MockClipboard {
    pub should_fail: bool,
    pub calls: Arc<Mutex<usize>>,
}

impl CaptureClipboard for MockClipboard {
    fn copy(&self, _image_data: &[u8]) -> Result<(), CaptureError> {
        *self.calls.lock().unwrap() += 1;
        if self.should_fail {
            Err(CaptureError::ClipboardError(
                "clipboard failure".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

fn create_placeholder_image() -> Vec<u8> {
    use cairo::{Context, FontSlant, FontWeight, Format, ImageSurface};

    let surface = ImageSurface::create(Format::ARgb32, 100, 100).unwrap();
    let ctx = Context::new(&surface).unwrap();

    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.paint().unwrap();

    ctx.set_source_rgb(1.0, 1.0, 1.0);
    ctx.select_font_face("Sans", FontSlant::Normal, FontWeight::Bold);
    ctx.set_font_size(20.0);
    ctx.move_to(10.0, 50.0);
    ctx.show_text("TEST").unwrap();

    let mut buffer = Vec::new();
    surface.write_to_png(&mut buffer).unwrap();
    buffer
}

#[test]
fn test_create_placeholder_image() {
    let image = create_placeholder_image();
    assert!(!image.is_empty());
    // PNG signature
    assert_eq!(&image[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
}

#[tokio::test]
async fn test_capture_manager_creation() {
    let manager = CaptureManager::new(&tokio::runtime::Handle::current());
    let status = manager.get_status().await;
    assert_eq!(status, CaptureStatus::Idle);
}

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
