use std::sync::Arc;

use crate::capture::{CaptureError, ImageFormatMetadata, RenderedImage};
use crate::draw::{
    BlurRectParams, Color, EraserReplayContext, Frame, Shape, render_blur_rect,
    render_eraser_stroke, render_shape,
};
use crate::render_profiles::RenderColorProfile;
use crate::util::Rect;

#[derive(Debug, Clone)]
pub struct CanvasExportSnapshot {
    pub viewport: CanvasExportViewport,
    pub backdrop: CanvasExportBackdropSnapshot,
    pub board: BoardExportSnapshot,
    pub render_profile: Option<RenderColorProfile>,
}

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
pub struct BoardPdfExportSnapshot {
    pub logical_width: u32,
    pub logical_height: u32,
    pub pages: Vec<CanvasPageExportSnapshot>,
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

fn render_canvas_surface(
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

pub fn draw_canvas_page(
    ctx: &cairo::Context,
    page: &CanvasPageExportSnapshot,
    output_scale: f64,
) -> Result<(), CaptureError> {
    let backdrop = ExportBackdrop::new(&page.backdrop)?;
    draw_canvas_page_contents(ctx, page, &backdrop, output_scale);
    Ok(())
}

pub fn render_board_pdf(snapshot: &BoardPdfExportSnapshot) -> Result<Vec<u8>, CaptureError> {
    if snapshot.logical_width == 0 || snapshot.logical_height == 0 {
        return Err(CaptureError::ImageError(
            "Board PDF export requires a configured non-empty surface".to_string(),
        ));
    }
    if snapshot.pages.is_empty() {
        return Err(CaptureError::ImageError(
            "Board PDF export requires at least one page".to_string(),
        ));
    }

    let width = snapshot.logical_width as f64;
    let height = snapshot.logical_height as f64;
    let surface = cairo::PdfSurface::for_stream(width, height, Vec::<u8>::new())
        .map_err(|err| CaptureError::ImageError(format!("Failed to create PDF surface: {err}")))?;
    let ctx = cairo::Context::new(&surface)
        .map_err(|err| CaptureError::ImageError(format!("Failed to create PDF context: {err}")))?;

    for page in &snapshot.pages {
        surface.set_size(width, height).map_err(|err| {
            CaptureError::ImageError(format!("Failed to set PDF page size: {err}"))
        })?;
        draw_canvas_page(&ctx, page, 1.0)?;
        ctx.show_page()
            .map_err(|err| CaptureError::ImageError(format!("Failed to finish PDF page: {err}")))?;
    }

    drop(ctx);

    let stream = surface
        .finish_output_stream()
        .map_err(|err| CaptureError::ImageError(format!("Failed to finish PDF output: {err}")))?;
    let bytes = stream.downcast::<Vec<u8>>().map_err(|_| {
        CaptureError::ImageError("PDF output stream had unexpected type".to_string())
    })?;
    Ok(*bytes)
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

fn draw_canvas_page_contents(
    ctx: &cairo::Context,
    page: &CanvasPageExportSnapshot,
    backdrop: &ExportBackdrop,
    output_scale: f64,
) {
    let _ = ctx.save();
    if (output_scale - 1.0).abs() > f64::EPSILON {
        ctx.scale(output_scale, output_scale);
    }
    ctx.rectangle(
        0.0,
        0.0,
        page.viewport_width as f64,
        page.viewport_height as f64,
    );
    ctx.clip();
    ctx.translate(-(page.origin_x as f64), -(page.origin_y as f64));

    backdrop.paint(ctx);
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

    let _ = ctx.restore();
}

struct ExportBackdrop {
    surface: Option<cairo::ImageSurface>,
    pattern: Option<cairo::SurfacePattern>,
    bg_color: Option<Color>,
    logical_to_image_scale_x: f64,
    logical_to_image_scale_y: f64,
}

impl ExportBackdrop {
    fn new(snapshot: &CanvasExportBackdropSnapshot) -> Result<Self, CaptureError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RenderColorMappingConfig, RenderProfileConfig};
    use crate::draw::{BLACK, RED, Shape, WHITE};

    fn snapshot(frame: Frame, viewport: CanvasExportViewport) -> CanvasExportSnapshot {
        CanvasExportSnapshot {
            viewport,
            backdrop: CanvasExportBackdropSnapshot::Transparent,
            board: BoardExportSnapshot { frame },
            render_profile: None,
        }
    }

    fn page_snapshot(frame: Frame) -> CanvasPageExportSnapshot {
        CanvasPageExportSnapshot {
            frame,
            backdrop: CanvasExportBackdropSnapshot::Transparent,
            viewport_width: 20,
            viewport_height: 20,
            origin_x: 0,
            origin_y: 0,
        }
    }

    fn pixel(surface: &mut cairo::ImageSurface, x: i32, y: i32) -> u32 {
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().expect("surface data");
        let offset = y as usize * stride + x as usize * 4;
        u32::from_ne_bytes(data[offset..offset + 4].try_into().expect("pixel"))
    }

    #[test]
    fn export_uses_current_viewport_origin() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 20,
            y: 10,
            w: 8,
            h: 8,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        let mut surface = render_canvas_surface(&snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 20,
                logical_height: 20,
                scale: 1,
                origin_x: 20,
                origin_y: 10,
            },
        ))
        .expect("surface");

