use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;

use crate::capture::{
    dependencies::{CaptureClipboard, CaptureFileSaver, CaptureSource},
    file::FileSaveConfig,
    types::{CaptureError, CaptureType},
};

#[derive(Clone)]
pub(super) struct MockSource {
    pub(super) data: Vec<u8>,
    pub(super) error: Arc<Mutex<Option<CaptureError>>>,
    pub(super) captured_types: Arc<Mutex<Vec<CaptureType>>>,
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
pub(super) struct MockSaver {
    pub(super) should_fail: bool,
    pub(super) path: PathBuf,
    pub(super) calls: Arc<Mutex<usize>>,
}

impl CaptureFileSaver for MockSaver {
    fn save(&self, _image_data: &[u8], _config: &FileSaveConfig) -> Result<PathBuf, CaptureError> {
        *self.calls.lock().unwrap() += 1;
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
pub(super) struct MockClipboard {
    pub(super) should_fail: bool,
    pub(super) calls: Arc<Mutex<usize>>,
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

pub(super) fn create_placeholder_image() -> Vec<u8> {
    use crate::ui_text::{UiTextStyle, draw_text_baseline};
    use cairo::{Context, FontSlant, FontWeight, Format, ImageSurface};

    let surface = ImageSurface::create(Format::ARgb32, 100, 100).unwrap();
    let ctx = Context::new(&surface).unwrap();

    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.paint().unwrap();

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
    surface.write_to_png(&mut buffer).unwrap();
    buffer
}
