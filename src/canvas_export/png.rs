use crate::capture::{CaptureError, ImageFormatMetadata, RenderedImage};
use crate::draw::Frame;
use crate::render_profiles::RenderColorProfile;
use crate::util::Rect;

use super::page::{CanvasExportBackdropSnapshot, CanvasPageExportSnapshot, SpotlightPassSnapshot};

#[derive(Debug, Clone)]
pub struct CanvasExportSnapshot {
    pub viewport: CanvasExportViewport,
    pub backdrop: CanvasExportBackdropSnapshot,
    pub board: BoardExportSnapshot,
    pub render_profile: Option<RenderColorProfile>,
    /// Spotlight appearance, mirrored from the live overlay.
    pub spotlight: SpotlightPassSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasExportViewport {
    pub logical_width: u32,
    pub logical_height: u32,
    pub scale: i32,
    /// Exact output pixels when capture geometry is fractional or otherwise
    /// cannot be represented by the integer Wayland buffer scale.
    pub physical_size: Option<(u32, u32)>,
    pub origin_x: i32,
    pub origin_y: i32,
}

impl CanvasExportViewport {
    fn physical_dimensions(self) -> Option<(u32, u32)> {
        if self.logical_width == 0 || self.logical_height == 0 {
            return None;
        }
        match self.physical_size {
            Some((width, height)) if width > 0 && height > 0 => Some((width, height)),
            Some(_) => None,
            None => {
                let scale = u32::try_from(self.scale.max(1)).ok()?;
                Some((
                    self.logical_width.checked_mul(scale)?,
                    self.logical_height.checked_mul(scale)?,
                ))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct BoardExportSnapshot {
    pub frame: Frame,
}

pub fn render_canvas_png(snapshot: &CanvasExportSnapshot) -> Result<RenderedImage, CaptureError> {
    let surface = render_canvas_surface(snapshot)?;
    let width = surface.width().max(0) as u32;
    let height = surface.height().max(0) as u32;
    let mut bytes = Vec::new();
    surface
        .write_to_png(&mut bytes)
        .map_err(|err| CaptureError::ImageError(format!("Failed to encode canvas PNG: {err}")))?;

    Ok(RenderedImage {
        bytes,
        format: ImageFormatMetadata::png(),
        width,
        height,
    })
}

pub(crate) fn render_canvas_surface(
    snapshot: &CanvasExportSnapshot,
) -> Result<cairo::ImageSurface, CaptureError> {
    let viewport = snapshot.viewport;
    let Some((physical_width, physical_height)) = viewport.physical_dimensions() else {
        return Err(CaptureError::ImageError(
            "Canvas export requires a configured non-empty surface".to_string(),
        ));
    };

    let mut surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        physical_width as i32,
        physical_height as i32,
    )
    .map_err(|err| CaptureError::ImageError(format!("Failed to create canvas surface: {err}")))?;
    {
        let ctx = cairo::Context::new(&surface).map_err(|err| {
            CaptureError::ImageError(format!("Failed to create canvas context: {err}"))
        })?;

        let page = canvas_page_from_snapshot(snapshot);
        let output_scale_x = physical_width as f64 / viewport.logical_width as f64;
        let output_scale_y = physical_height as f64 / viewport.logical_height as f64;
        super::page::draw_canvas_page_scaled(&ctx, &page, output_scale_x, output_scale_y)?;
    }

    if let Some(profile) = snapshot.render_profile.as_ref() {
        surface.flush();
        {
            let width = surface.width();
            let height = surface.height();
            let stride = surface.stride();
            let mut data = surface.data().map_err(|err| {
                CaptureError::ImageError(format!("Failed to access canvas pixels: {err}"))
            })?;
            profile.remap_argb8888_regions(
                &mut data,
                width,
                height,
                stride,
                &[Rect {
                    x: 0,
                    y: 0,
                    width,
                    height,
                }],
            );
        }
        surface.mark_dirty();
    }

    Ok(surface)
}

fn canvas_page_from_snapshot(snapshot: &CanvasExportSnapshot) -> CanvasPageExportSnapshot {
    CanvasPageExportSnapshot {
        frame: snapshot.board.frame.clone_without_history(),
        backdrop: snapshot.backdrop.clone(),
        viewport_width: snapshot.viewport.logical_width,
        viewport_height: snapshot.viewport.logical_height,
        origin_x: snapshot.viewport.origin_x,
        origin_y: snapshot.viewport.origin_y,
        spotlight: snapshot.spotlight,
    }
}