        assert_ne!(pixel(&mut surface, 3, 3), 0);
    }

    #[test]
    fn export_scale_creates_physical_surface_and_scales_geometry() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 1,
            y: 1,
            w: 4,
            h: 4,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        let mut surface = render_canvas_surface(&snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 10,
                logical_height: 10,
                scale: 2,
                origin_x: 0,
                origin_y: 0,
            },
        ))
        .expect("surface");

        assert_eq!(surface.width(), 20);
        assert_eq!(surface.height(), 20);
        assert_ne!(pixel(&mut surface, 4, 4), 0);
        assert_eq!(pixel(&mut surface, 0, 0), 0);
    }

    #[test]
    fn draw_canvas_page_uses_explicit_output_scale() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 4,
            y: 4,
            w: 2,
            h: 2,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, 20, 20).expect("surface");
        {
            let ctx = cairo::Context::new(&surface).expect("context");
            draw_canvas_page(&ctx, &page_snapshot(frame), 2.0).expect("draw");
        }

        assert_ne!(pixel(&mut surface, 9, 9), 0);
        assert_eq!(pixel(&mut surface, 1, 1), 0);
    }

    #[test]
    fn render_board_pdf_returns_pdf_bytes() {
        let pdf = render_board_pdf(&BoardPdfExportSnapshot {
            logical_width: 64,
            logical_height: 48,
            pages: vec![page_snapshot(Frame::new())],
        })
        .expect("pdf");

        assert!(pdf.starts_with(b"%PDF-"));
    }

    #[test]
    fn render_board_pdf_rejects_zero_dimensions() {
        let err = render_board_pdf(&BoardPdfExportSnapshot {
            logical_width: 0,
            logical_height: 48,
            pages: vec![page_snapshot(Frame::new())],
        })
        .expect_err("zero width should fail");

        assert!(
            err.to_string().contains("non-empty surface"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn render_board_pdf_rejects_empty_pages() {
        let err = render_board_pdf(&BoardPdfExportSnapshot {
            logical_width: 64,
            logical_height: 48,
            pages: Vec::new(),
        })
        .expect_err("empty pages should fail");

        assert!(
            err.to_string().contains("at least one page"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn export_applies_cloned_profile_to_pixels() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 6,
            h: 6,
            fill: true,
            color: BLACK,
            thick: 1.0,
        });
        let profile = RenderColorProfile::from_config(&RenderProfileConfig {
            id: "print".to_string(),
            name: "Print".to_string(),
            mappings: vec![RenderColorMappingConfig {
                from: "#000000".to_string(),
                to: "#FFFFFF".to_string(),
            }],
        })
        .expect("profile");
        let mut export = snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 8,
                logical_height: 8,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        );
        export.render_profile = Some(profile);

        let mut surface = render_canvas_surface(&export).expect("surface");

        assert_eq!(pixel(&mut surface, 2, 2), 0xffffffff);
    }

    #[test]
    fn export_replays_eraser_on_solid_background() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 12,
            h: 12,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        frame.add_shape(Shape::EraserStroke {
            points: vec![(6, 6)],
            brush: crate::draw::EraserBrush {
                size: 6.0,
                kind: crate::draw::EraserKind::Circle,
            },
        });
        let mut export = snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 14,
                logical_height: 14,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        );
        export.backdrop = CanvasExportBackdropSnapshot::Solid(WHITE);
        let mut surface = render_canvas_surface(&export).expect("surface");

        assert_eq!(pixel(&mut surface, 6, 6), 0xffffffff);
    }

    #[test]
    fn export_replays_eraser_on_transparent_background() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 12,
            h: 12,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        frame.add_shape(Shape::EraserStroke {
            points: vec![(6, 6)],
            brush: crate::draw::EraserBrush {
                size: 6.0,
                kind: crate::draw::EraserKind::Circle,
            },
        });
        let mut surface = render_canvas_surface(&snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 14,
                logical_height: 14,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        ))
        .expect("surface");

        assert_eq!(pixel(&mut surface, 6, 6), 0);
    }

    #[test]
    fn export_blur_uses_placeholder_without_persisted_backdrop() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::BlurRect {
            x: 2,
            y: 2,
            w: 8,
            h: 8,
            strength: 12.0,
        });
        let mut surface = render_canvas_surface(&snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 14,
                logical_height: 14,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        ))
        .expect("surface");

        assert_ne!(pixel(&mut surface, 5, 5), 0);
    }

    #[test]
    fn export_blur_replays_against_persisted_image_backdrop() {
        let width = 16;
        let height = 16;
        let stride = width * 4;
        let mut data = vec![0u8; (stride * height) as usize];
        for y in 0..height {
            for x in 0..width {
                let offset = (y * stride + x * 4) as usize;
                let red = if x < 8 { 255 } else { 0 };
                let blue = if x < 8 { 0 } else { 255 };
                data[offset..offset + 4].copy_from_slice(
                    &(0xff000000u32 | ((red as u32) << 16) | blue as u32).to_ne_bytes(),
                );
            }
        }
        let mut frame = Frame::new();
        frame.add_shape(Shape::BlurRect {
            x: 4,
            y: 4,
            w: 8,
            h: 8,
            strength: 12.0,
        });
        let mut export = snapshot(
            frame,
            CanvasExportViewport {
                logical_width: width as u32,
                logical_height: height as u32,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        );
        export.backdrop = CanvasExportBackdropSnapshot::PersistedImage {
            data: Arc::from(data),
            width,
            height,
            stride,
            logical_to_image_scale_x: 1.0,
            logical_to_image_scale_y: 1.0,
        };
        let mut surface = render_canvas_surface(&export).expect("surface");

        assert_ne!(pixel(&mut surface, 6, 6), 0);
        assert_ne!(pixel(&mut surface, 6, 6), pixel(&mut surface, 1, 1));
    }

    #[test]
    fn export_rejects_invalid_persisted_image_backdrop_buffer() {
        let mut export = snapshot(
            Frame::new(),
            CanvasExportViewport {
                logical_width: 4,
                logical_height: 4,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        );
        export.backdrop = CanvasExportBackdropSnapshot::PersistedImage {
            data: Arc::from(vec![0u8; 8]),
            width: 4,
            height: 4,
            stride: 16,
            logical_to_image_scale_x: 1.0,
            logical_to_image_scale_y: 1.0,
        };

        let err = match render_canvas_surface(&export) {
            Ok(_) => panic!("short backdrop must fail"),
            Err(err) => err,
        };

        assert!(
            err.to_string().contains("buffer is too small"),
            "unexpected error: {err}"
        );
    }
}
