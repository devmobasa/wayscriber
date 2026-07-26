use std::{path::PathBuf, sync::mpsc::TryRecvError};

use crate::capture::{
    dependencies::{CaptureClipboard, CaptureDependencies, CaptureFileSaver, CaptureSource},
    file::FileSaveConfig,
    pipeline::{CaptureRequest, deliver_document, deliver_image, perform_capture},
    types::{
        CaptureDestination, CaptureError, CaptureType, DocumentDeliveryRequest,
        ImageDeliveryRequest, ImageFormatMetadata, ImageOperationKind, RenderedDocument,
        RenderedImage,
    },
};

use super::fixtures::{MockClipboard, MockSaver, MockSource};

fn dependencies(
    source: impl CaptureSource + 'static,
    saver: impl CaptureFileSaver + 'static,
    clipboard: impl CaptureClipboard + 'static,
) -> CaptureDependencies {
    CaptureDependencies {
        source: Box::new(source),
        saver: Box::new(saver),
        clipboard: Box::new(clipboard),
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
    let (saver, saved_config_rx) = MockSaver::recording(false, "unused.png");
    let (clipboard, copied_image_rx) = MockClipboard::recording(false);
    let mut deps = dependencies(MockSource::succeeding(vec![1, 2, 3]), saver, clipboard);
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::ClipboardOnly,
        save_config: None,
    };

    let result = perform_capture(request, &mut deps)
        .await
        .expect("fixture source and clipboard complete a clipboard-only capture");
    assert_eq!(result.operation, ImageOperationKind::Screenshot);
    assert!(result.fallback_format_override.is_none());
    assert!(result.saved_path.is_none());
    assert!(result.copied_to_clipboard);
    assert_eq!(
        copied_image_rx
            .try_recv()
            .expect("fixture clipboard reports its completed copy before returning"),
        vec![1, 2, 3]
    );
    assert!(matches!(
        saved_config_rx.try_recv(),
        Err(TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn deliver_image_file_only_saves_rendered_format_extension() {
    let (saver, saved_config_rx) = MockSaver::recording(false, "/tmp/canvas.png");
    let mut deps = dependencies(
        MockSource::succeeding(Vec::new()),
        saver,
        MockClipboard::succeeding(),
    );
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

    let result = deliver_image(request, &mut deps)
        .await
        .expect("fixture saver completes a file-only image delivery");

    assert_eq!(result.operation, ImageOperationKind::CanvasExport);
    assert_eq!(
        result.fallback_format_override,
        Some(ImageFormatMetadata::png())
    );
    assert_eq!(
        saved_config_rx
            .try_recv()
            .expect("fixture saver reports the config it received before returning")
            .format,
        "png"
    );
    assert_eq!(result.saved_path, Some(PathBuf::from("/tmp/canvas.png")));
}

#[tokio::test]
async fn deliver_document_file_only_saves_pdf_bytes_with_pdf_extension() {
    let (saver, saved_config_rx) = MockSaver::recording(false, "/tmp/board.pdf");
    let (clipboard, copied_image_rx) = MockClipboard::recording(false);
    let mut deps = dependencies(MockSource::succeeding(Vec::new()), saver, clipboard);
    let request = DocumentDeliveryRequest {
        document: rendered_pdf(b"%PDF-".to_vec()),
        destination: CaptureDestination::FileOnly,
        save_config: Some(FileSaveConfig {
            format: "png".to_string(),
            ..FileSaveConfig::default()
        }),
        operation: ImageOperationKind::BoardPdfExport,
    };

    let result = deliver_document(request, &mut deps)
        .await
        .expect("fixture saver completes a file-only document delivery");

    assert_eq!(result.operation, ImageOperationKind::BoardPdfExport);
    assert_eq!(result.image_data, b"%PDF-".to_vec());
    assert_eq!(result.saved_path, Some(PathBuf::from("/tmp/board.pdf")));
    assert!(!result.copied_to_clipboard);
    assert_eq!(
        saved_config_rx
            .try_recv()
            .expect("fixture saver reports the document config before returning")
            .format,
        "pdf"
    );
    assert!(matches!(
        copied_image_rx.try_recv(),
        Err(TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn deliver_document_rejects_clipboard_destinations() {
    let mut deps = dependencies(
        MockSource::succeeding(Vec::new()),
        MockSaver::succeeding("/tmp/unused.pdf"),
        MockClipboard::succeeding(),
    );
    let request = DocumentDeliveryRequest {
        document: rendered_pdf(Vec::new()),
        destination: CaptureDestination::ClipboardOnly,
        save_config: Some(FileSaveConfig::default()),
        operation: ImageOperationKind::BoardPdfExport,
    };

    let err = deliver_document(request, &mut deps)
        .await
        .expect_err("fixture requests the unsupported clipboard document destination");

    assert!(
        err.to_string().contains("not supported yet"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn deliver_document_requires_save_config() {
    let mut deps = dependencies(
        MockSource::succeeding(Vec::new()),
        MockSaver::succeeding("/tmp/unused.pdf"),
        MockClipboard::succeeding(),
    );
    let request = DocumentDeliveryRequest {
        document: rendered_pdf(Vec::new()),
        destination: CaptureDestination::FileOnly,
        save_config: None,
        operation: ImageOperationKind::BoardPdfExport,
    };

    let err = deliver_document(request, &mut deps)
        .await
        .expect_err("fixture omits the required document save config");

    assert!(
        err.to_string().contains("file save configuration"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn deliver_document_requires_save_directory() {
    let mut deps = dependencies(
        MockSource::succeeding(Vec::new()),
        MockSaver::succeeding("/tmp/unused.pdf"),
        MockClipboard::succeeding(),
    );
    let request = DocumentDeliveryRequest {
        document: rendered_pdf(Vec::new()),
        destination: CaptureDestination::FileOnly,
        save_config: Some(FileSaveConfig {
            save_directory: PathBuf::new(),
            ..FileSaveConfig::default()
        }),
        operation: ImageOperationKind::BoardPdfExport,
    };

    let err = deliver_document(request, &mut deps)
        .await
        .expect_err("fixture supplies an empty document save directory");

    assert!(
        err.to_string().contains("save directory"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn deliver_image_clipboard_only_copies_png_bytes() {
    let (clipboard, copied_image_rx) = MockClipboard::recording(false);
    let mut deps = dependencies(
        MockSource::succeeding(Vec::new()),
        MockSaver::succeeding("/tmp/unused.png"),
        clipboard,
    );
    let bytes = vec![1, 2, 3, 4];
    let request = ImageDeliveryRequest {
        image: rendered_png(bytes.clone()),
        destination: CaptureDestination::ClipboardOnly,
        save_config: None,
        operation: ImageOperationKind::CanvasExport,
        fallback_format_override: Some(ImageFormatMetadata::png()),
    };

    let result = deliver_image(request, &mut deps)
        .await
        .expect("fixture clipboard completes a clipboard-only image delivery");

    assert!(result.copied_to_clipboard);
    assert_eq!(
        copied_image_rx
            .try_recv()
            .expect("fixture clipboard reports the copied bytes before returning"),
        bytes
    );
}

#[tokio::test]
async fn deliver_image_clipboard_and_file_keeps_file_success_when_clipboard_fails() {
    let mut deps = dependencies(
        MockSource::succeeding(Vec::new()),
        MockSaver::succeeding("/tmp/partial.png"),
        MockClipboard::failing(),
    );
    let request = ImageDeliveryRequest {
        image: rendered_png(vec![1, 2, 3]),
        destination: CaptureDestination::ClipboardAndFile,
        save_config: Some(FileSaveConfig::default()),
        operation: ImageOperationKind::CanvasExport,
        fallback_format_override: Some(ImageFormatMetadata::png()),
    };

    let result = deliver_image(request, &mut deps)
        .await
        .expect("fixture file save succeeds when its clipboard peer fails");

    assert_eq!(result.saved_path, Some(PathBuf::from("/tmp/partial.png")));
    assert!(!result.copied_to_clipboard);
}

#[tokio::test]
async fn deliver_image_clipboard_and_file_keeps_clipboard_success_when_file_fails() {
    let (clipboard, copied_image_rx) = MockClipboard::recording(false);
    let mut deps = dependencies(
        MockSource::succeeding(Vec::new()),
        MockSaver::failing("/tmp/partial.png"),
        clipboard,
    );
    let request = ImageDeliveryRequest {
        image: rendered_png(vec![1, 2, 3]),
        destination: CaptureDestination::ClipboardAndFile,
        save_config: Some(FileSaveConfig::default()),
        operation: ImageOperationKind::CanvasExport,
        fallback_format_override: Some(ImageFormatMetadata::png()),
    };

    let result = deliver_image(request, &mut deps)
        .await
        .expect("fixture clipboard succeeds when its file-save peer fails");

    assert!(result.saved_path.is_none());
    assert!(result.copied_to_clipboard);
    assert_eq!(
        copied_image_rx
            .try_recv()
            .expect("fixture clipboard reports its successful copy before returning"),
        vec![1, 2, 3]
    );
}

#[tokio::test]
async fn test_perform_capture_file_only_success() {
    let (saver, saved_config_rx) = MockSaver::recording(false, "/tmp/test.png");
    let (clipboard, copied_image_rx) = MockClipboard::recording(false);
    let mut deps = dependencies(MockSource::succeeding(vec![4, 5, 6]), saver, clipboard);
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::FileOnly,
        save_config: Some(FileSaveConfig::default()),
    };

    let result = perform_capture(request, &mut deps)
        .await
        .expect("fixture source and saver complete a file-only capture");
    assert!(result.saved_path.is_some());
    assert!(!result.copied_to_clipboard);
    saved_config_rx
        .try_recv()
        .expect("fixture saver reports its file-only capture before returning");
    assert!(matches!(
        copied_image_rx.try_recv(),
        Err(TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn test_perform_capture_clipboard_failure() {
    let (clipboard, copied_image_rx) = MockClipboard::recording(true);
    let mut deps = dependencies(
        MockSource::succeeding(vec![7, 8, 9]),
        MockSaver::succeeding("/tmp/a.png"),
        clipboard,
    );
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::ClipboardOnly,
        save_config: None,
    };

    let result = perform_capture(request, &mut deps)
        .await
        .expect("fixture pipeline preserves a handled clipboard failure");
    assert!(!result.copied_to_clipboard);
    assert_eq!(
        copied_image_rx
            .try_recv()
            .expect("fixture clipboard reports its failed copy attempt before returning"),
        vec![7, 8, 9]
    );
}

#[tokio::test]
async fn test_perform_capture_save_failure() {
    let (saver, saved_config_rx) = MockSaver::recording(true, "/tmp/should_fail.png");
    let mut deps = dependencies(
        MockSource::succeeding(vec![10, 11, 12]),
        saver,
        MockClipboard::succeeding(),
    );
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::FileOnly,
        save_config: Some(FileSaveConfig::default()),
    };

    let err = perform_capture(request, &mut deps)
        .await
        .expect_err("fixture saver rejects the file-only capture");
    assert!(
        matches!(&err, CaptureError::SaveError(_)),
        "expected SaveError, got {err:?}"
    );
    saved_config_rx
        .try_recv()
        .expect("fixture saver reports its failed save attempt before returning");
}

#[tokio::test]
async fn test_perform_capture_clipboard_and_file_success() {
    let (saver, saved_config_rx) = MockSaver::recording(false, "/tmp/combined.png");
    let (clipboard, copied_image_rx) = MockClipboard::recording(false);
    let mut deps = dependencies(MockSource::succeeding(vec![21, 22, 23]), saver, clipboard);
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::ClipboardAndFile,
        save_config: Some(FileSaveConfig::default()),
    };

    let result = perform_capture(request, &mut deps)
        .await
        .expect("fixture saver and clipboard complete the combined capture");
    assert!(result.saved_path.is_some());
    assert!(result.copied_to_clipboard);
    saved_config_rx
        .try_recv()
        .expect("fixture saver reports the combined capture before returning");
    copied_image_rx
        .try_recv()
        .expect("fixture clipboard reports the combined capture before returning");
}

#[tokio::test]
async fn test_perform_capture_clipboard_and_file_save_failure_still_copies() {
    let (saver, saved_config_rx) = MockSaver::recording(true, "/tmp/combined_fail.png");
    let (clipboard, copied_image_rx) = MockClipboard::recording(false);
    let mut deps = dependencies(MockSource::succeeding(vec![21, 22, 23]), saver, clipboard);
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::ClipboardAndFile,
        save_config: Some(FileSaveConfig::default()),
    };

    let result = perform_capture(request, &mut deps)
        .await
        .expect("fixture combined capture retains its successful clipboard result");
    assert!(result.saved_path.is_none());
    assert!(result.copied_to_clipboard);
    saved_config_rx
        .try_recv()
        .expect("fixture saver reports its failed combined save before returning");
    copied_image_rx
        .try_recv()
        .expect("fixture clipboard reports its successful combined copy before returning");
}

#[tokio::test]
async fn perform_capture_propagates_source_error() {
    let mut deps = dependencies(
        MockSource::failing(CaptureError::ImageError("boom".to_string())),
        MockSaver::succeeding("/tmp/unneeded.png"),
        MockClipboard::succeeding(),
    );
    let request = CaptureRequest {
        capture_type: CaptureType::FullScreen,
        destination: CaptureDestination::ClipboardOnly,
        save_config: None,
    };

    let err = perform_capture(request, &mut deps)
        .await
        .expect_err("fixture source returns its configured image error");
    assert!(
        matches!(&err, CaptureError::ImageError(message) if message.contains("boom")),
        "expected ImageError containing 'boom', got {err:?}"
    );
}
