use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::canvas_export::{
    BoardExportSnapshot, CanvasExportBackdropSnapshot, CanvasExportSnapshot, CanvasExportViewport,
    SpotlightPassSnapshot,
};
use crate::capture::{
    dependencies::{CaptureClipboard, CaptureDependencies, CaptureFileSaver},
    file::FileSaveConfig,
    pipeline::{CaptureRequest, deliver_document, deliver_image, perform_capture},
    types::{
        CaptureDestination, CaptureError, CaptureType, DocumentAttachment, DocumentDeliveryRequest,
        ImageDeliveryRequest, ImageFormatMetadata, ImageOperationKind, RenderedDocument,
        RenderedImage,
    },
};
use crate::draw::Frame;

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

fn rendered_pdf(bytes: Vec<u8>) -> RenderedDocument {
    RenderedDocument {
        bytes,
        extension: "pdf".to_string(),
        mime_type: "application/pdf".to_string(),
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
async fn deliver_document_file_only_saves_pdf_bytes_with_pdf_extension() {
    let configs = Arc::new(Mutex::new(Vec::new()));
    let saver = RecordingSaver {
        should_fail: false,
        path: PathBuf::from("/tmp/board.pdf"),
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
        clipboard: Arc::new(clipboard.clone()),
    };
    let request = DocumentDeliveryRequest {
        attachments: Vec::new(),
        document: rendered_pdf(b"%PDF-".to_vec()),
        destination: CaptureDestination::FileOnly,
        save_config: Some(FileSaveConfig {
            format: "png".to_string(),
            ..FileSaveConfig::default()
        }),
        operation: ImageOperationKind::BoardPdfExport,
    };

    let result = deliver_document(request, Arc::new(deps)).await.unwrap();

    assert_eq!(result.operation, ImageOperationKind::BoardPdfExport);
    assert_eq!(result.image_data, b"%PDF-".to_vec());
    assert_eq!(result.saved_path, Some(PathBuf::from("/tmp/board.pdf")));
    assert!(!result.copied_to_clipboard);
    assert_eq!(*saver.calls.lock().unwrap(), 1);
    assert_eq!(configs.lock().unwrap()[0].format, "pdf");
    assert_eq!(*clipboard.calls.lock().unwrap(), 0);
}

#[tokio::test]
async fn deliver_document_rejects_clipboard_destinations() {
    let deps = CaptureDependencies {
        source: Arc::new(MockSource {
            data: Vec::new(),
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        }),
        saver: Arc::new(RecordingSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/unused.pdf"),
            calls: Arc::new(Mutex::new(0)),
            configs: Arc::new(Mutex::new(Vec::new())),
        }),
        clipboard: Arc::new(RecordingClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
            copied: Arc::new(Mutex::new(Vec::new())),
        }),
    };
    let request = DocumentDeliveryRequest {
        attachments: Vec::new(),
        document: rendered_pdf(Vec::new()),
        destination: CaptureDestination::ClipboardOnly,
        save_config: Some(FileSaveConfig::default()),
        operation: ImageOperationKind::BoardPdfExport,
    };

    let err = deliver_document(request, Arc::new(deps))
        .await
        .expect_err("clipboard PDF should fail");

    assert!(
        err.to_string().contains("not supported yet"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn deliver_document_requires_save_config() {
    let deps = CaptureDependencies {
        source: Arc::new(MockSource {
            data: Vec::new(),
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        }),
        saver: Arc::new(RecordingSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/unused.pdf"),
            calls: Arc::new(Mutex::new(0)),
            configs: Arc::new(Mutex::new(Vec::new())),
        }),
        clipboard: Arc::new(RecordingClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
            copied: Arc::new(Mutex::new(Vec::new())),
        }),
    };
    let request = DocumentDeliveryRequest {
        attachments: Vec::new(),
        document: rendered_pdf(Vec::new()),
        destination: CaptureDestination::FileOnly,
        save_config: None,
        operation: ImageOperationKind::BoardPdfExport,
    };

    let err = deliver_document(request, Arc::new(deps))
        .await
        .expect_err("missing save config should fail");

    assert!(
        err.to_string().contains("file save configuration"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn deliver_document_requires_save_directory() {
    let deps = CaptureDependencies {
        source: Arc::new(MockSource {
            data: Vec::new(),
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        }),
        saver: Arc::new(RecordingSaver {
            should_fail: false,
            path: PathBuf::from("/tmp/unused.pdf"),
            calls: Arc::new(Mutex::new(0)),
            configs: Arc::new(Mutex::new(Vec::new())),
        }),
        clipboard: Arc::new(RecordingClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
            copied: Arc::new(Mutex::new(Vec::new())),
        }),
    };
    let request = DocumentDeliveryRequest {
        attachments: Vec::new(),
        document: rendered_pdf(Vec::new()),
        destination: CaptureDestination::FileOnly,
        save_config: Some(FileSaveConfig {
            save_directory: PathBuf::new(),
            ..FileSaveConfig::default()
        }),
        operation: ImageOperationKind::BoardPdfExport,
    };

    let err = deliver_document(request, Arc::new(deps))
        .await
        .expect_err("empty save directory should fail");

    assert!(
        err.to_string().contains("save directory"),
        "unexpected error: {err}"
    );
}

/// A saver that writes real files but refuses one named stem, so a bundle can
/// fail partway through exactly as a disk filling up mid-export would.
#[derive(Clone)]
struct BundleSaver {
    failing_stem: String,
}

impl CaptureFileSaver for BundleSaver {
    fn save(&self, bytes: &[u8], config: &FileSaveConfig) -> Result<PathBuf, CaptureError> {
        if config.filename_template == self.failing_stem {
            return Err(CaptureError::SaveError(std::io::Error::other(
                "no space left on device",
            )));
        }
        let path = config
            .save_directory
            .join(format!("{}.{}", config.filename_template, config.format));
        std::fs::write(&path, bytes).map_err(CaptureError::SaveError)?;
        Ok(path)
    }
}

fn bundle_dependencies(failing_stem: &str) -> CaptureDependencies {
    CaptureDependencies {
        source: Arc::new(MockSource {
            data: Vec::new(),
            error: Arc::new(Mutex::new(None)),
            captured_types: Arc::new(Mutex::new(Vec::new())),
        }),
        saver: Arc::new(BundleSaver {
            failing_stem: failing_stem.to_string(),
        }),
        clipboard: Arc::new(RecordingClipboard {
            should_fail: false,
            calls: Arc::new(Mutex::new(0)),
            copied: Arc::new(Mutex::new(Vec::new())),
        }),
    }
}

fn guide_bundle_request(save_directory: PathBuf) -> DocumentDeliveryRequest {
    DocumentDeliveryRequest {
        attachments: vec![
            DocumentAttachment::bytes("step-01", "png", b"first".to_vec()),
            DocumentAttachment::bytes("step-02", "png", b"second".to_vec()),
        ],
        document: RenderedDocument {
            bytes: b"# Guide\n".to_vec(),
            extension: "md".to_string(),
            mime_type: "text/markdown".to_string(),
        },
        destination: CaptureDestination::FileOnly,
        save_config: Some(FileSaveConfig {
            save_directory,
            filename_template: "steps_guide".to_string(),
            format: "md".to_string(),
        }),
        operation: ImageOperationKind::StepsGuideExport,
    }
}

#[tokio::test]
async fn deliver_document_bundle_writes_document_and_attachments() {
    let temp = crate::test_temp::tempdir().expect("temp dir");
    let deps = bundle_dependencies("");

    let result = deliver_document(
        guide_bundle_request(temp.path().to_path_buf()),
        Arc::new(deps),
    )
    .await
    .expect("bundle delivery succeeds");

    let bundle_dir = result
        .saved_path
        .as_ref()
        .and_then(|path| path.parent())
        .expect("the guide lands inside a bundle directory");
    assert!(bundle_dir.join("guide.md").is_file());
    assert!(bundle_dir.join("step-01.png").is_file());
    assert!(bundle_dir.join("step-02.png").is_file());
}

#[tokio::test]
async fn deliver_document_bundle_renders_canvas_attachments_during_delivery() {
    let temp = crate::test_temp::tempdir().expect("temp dir");
    let deps = bundle_dependencies("");
    let mut request = guide_bundle_request(temp.path().to_path_buf());
    request.attachments[0] = DocumentAttachment::canvas_png(
        "step-01",
        CanvasExportSnapshot {
            viewport: CanvasExportViewport {
                logical_width: 4,
                logical_height: 3,
                scale: 1,
                physical_size: None,
                origin_x: 0,
                origin_y: 0,
            },
            backdrop: CanvasExportBackdropSnapshot::Transparent,
            board: BoardExportSnapshot {
                frame: Frame::new(),
            },
            render_profile: None,
            spotlight: SpotlightPassSnapshot::default(),
        },
    );

    let result = deliver_document(request, Arc::new(deps))
        .await
        .expect("bundle delivery succeeds");
    let image = result
        .saved_path
        .as_ref()
        .and_then(|path| path.parent())
        .map(|directory| directory.join("step-01.png"))
        .and_then(|path| std::fs::read(path).ok())
        .expect("rendered attachment is readable");

    assert!(image.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[tokio::test]
async fn deliver_document_bundle_removes_the_directory_when_an_attachment_fails() {
    let temp = crate::test_temp::tempdir().expect("temp dir");
    // The guide and its first image land; the second image cannot be written.
    let deps = bundle_dependencies("step-02");

    let err = deliver_document(
        guide_bundle_request(temp.path().to_path_buf()),
        Arc::new(deps),
    )
    .await
    .expect_err("a failed attachment fails the export");
    assert!(err.to_string().contains("no space left"), "{err}");

    let leftovers: Vec<_> = std::fs::read_dir(temp.path())
        .expect("temp dir readable")
        .map(|entry| entry.expect("entry").path())
        .collect();
    assert!(
        leftovers.is_empty(),
        "a partial bundle that references missing images must not be left behind: {leftovers:?}"
    );
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
