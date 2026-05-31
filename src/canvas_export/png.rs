use crate::capture::{CaptureError, ImageFormatMetadata, RenderedImage};
use crate::draw::Frame;
use crate::render_profiles::RenderColorProfile;
use crate::util::Rect;

use super::page::{CanvasExportBackdropSnapshot, CanvasPageExportSnapshot, draw_canvas_page};

#[derive(Debug, Clone)]
pub struct CanvasExportSnapshot {
    pub viewport: CanvasExportViewport,
    pub backdrop: CanvasExportBackdropSnapshot,
    pub board: BoardExportSnapshot,
    pub render_profile: Option<RenderColorProfile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasExportViewport {
    pub logical_width: u32,
    pub logical_height: u32,
    pub scale: i32,
    pub origin_x: i32,
    pub origin_y: i32,
}

#[derive(Debug, Clone)]
pub struct BoardExportSnapshot {
    pub frame: Frame,
}

pub fn render_canvas_png(snapshot: &CanvasExportSnapshot) -> Result<RenderedImage, CaptureError> {
    let surface = render_canvas_surface(snapshot)?;
    let mut bytes = Vec::new();
    surface
        .write_to_png(&mut bytes)
        .map_err(|err| CaptureError::ImageError(format!("Failed to encode canvas PNG: {err}")))?;

    Ok(RenderedImage {
        bytes,
        format: ImageFormatMetadata::png(),
        width: snapshot
            .viewport
            .logical_width
            .saturating_mul(snapshot.viewport.scale.max(1) as u32),
        height: snapshot
            .viewport
            .logical_height
            .saturating_mul(snapshot.viewport.scale.max(1) as u32),
    })
}

pub(crate) fn render_canvas_surface(
    snapshot: &CanvasExportSnapshot,
) -> Result<cairo::ImageSurface, CaptureError> {
    let viewport = snapshot.viewport;
    let scale = viewport.scale.max(1);
    let physical_width = viewport.logical_width.saturating_mul(scale as u32);
    let physical_height = viewport.logical_height.saturating_mul(scale as u32);
    if physical_width == 0 || physical_height == 0 {
        return Err(CaptureError::ImageError(
            "Canvas export requires a configured non-empty surface".to_string(),
        ));
    }

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
        draw_canvas_page(&ctx, &page, scale as f64)?;
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
    }
}
