use std::sync::Arc;

use crate::capture::CaptureError;
use crate::draw::{
    BlurRectParams, Color, EraserReplayContext, Frame, Shape, render_blur_rect,
    render_eraser_stroke, render_shape,
};

#[derive(Debug, Clone)]
pub struct CanvasPageExportSnapshot {
    pub frame: Frame,
    pub backdrop: CanvasExportBackdropSnapshot,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub origin_x: i32,
    pub origin_y: i32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Persisted image backdrops are currently exercised by export tests.
pub enum CanvasExportBackdropSnapshot {
    Transparent,
    Solid(Color),
    PersistedImage {
        data: Arc<[u8]>,
        width: i32,
        height: i32,
        stride: i32,
        logical_to_image_scale_x: f64,
        logical_to_image_scale_y: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasExportRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl CanvasExportRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Option<Self> {
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            return None;
        }
        Some(Self {
            x,
            y,
            width,
            height,
        })
    }
}

pub fn draw_canvas_page(
    ctx: &cairo::Context,
    page: &CanvasPageExportSnapshot,
    output_scale: f64,
) -> Result<(), CaptureError> {
    let backdrop = ExportBackdrop::new(&page.backdrop)?;
    let source = CanvasExportRect {
        x: page.origin_x as f64,
        y: page.origin_y as f64,
        width: page.viewport_width as f64,
        height: page.viewport_height as f64,
    };
    let destination = CanvasExportRect {
        x: 0.0,
        y: 0.0,
        width: page.viewport_width as f64,
        height: page.viewport_height as f64,
    };

    let _ = ctx.save();
    if (output_scale - 1.0).abs() > f64::EPSILON {
        ctx.scale(output_scale, output_scale);
    }
    draw_canvas_page_region(ctx, page, &backdrop, source, destination, true);
    let _ = ctx.restore();
    Ok(())
}

pub(crate) fn draw_canvas_page_region(
    ctx: &cairo::Context,
    page: &CanvasPageExportSnapshot,
    backdrop: &ExportBackdrop,
    source: CanvasExportRect,
    destination: CanvasExportRect,
    paint_backdrop: bool,
) {
    let _ = ctx.save();
    ctx.rectangle(
        destination.x,
        destination.y,
        destination.width,
        destination.height,
    );
    ctx.clip();
    ctx.translate(destination.x, destination.y);
    ctx.scale(
        destination.width / source.width,
        destination.height / source.height,
    );
    ctx.translate(-source.x, -source.y);
    draw_canvas_page_contents(ctx, page, backdrop, paint_backdrop);
    let _ = ctx.restore();
}

pub(crate) fn paint_pdf_page_background(
    ctx: &cairo::Context,
    page: &CanvasPageExportSnapshot,
    width: f64,
    height: f64,
) {
    let CanvasExportBackdropSnapshot::Solid(color) = page.backdrop else {
        return;
    };
    let _ = ctx.save();
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.rectangle(0.0, 0.0, width, height);
    let _ = ctx.fill();
    let _ = ctx.restore();
}

pub(crate) struct ExportBackdrop {
    surface: Option<cairo::ImageSurface>,
    pattern: Option<cairo::SurfacePattern>,
    bg_color: Option<Color>,
    logical_to_image_scale_x: f64,
    logical_to_image_scale_y: f64,
}

impl ExportBackdrop {
    pub(crate) fn new(snapshot: &CanvasExportBackdropSnapshot) -> Result<Self, CaptureError> {
        match snapshot {
            CanvasExportBackdropSnapshot::Transparent => Ok(Self {
                surface: None,
                pattern: None,
                bg_color: None,
                logical_to_image_scale_x: 1.0,
                logical_to_image_scale_y: 1.0,
            }),
            CanvasExportBackdropSnapshot::Solid(color) => Ok(Self {
                surface: None,
                pattern: None,
                bg_color: Some(*color),
                logical_to_image_scale_x: 1.0,
                logical_to_image_scale_y: 1.0,
            }),
            CanvasExportBackdropSnapshot::PersistedImage {
                data,
                width,
                height,
                stride,
                logical_to_image_scale_x,
                logical_to_image_scale_y,
            } => {
                validate_persisted_image_backdrop(data.len(), *width, *height, *stride)?;

                // SAFETY: dimensions and stride have been checked, and the Arc-backed
                // byte slice covers every row Cairo may read for this temporary surface.
                // The surface is owned by ExportBackdrop and dropped before the snapshot.
                let surface = unsafe {
                    cairo::ImageSurface::create_for_data_unsafe(
                        data.as_ptr() as *mut u8,
                        cairo::Format::ARgb32,
                        *width,
                        *height,
                        *stride,
                    )
                }
                .map_err(|err| {
                    CaptureError::ImageError(format!("Failed to create export backdrop: {err}"))
                })?;
                let pattern = cairo::SurfacePattern::create(&surface);
                pattern.set_extend(cairo::Extend::Pad);
                let mut matrix = cairo::Matrix::identity();
                matrix.scale(
                    logical_to_image_scale_x.max(f64::MIN_POSITIVE),
                    logical_to_image_scale_y.max(f64::MIN_POSITIVE),
                );
                pattern.set_matrix(matrix);
                Ok(Self {
                    surface: Some(surface),
                    pattern: Some(pattern),
                    bg_color: None,
                    logical_to_image_scale_x: *logical_to_image_scale_x,
                    logical_to_image_scale_y: *logical_to_image_scale_y,
                })
            }
        }
    }

