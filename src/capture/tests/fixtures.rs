use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
};

use crate::capture::{
    dependencies::{
        CaptureClipboard, CaptureClipboardFuture, CaptureFileSaver, CaptureFuture,
        CaptureSaveFuture, CaptureSource,
    },
    file::FileSaveConfig,
    types::{CaptureError, CaptureType},
};

pub(super) struct MockSource {
    data: Vec<u8>,
    error: Option<CaptureError>,
    captured_type_tx: Option<Sender<CaptureType>>,
}

impl MockSource {
    pub(super) fn succeeding(data: Vec<u8>) -> Self {
        Self {
            data,
            error: None,
            captured_type_tx: None,
        }
    }

    pub(super) fn failing(error: CaptureError) -> Self {
        Self {
            data: Vec::new(),
            error: Some(error),
            captured_type_tx: None,
        }
    }

    pub(super) fn recording(data: Vec<u8>) -> (Self, Receiver<CaptureType>) {
        let (captured_type_tx, captured_type_rx) = mpsc::channel();
        (
            Self {
                data,
                error: None,
                captured_type_tx: Some(captured_type_tx),
            },
            captured_type_rx,
        )
    }
}

impl CaptureSource for MockSource {
    fn capture(&mut self, capture_type: CaptureType) -> CaptureFuture<'_> {
        if let Some(captured_type_tx) = self.captured_type_tx.as_ref() {
            let _ = captured_type_tx.send(capture_type);
        }
        let result = match self.error.take() {
            Some(error) => Err(error),
            None => Ok(std::mem::take(&mut self.data)),
        };
        Box::pin(async move { result })
    }
}

pub(super) struct MockSaver {
    should_fail: bool,
    path: PathBuf,
    saved_config_tx: Option<Sender<FileSaveConfig>>,
}

impl MockSaver {
    pub(super) fn succeeding(path: impl Into<PathBuf>) -> Self {
        Self {
            should_fail: false,
            path: path.into(),
            saved_config_tx: None,
        }
    }

    pub(super) fn failing(path: impl Into<PathBuf>) -> Self {
        Self {
            should_fail: true,
            path: path.into(),
            saved_config_tx: None,
        }
    }

    pub(super) fn recording(
        should_fail: bool,
        path: impl Into<PathBuf>,
    ) -> (Self, Receiver<FileSaveConfig>) {
        let (saved_config_tx, saved_config_rx) = mpsc::channel();
        (
            Self {
                should_fail,
                path: path.into(),
                saved_config_tx: Some(saved_config_tx),
            },
            saved_config_rx,
        )
    }
}

impl CaptureFileSaver for MockSaver {
    fn save(&mut self, _image_data: Vec<u8>, config: FileSaveConfig) -> CaptureSaveFuture<'_> {
        if let Some(saved_config_tx) = self.saved_config_tx.as_ref() {
            let _ = saved_config_tx.send(config);
        }
        let result = if self.should_fail {
            Err(CaptureError::SaveError(std::io::Error::other(
                "save failed",
            )))
        } else {
            Ok(self.path.clone())
        };
        Box::pin(async move { result })
    }
}

pub(super) struct MockClipboard {
    should_fail: bool,
    copied_image_tx: Option<Sender<Vec<u8>>>,
}

impl MockClipboard {
    pub(super) fn succeeding() -> Self {
        Self {
            should_fail: false,
            copied_image_tx: None,
        }
    }

    pub(super) fn failing() -> Self {
        Self {
            should_fail: true,
            copied_image_tx: None,
        }
    }

    pub(super) fn recording(should_fail: bool) -> (Self, Receiver<Vec<u8>>) {
        let (copied_image_tx, copied_image_rx) = mpsc::channel();
        (
            Self {
                should_fail,
                copied_image_tx: Some(copied_image_tx),
            },
            copied_image_rx,
        )
    }
}

impl CaptureClipboard for MockClipboard {
    fn copy(&mut self, image_data: Vec<u8>) -> CaptureClipboardFuture<'_> {
        if let Some(copied_image_tx) = self.copied_image_tx.as_ref() {
            let _ = copied_image_tx.send(image_data);
        }
        let result = if self.should_fail {
            Err(CaptureError::ClipboardError(
                "clipboard failure".to_string(),
            ))
        } else {
            Ok(())
        };
        Box::pin(async move { result })
    }
}

pub(super) fn create_placeholder_image() -> Vec<u8> {
    use crate::ui_text::{UiTextStyle, draw_text_baseline};
    use cairo::{Context, FontSlant, FontWeight, Format, ImageSurface};

    let surface = ImageSurface::create(Format::ARgb32, 100, 100)
        .expect("fixture uses positive dimensions supported by Cairo");
    let ctx = Context::new(&surface).expect("fixture surface accepts a Cairo drawing context");

    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.paint()
        .expect("fixture surface accepts an opaque background paint");

    ctx.set_source_rgb(1.0, 1.0, 1.0);
    draw_text_baseline(
        &ctx,
        UiTextStyle {
            family: "Sans",
            slant: FontSlant::Normal,
            weight: FontWeight::Bold,
            size: 20.0,
        },
        "TEST",
        10.0,
        50.0,
        None,
    );

    let mut buffer = Vec::new();
    surface
        .write_to_png(&mut buffer)
        .expect("fixture Cairo surface can be encoded as PNG bytes");
    buffer
}
