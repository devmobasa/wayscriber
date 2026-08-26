use std::sync::Arc;

use crate::capture::CaptureError;
use crate::draw::{
    BlurRectParams, Color, EraserReplayContext, Frame, Shape, SpotlightMagnifierOutcome,
    SpotlightMagnifierScratch, SpotlightMagnifierSource, SpotlightPass, render_blur_rect,
    render_eraser_stroke, render_shape_over, render_spotlight_magnification_pass,
    render_spotlight_pass, spotlight_regions_for_frame,
};
use crate::screen_pixels::ScreenImage;

#[derive(Debug, Clone)]
pub struct CanvasPageExportSnapshot {
    pub frame: Frame,
    pub backdrop: CanvasExportBackdropSnapshot,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub origin_x: i32,
    pub origin_y: i32,
    /// Dim/feather settings for the spotlight pass, mirroring the live overlay.
    pub spotlight: SpotlightPassSnapshot,
}

/// Spotlight appearance carried into an export.
#[derive(Debug, Clone, Copy)]
pub struct SpotlightPassSnapshot {
    pub dim_opacity: f64,
    pub feather: f64,
}

impl Default for SpotlightPassSnapshot {
    fn default() -> Self {
        Self {
            dim_opacity: 0.6,
            feather: 0.35,
        }
    }
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

impl CanvasExportBackdropSnapshot {
    /// Loupe availability for this backdrop, answered without decoding it.
    ///
    /// Mirrors what [`ExportBackdrop::new`] will produce for the same variant,
    /// which is what lets the main-thread preflight refuse a page before a
    /// render worker is ever submitted.
    pub(crate) fn magnifier_source(&self) -> SpotlightMagnifierSource {
        match self {
            Self::Transparent => SpotlightMagnifierSource::from_backdrop(None, false),
            Self::Solid(_) => SpotlightMagnifierSource::from_backdrop(None, true),
            Self::PersistedImage { .. } => SpotlightMagnifierSource::immutable_raster(),
        }
    }
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
    let target_size = (
        (f64::from(page.viewport_width) * output_scale).ceil() as u32,
        (f64::from(page.viewport_height) * output_scale).ceil() as u32,
    );
    let rendered = draw_canvas_page_region(
        ctx,
        page,
        &backdrop,
        source,
        destination,
        true,
        Some(target_size),
    );
    let _ = ctx.restore();
    rendered
}

pub(crate) fn draw_canvas_page_region(
    ctx: &cairo::Context,
    page: &CanvasPageExportSnapshot,
    backdrop: &ExportBackdrop,
    source: CanvasExportRect,
    destination: CanvasExportRect,
    paint_backdrop: bool,
    fallback_target_size: Option<(u32, u32)>,
) -> Result<(), CaptureError> {
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
    let rendered =
        draw_canvas_page_contents(ctx, page, backdrop, paint_backdrop, fallback_target_size);
    let _ = ctx.restore();
    rendered
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
    logical_image_origin_x: f64,
    logical_image_origin_y: f64,
    // Keeps zero-copy region pixels alive until Cairo's borrowed surface and
    // pattern have both been dropped.
    _region_source: Option<Arc<ScreenImage>>,
}

impl ExportBackdrop {
    /// Relative luminance of a solid page colour, when the backdrop is one.
    ///
    /// `None` for transparent and image backdrops: a transparent page has no
    /// colour to report, and an image's brightness varies across the page, so
    /// one number for the whole of it would be a guess.
    pub(crate) fn solid_luminance(&self) -> Option<f64> {
        self.bg_color
            .map(|color| crate::draw::perceived_luminance(color.r, color.g, color.b))
    }

    pub(crate) fn new(snapshot: &CanvasExportBackdropSnapshot) -> Result<Self, CaptureError> {
        match snapshot {
            CanvasExportBackdropSnapshot::Transparent => Ok(Self {
                surface: None,
                pattern: None,
                bg_color: None,
                logical_to_image_scale_x: 1.0,
                logical_to_image_scale_y: 1.0,
                logical_image_origin_x: 0.0,
                logical_image_origin_y: 0.0,
                _region_source: None,
            }),
            CanvasExportBackdropSnapshot::Solid(color) => Ok(Self {
                surface: None,
                pattern: None,
                bg_color: Some(*color),
                logical_to_image_scale_x: 1.0,
                logical_to_image_scale_y: 1.0,
                logical_image_origin_x: 0.0,
                logical_image_origin_y: 0.0,
                _region_source: None,
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
                    logical_image_origin_x: 0.0,
                    logical_image_origin_y: 0.0,
                    _region_source: None,
                })
            }
        }
    }

