use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::capture::{
    dependencies::{CaptureClipboard, CaptureDependencies, CaptureFileSaver},
    file::FileSaveConfig,
    pipeline::{CaptureRequest, deliver_image, perform_capture},
    types::{
        CaptureDestination, CaptureError, CaptureType, ImageDeliveryRequest, ImageFormatMetadata,
        ImageOperationKind, RenderedImage,
    },
};

use super::fixtures::{MockClipboard, MockSaver, MockSource};

#[derive(Clone)]
struct RecordingSaver {
    should_fail: bool,
    path: PathBuf,
    calls: Arc<Mutex<usize>>,
    configs: Arc<Mutex<Vec<FileSaveConfig>>>,
}

impl CaptureFileSaver for RecordingSaver {
    fn save(&self, _image_data: &[u8], config: &FileSaveConfig) -> Result<PathBuf, CaptureError> {
        *self.calls.lock().unwrap() += 1;
        self.configs.lock().unwrap().push(config.clone());
        if self.should_fail {
            Err(CaptureError::SaveError(std::io::Error::other(
                "save failed",
            )))
        } else {
            Ok(self.path.clone())
        }
    }
}

#[derive(Clone)]
struct RecordingClipboard {
    should_fail: bool,
    calls: Arc<Mutex<usize>>,
    copied: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl CaptureClipboard for RecordingClipboard {
    fn copy(&self, image_data: &[u8]) -> Result<(), CaptureError> {
        *self.calls.lock().unwrap() += 1;
        self.copied.lock().unwrap().push(image_data.to_vec());
        if self.should_fail {
            Err(CaptureError::ClipboardError(
                "clipboard failure".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

fn rendered_png(bytes: Vec<u8>) -> RenderedImage {
    RenderedImage {
        bytes,
        format: ImageFormatMetadata::png(),
        width: 2,
        height: 1,
    }
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
    assert_eq!(result.operation, ImageOperationKind::Screenshot);
    assert!(result.fallback_format_override.is_none());
    assert!(result.saved_path.is_none());
    assert!(result.copied_to_clipboard);
    assert_eq!(*clipboard_handle.calls.lock().unwrap(), 1);
    assert_eq!(*saver_handle.calls.lock().unwrap(), 0);
}

#[tokio::test]
async fn deliver_image_file_only_saves_rendered_format_extension() {
    let configs = Arc::new(Mutex::new(Vec::new()));
    let saver = RecordingSaver {
        should_fail: false,
        path: PathBuf::from("/tmp/canvas.png"),
        calls: Arc::new(Mutex::new(0)),
        configs: configs.clone(),
    };
    let clipboard = RecordingClipboard {
        should_fail: false,
        calls: Arc::new(Mutex::new(0)),
        copied: Arc::new(Mutex::new(Vec::new())),
    };
    let deps = CaptureDependencies {
        source: Arc::new(MockSource {
            data: Vec::new(),
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        }),
        saver: Arc::new(saver.clone()),
        clipboard: Arc::new(clipboard),
    };
    let request = ImageDeliveryRequest {
        image: rendered_png(vec![137, 80, 78, 71]),
        destination: CaptureDestination::FileOnly,
        save_config: Some(FileSaveConfig {
            format: "jpg".to_string(),
            ..FileSaveConfig::default()
        }),
        operation: ImageOperationKind::CanvasExport,
        fallback_format_override: Some(ImageFormatMetadata::png()),
    };

    let result = deliver_image(request, Arc::new(deps)).await.unwrap();

    assert_eq!(result.operation, ImageOperationKind::CanvasExport);
    assert_eq!(
        result.fallback_format_override,
        Some(ImageFormatMetadata::png())
    );
    assert_eq!(*saver.calls.lock().unwrap(), 1);
    assert_eq!(configs.lock().unwrap()[0].format, "png");
    assert_eq!(result.saved_path, Some(PathBuf::from("/tmp/canvas.png")));
}

#[tokio::test]
async fn deliver_image_clipboard_only_copies_png_bytes() {
    let copied = Arc::new(Mutex::new(Vec::new()));
    let clipboard = RecordingClipboard {
        should_fail: false,
        calls: Arc::new(Mutex::new(0)),
        copied: copied.clone(),
    };
    let deps = CaptureDependencies {
        source: Arc::new(MockSource {
            data: Vec::new(),
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        }),
        saver: Arc::new(RecordingSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/unused.png"),
            calls: Arc::new(Mutex::new(0)),
            configs: Arc::new(Mutex::new(Vec::new())),
        }),
        clipboard: Arc::new(clipboard.clone()),
    };
    let bytes = vec![1, 2, 3, 4];
    let request = ImageDeliveryRequest {
        image: rendered_png(bytes.clone()),
        destination: CaptureDestination::ClipboardOnly,
        save_config: None,
        operation: ImageOperationKind::CanvasExport,
        fallback_format_override: Some(ImageFormatMetadata::png()),
    };

    let result = deliver_image(request, Arc::new(deps)).await.unwrap();

    assert!(result.copied_to_clipboard);
    assert_eq!(*clipboard.calls.lock().unwrap(), 1);
    assert_eq!(copied.lock().unwrap()[0], bytes);
}

#[tokio::test]
async fn deliver_image_clipboard_and_file_keeps_file_success_when_clipboard_fails() {
    let saver = RecordingSaver {
        should_fail: false,
        path: PathBuf::from("/tmp/partial.png"),
        calls: Arc::new(Mutex::new(0)),
        configs: Arc::new(Mutex::new(Vec::new())),
    };
    let deps = CaptureDependencies {
        source: Arc::new(MockSource {
            data: Vec::new(),
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        }),
        saver: Arc::new(saver),
        clipboard: Arc::new(RecordingClipboard {
            should_fail: true,
            calls: Arc::new(Mutex::new(0)),
            copied: Arc::new(Mutex::new(Vec::new())),
        }),
    };
    let request = ImageDeliveryRequest {
        image: rendered_png(vec![1, 2, 3]),
        destination: CaptureDestination::ClipboardAndFile,
        save_config: Some(FileSaveConfig::default()),
        operation: ImageOperationKind::CanvasExport,
        fallback_format_override: Some(ImageFormatMetadata::png()),
    };

    let result = deliver_image(request, Arc::new(deps)).await.unwrap();

    assert_eq!(result.saved_path, Some(PathBuf::from("/tmp/partial.png")));
    assert!(!result.copied_to_clipboard);
}

#[tokio::test]
async fn deliver_image_clipboard_and_file_keeps_clipboard_success_when_file_fails() {
    let clipboard = RecordingClipboard {
        should_fail: false,
        calls: Arc::new(Mutex::new(0)),
        copied: Arc::new(Mutex::new(Vec::new())),
    };
    let deps = CaptureDependencies {
        source: Arc::new(MockSource {
            data: Vec::new(),
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        }),
        saver: Arc::new(RecordingSaver {
            should_fail: true,
            path: PathBuf::from("/tmp/partial.png"),
            calls: Arc::new(Mutex::new(0)),
            configs: Arc::new(Mutex::new(Vec::new())),
        }),
        clipboard: Arc::new(clipboard.clone()),
    };
    let request = ImageDeliveryRequest {
        image: rendered_png(vec![1, 2, 3]),
        destination: CaptureDestination::ClipboardAndFile,
        save_config: Some(FileSaveConfig::default()),
        operation: ImageOperationKind::CanvasExport,
        fallback_format_override: Some(ImageFormatMetadata::png()),
    };

    let result = deliver_image(request, Arc::new(deps)).await.unwrap();

    assert!(result.saved_path.is_none());
    assert!(result.copied_to_clipboard);
    assert_eq!(*clipboard.calls.lock().unwrap(), 1);
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