    fn paint(&self, ctx: &cairo::Context) {
        if let Some(color) = self.bg_color {
            ctx.set_source_rgba(color.r, color.g, color.b, color.a);
            let _ = ctx.paint();
            return;
        }

        let Some(surface) = self.surface.as_ref() else {
            return;
        };
        let _ = ctx.save();
        ctx.scale(
            1.0 / self.logical_to_image_scale_x.max(f64::MIN_POSITIVE),
            1.0 / self.logical_to_image_scale_y.max(f64::MIN_POSITIVE),
        );
        if ctx.set_source_surface(surface, 0.0, 0.0).is_ok() {
            let _ = ctx.paint();
        }
        let _ = ctx.restore();
    }

    fn replay_context(&self) -> EraserReplayContext<'_> {
        EraserReplayContext {
            pattern: self.pattern.as_ref().map(|p| p as &cairo::Pattern),
            surface: self.surface.as_ref(),
            backdrop_cache_key: self.surface.as_ref().map(|_| 1),
            bg_color: self.bg_color,
            logical_to_image_scale_x: self.logical_to_image_scale_x,
            logical_to_image_scale_y: self.logical_to_image_scale_y,
        }
    }
}

fn draw_canvas_page_contents(
    ctx: &cairo::Context,
    page: &CanvasPageExportSnapshot,
    backdrop: &ExportBackdrop,
    paint_backdrop: bool,
) {
    if paint_backdrop {
        backdrop.paint(ctx);
    }
    let replay_ctx = backdrop.replay_context();
    for drawn_shape in &page.frame.shapes {
        match &drawn_shape.shape {
            Shape::EraserStroke { points, brush } => {
                render_eraser_stroke(ctx, points, brush, &replay_ctx);
            }
            Shape::BlurRect {
                x,
                y,
                w,
                h,
                strength,
            } => render_blur_rect(
                ctx,
                BlurRectParams {
                    x: *x,
                    y: *y,
                    w: *w,
                    h: *h,
                    strength: *strength,
                    cacheable: false,
                },
                &replay_ctx,
            ),
            other => render_shape(ctx, other),
        }
    }
}

fn validate_persisted_image_backdrop(
    data_len: usize,
    width: i32,
    height: i32,
    stride: i32,
) -> Result<(), CaptureError> {
    if width <= 0 || height <= 0 {
        return Err(CaptureError::ImageError(format!(
            "Invalid export backdrop dimensions: {width}x{height}"
        )));
    }
    if stride <= 0 {
        return Err(CaptureError::ImageError(format!(
            "Invalid export backdrop stride: {stride}"
        )));
    }

    let width = width as usize;
    let height = height as usize;
    let stride = stride as usize;
    let min_stride = width.checked_mul(4).ok_or_else(|| {
        CaptureError::ImageError("Export backdrop width is too large".to_string())
    })?;
    if stride < min_stride {
        return Err(CaptureError::ImageError(format!(
            "Export backdrop stride {stride} is too small for width {width}"
        )));
    }

    let required_len = stride.checked_mul(height).ok_or_else(|| {
        CaptureError::ImageError("Export backdrop buffer size overflow".to_string())
    })?;
    if data_len < required_len {
        return Err(CaptureError::ImageError(format!(
            "Export backdrop buffer is too small: {data_len} bytes for {required_len} required"
        )));
    }

    Ok(())
}