    pub(crate) fn from_region_source(
        image: Arc<ScreenImage>,
        logical_bounds: CanvasExportRect,
    ) -> Result<Self, CaptureError> {
        let width = i32::try_from(image.width).map_err(|_| {
            CaptureError::ImageError("Region backdrop width is too large".to_string())
        })?;
        let height = i32::try_from(image.height).map_err(|_| {
            CaptureError::ImageError("Region backdrop height is too large".to_string())
        })?;
        validate_persisted_image_backdrop(image.data.len(), width, height, image.stride)?;
        let scale_x = f64::from(width) / logical_bounds.width;
        let scale_y = f64::from(height) / logical_bounds.height;
        if !scale_x.is_finite() || !scale_y.is_finite() || scale_x <= 0.0 || scale_y <= 0.0 {
            return Err(CaptureError::ImageError(
                "Region backdrop has an invalid canvas mapping".to_string(),
            ));
        }
        // SAFETY: ScreenImage owns at least every validated row and remains
        // alive in `_region_source` until after this private Cairo source
        // surface and its pattern are dropped. This backdrop is read-only:
        // it is used only as a paint/blur/eraser replay source.
        let surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                image.data.as_ptr() as *mut u8,
                cairo::Format::ARgb32,
                width,
                height,
                image.stride,
            )
        }
        .map_err(|err| {
            CaptureError::ImageError(format!("Failed to create region backdrop: {err}"))
        })?;
        let pattern = cairo::SurfacePattern::create(&surface);
        pattern.set_extend(cairo::Extend::Pad);
        pattern.set_matrix(cairo::Matrix::new(
            scale_x,
            0.0,
            0.0,
            scale_y,
            -logical_bounds.x * scale_x,
            -logical_bounds.y * scale_y,
        ));
        Ok(Self {
            surface: Some(surface),
            pattern: Some(pattern),
            bg_color: None,
            logical_to_image_scale_x: scale_x,
            logical_to_image_scale_y: scale_y,
            logical_image_origin_x: logical_bounds.x,
            logical_image_origin_y: logical_bounds.y,
            _region_source: Some(image),
        })
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
        ctx.translate(self.logical_image_origin_x, self.logical_image_origin_y);
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
            logical_image_origin_x: self.logical_image_origin_x,
            logical_image_origin_y: self.logical_image_origin_y,
        }
    }

    /// An export backdrop is an immutable snapshot: its pixels cannot be
    /// recaptured or invalidated part-way through the render, so a present
    /// raster surface needs no generation of its own.
    fn magnifier_source(&self) -> SpotlightMagnifierSource {
        SpotlightMagnifierSource::from_backdrop(
            self.surface
                .is_some()
                .then_some(crate::draw::IMMUTABLE_RASTER_SOURCE_TOKEN),
            self.bg_color.is_some(),
        )
    }
}

fn draw_canvas_page_contents(
    ctx: &cairo::Context,
    page: &CanvasPageExportSnapshot,
    backdrop: &ExportBackdrop,
    paint_backdrop: bool,
    fallback_target_size: Option<(u32, u32)>,
) -> Result<(), CaptureError> {
    if paint_backdrop {
        backdrop.paint(ctx);
    }
    let replay_ctx = backdrop.replay_context();
    // What text should contrast with when the target cannot be read back. A PDF
    // page is a vector surface with no pixels to probe, so without this a board
    // exported to PDF would pick a different halo from the same board on screen.
    // Raster exports ignore it and probe, which also sees the shapes underneath.
    let known_background_luminance = backdrop.solid_luminance();

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
                style,
            } => render_blur_rect(
                ctx,
                BlurRectParams {
                    x: *x,
                    y: *y,
                    w: *w,
                    h: *h,
                    strength: *strength,
                    style: *style,
                    cacheable: false,
                },
                &replay_ctx,
            ),
            other => render_shape_over(ctx, other, known_background_luminance),
        }
    }

    // After the shapes, matching the live canvas: eraser strokes clear their path
    // and replay the backdrop, so a dim layer painted earlier would be punched
    // away. Runs regardless of `paint_backdrop` — a PDF page with a solid
    // backdrop is already filled page-wide but still needs dimming.
    let regions = spotlight_regions_for_frame(&page.frame);
    let source = backdrop.magnifier_source();
    let mut scratch = SpotlightMagnifierScratch::default();
    match render_spotlight_magnification_pass(
        ctx,
        &regions,
        page.spotlight.feather,
        source,
        fallback_target_size,
        &mut scratch,
    )
    .map_err(|err| {
        CaptureError::ImageError(format!("Failed to render Spotlight magnification: {err}"))
    })? {
        SpotlightMagnifierOutcome::SourceUnavailable => {
            return Err(CaptureError::ImageError(
                "Spotlight magnification needs complete backdrop pixels; Freeze screen to magnify before exporting"
                    .to_string(),
            ));
        }
        // An export must never silently save a 1x result, so a refused
        // allocation fails the render instead of degrading like the live canvas.
        SpotlightMagnifierOutcome::AllocationFailed => {
            return Err(CaptureError::ImageError(
                "Spotlight magnification could not allocate its render buffer".to_string(),
            ));
        }
        SpotlightMagnifierOutcome::NotNeeded | SpotlightMagnifierOutcome::Rendered(_) => {}
    }

    render_spotlight_pass(
        ctx,
        &regions,
        SpotlightPass {
            dim_opacity: page.spotlight.dim_opacity,
            feather: page.spotlight.feather,
        },
    );
    Ok(())
}

pub(crate) fn frame_has_magnified_spotlight(frame: &Frame) -> bool {
    spotlight_regions_for_frame(frame)
        .iter()
        .any(|region| crate::draw::spotlight_magnification_is_active(region.magnification))
}

/// Refuses an export whose own backdrop cannot feed the loupes on its frame.
///
/// Availability comes from [`CanvasExportBackdropSnapshot::magnifier_source`],
/// the same rule the renderer applies, so the preflight and the render cannot
/// disagree about which page is exportable.
///
/// This asks the *snapshot's* backdrop. Region export deliberately renders a
/// `Transparent` snapshot against a backdrop built from the captured region
/// (see [`ExportBackdrop::from_region_source`]), so it has a complete source
/// this function cannot see and must not call it.
///
/// `recovery` is the caller's own way out, because the two export paths do not
/// share one. Freezing does not help a canvas PNG — that export excludes
/// frozen and zoom desktop pixels by design — so each caller states the step
/// that actually works for it rather than offering generic advice.
pub(crate) fn validate_spotlight_magnifier_source(
    frame: &Frame,
    backdrop: &CanvasExportBackdropSnapshot,
    subject: &str,
    recovery: &str,
) -> Result<(), CaptureError> {
    if frame_has_magnified_spotlight(frame) && !backdrop.magnifier_source().is_complete() {
        return Err(CaptureError::ImageError(format!(
            "{subject} contains a magnified Spotlight but has no complete pixel source; {recovery}"
        )));
    }
    Ok(())
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

    #[test]
    fn region_backdrop_retains_the_exact_shared_image_without_copying_pixels() {
        let image = Arc::new(ScreenImage {
            width: 2,
            height: 2,
            stride: 8,
            data: vec![0xFF; 16],
        });
        let pixels = image.data.as_ptr();
        let backdrop = ExportBackdrop::from_region_source(
            Arc::clone(&image),
            CanvasExportRect::new(10.0, 20.0, 2.0, 2.0).unwrap(),
        )
        .expect("shared region backdrop");

        let retained = backdrop
            ._region_source
            .as_ref()
            .expect("region source is retained");
        assert!(Arc::ptr_eq(retained, &image));
        assert_eq!(retained.data.as_ptr(), pixels);
    }
}

#[cfg(test)]
mod backdrop_luminance_tests {
    use super::*;

    #[test]
    fn a_solid_page_reports_its_own_brightness_for_text_to_contrast_with() {
        let white = ExportBackdrop::new(&CanvasExportBackdropSnapshot::Solid(Color::new(
            1.0, 1.0, 1.0, 1.0,
        )))
        .expect("backdrop");
        let black = ExportBackdrop::new(&CanvasExportBackdropSnapshot::Solid(Color::new(
            0.0, 0.0, 0.0, 1.0,
        )))
        .expect("backdrop");

        assert!(white.solid_luminance().expect("known") > 0.9);
        assert!(black.solid_luminance().expect("known") < 0.1);
    }

    #[test]
    fn a_transparent_page_has_no_colour_to_report() {
        let backdrop =
            ExportBackdrop::new(&CanvasExportBackdropSnapshot::Transparent).expect("backdrop");

        assert_eq!(backdrop.solid_luminance(), None);
    }

    #[test]
    fn a_whiteboard_pdf_and_a_whiteboard_on_screen_choose_the_same_halo() {
        // The screen probes and gets ~1.0; the PDF page cannot be probed and
        // falls back to this. Both must reach the same decision, or an exported
        // board looks different from the board it was exported from.
        let whiteboard = ExportBackdrop::new(&CanvasExportBackdropSnapshot::Solid(Color::new(
            1.0, 1.0, 1.0, 1.0,
        )))
        .expect("backdrop");
        let red = Color::new(0.96, 0.2, 0.25, 1.0);

        let on_screen = crate::draw::text_outline_color(red, Some(1.0));
        let in_pdf = crate::draw::text_outline_color(red, whiteboard.solid_luminance());

        assert_eq!(on_screen, in_pdf);
        assert!(in_pdf.r < 0.5, "and it is the dark halo");
    }
}
