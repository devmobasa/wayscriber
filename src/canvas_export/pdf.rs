use crate::capture::CaptureError;
use crate::config::{PdfExportConfig, PdfFitMode, PdfOrientation, PdfPageSize};
use crate::draw::{RenderCaches, RenderCtx};

use super::page::{
    CanvasExportBackdropSnapshot, CanvasExportRect, CanvasPageExportSnapshot, ExportBackdrop,
    draw_canvas_page_region, frame_has_magnified_spotlight, paint_pdf_page_background,
    validate_spotlight_magnifier_source,
};
use super::pdf_labels::render_pdf_label;

const A4_WIDTH: f64 = 595.0;
const A4_HEIGHT: f64 = 842.0;
const LETTER_WIDTH: f64 = 612.0;
const LETTER_HEIGHT: f64 = 792.0;

#[derive(Debug, Clone)]
pub struct BoardPdfExportSnapshot {
    pub pages: Vec<PdfPageExportSnapshot>,
    pub labels: crate::config::PdfLabelConfig,
}

#[derive(Debug, Clone)]
pub struct PdfPageExportSnapshot {
    pub page: CanvasPageExportSnapshot,
    pub metadata: PdfPageMetadata,
    pub layout: PdfPageLayout,
}

impl BoardPdfExportSnapshot {
    /// Validates every page before a document worker is submitted and names
    /// the exact board/page whose retained source is incomplete.
    ///
    /// A transparent PDF page does have a way to gain a source, unlike a canvas
    /// PNG: `[export.pdf] transparent_background = "desktop"` captures the
    /// desktop behind the overlay for exactly these pages.
    pub fn validate_spotlight_sources(&self) -> Result<(), CaptureError> {
        for page in &self.pages {
            let subject = format!(
                "Board '{}', {}",
                page.metadata.board_name, page.metadata.page_name_label
            );
            validate_spotlight_magnifier_source(
                &page.page.frame,
                &page.page.backdrop,
                &subject,
                "give the page a solid board background, or set [export.pdf] transparent_background = \"desktop\" to capture one",
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfPageMetadata {
    pub app_board_index: usize,
    pub app_board_count: usize,
    pub export_board_index: usize,
    pub export_board_count: usize,
    pub board_page_index: usize,
    pub board_page_count: usize,
    pub document_page_index: usize,
    pub document_page_count: usize,
    pub board_name: String,
    pub page_name: Option<String>,
    pub app_board_label: String,
    pub app_board_count_label: String,
    pub export_board_label: String,
    pub export_board_count_label: String,
    pub board_page_label: String,
    pub board_page_count_label: String,
    pub document_page_label: String,
    pub document_page_count_label: String,
    pub page_name_label: String,
}

impl PdfPageMetadata {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app_board_index: usize,
        app_board_count: usize,
        export_board_index: usize,
        export_board_count: usize,
        board_page_index: usize,
        board_page_count: usize,
        document_page_index: usize,
        document_page_count: usize,
        board_name: String,
        page_name: Option<String>,
    ) -> Self {
        let page_name_label = page_name
            .as_ref()
            .filter(|name| !name.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| format!("Page {}", board_page_index + 1));
        Self {
            app_board_index,
            app_board_count,
            export_board_index,
            export_board_count,
            board_page_index,
            board_page_count,
            document_page_index,
            document_page_count,
            board_name,
            page_name,
            app_board_label: (app_board_index + 1).to_string(),
            app_board_count_label: app_board_count.to_string(),
            export_board_label: (export_board_index + 1).to_string(),
            export_board_count_label: export_board_count.to_string(),
            board_page_label: (board_page_index + 1).to_string(),
            board_page_count_label: board_page_count.to_string(),
            document_page_label: (document_page_index + 1).to_string(),
            document_page_count_label: document_page_count.to_string(),
            page_name_label,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfPageLayout {
    pub page_width: f64,
    pub page_height: f64,
    pub source_rect: CanvasExportRect,
    pub destination_rect: CanvasExportRect,
}

pub fn render_board_pdf(snapshot: &BoardPdfExportSnapshot) -> Result<Vec<u8>, CaptureError> {
    if snapshot.pages.is_empty() {
        return Err(CaptureError::ImageError(
            "Board PDF export requires at least one page".to_string(),
        ));
    }
    snapshot.validate_spotlight_sources()?;

    let first = snapshot.pages[0].layout;
    validate_page_size(first.page_width, first.page_height)?;
    let surface =
        cairo::PdfSurface::for_stream(first.page_width, first.page_height, Vec::<u8>::new())
            .map_err(|err| {
                CaptureError::ImageError(format!("Failed to create PDF surface: {err}"))
            })?;
    let ctx = cairo::Context::new(&surface)
        .map_err(|err| CaptureError::ImageError(format!("Failed to create PDF context: {err}")))?;

    let mut caches = RenderCaches::default();
    for page in &snapshot.pages {
        let layout = page.layout;
        validate_page_size(layout.page_width, layout.page_height)?;
        surface
            .set_size(layout.page_width, layout.page_height)
            .map_err(|err| {
                CaptureError::ImageError(format!("Failed to set PDF page size: {err}"))
            })?;
        paint_pdf_page_background(&ctx, &page.page, layout.page_width, layout.page_height);
        let backdrop = ExportBackdrop::new(&page.page.backdrop)?;
        if frame_has_magnified_spotlight(&page.page.frame) {
            render_magnified_page_raster(
                &mut RenderCtx::new(&ctx, &mut caches),
                &page.page,
                &backdrop,
                layout,
            )?;
        } else {
            let paint_content_backdrop = matches!(
                page.page.backdrop,
                CanvasExportBackdropSnapshot::PersistedImage { .. }
            );
            draw_canvas_page_region(
                &mut RenderCtx::new(&ctx, &mut caches),
                &page.page,
                &backdrop,
                layout.source_rect,
                layout.destination_rect,
                paint_content_backdrop,
                // A vector page has no raster size to fall back on, and it is
                // only taken when the page holds no magnified Spotlight.
                None,
            )?;
        }
        render_pdf_label(
            &ctx,
            &snapshot.labels,
            &page.metadata,
            layout.page_width,
            layout.page_height,
        );
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

fn render_magnified_page_raster(
    render: &mut RenderCtx<'_, '_>,
    page: &CanvasPageExportSnapshot,
    backdrop: &ExportBackdrop,
    layout: PdfPageLayout,
) -> Result<(), CaptureError> {
    let pdf_ctx = render.cairo;
    let (width, height) = checked_raster_dimensions(layout.source_rect)?;
    let surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).map_err(|err| {
            CaptureError::ImageError(format!("Failed to create magnified PDF page: {err}"))
        })?;
    let raster_ctx = cairo::Context::new(&surface).map_err(|err| {
        CaptureError::ImageError(format!(
            "Failed to create magnified PDF page context: {err}"
        ))
    })?;
    draw_canvas_page_region(
        &mut RenderCtx::new(&raster_ctx, render.caches),
        page,
        backdrop,
        layout.source_rect,
        CanvasExportRect {
            x: 0.0,
            y: 0.0,
            width: f64::from(width),
            height: f64::from(height),
        },
        true,
        Some((width as u32, height as u32)),
    )?;
    drop(raster_ctx);

    crate::draw::with_saved_state(pdf_ctx, || {
        pdf_ctx.rectangle(
            layout.destination_rect.x,
            layout.destination_rect.y,
            layout.destination_rect.width,
            layout.destination_rect.height,
        );
        pdf_ctx.clip();
        pdf_ctx.translate(layout.destination_rect.x, layout.destination_rect.y);
        pdf_ctx.scale(
            layout.destination_rect.width / f64::from(width),
            layout.destination_rect.height / f64::from(height),
        );
        pdf_ctx.set_source_surface(&surface, 0.0, 0.0)?;
        pdf_ctx.paint()
    })
    .map_err(|err| CaptureError::ImageError(format!("Failed to paint magnified PDF page: {err}")))
}

fn checked_raster_dimensions(source: CanvasExportRect) -> Result<(i32, i32), CaptureError> {
    const MAX_RASTER_BYTES: u64 = 512 * 1024 * 1024;
    let width = source.width.ceil();
    let height = source.height.ceil();
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > f64::from(i32::MAX)
        || height > f64::from(i32::MAX)
    {
        return Err(CaptureError::ImageError(
            "Magnified PDF page dimensions are invalid or too large".to_string(),
        ));
    }
    let width = width as i32;
    let height = height as i32;
    let bytes = (width as u64)
        .checked_mul(height as u64)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| {
            CaptureError::ImageError("Magnified PDF page allocation overflow".to_string())
        })?;
    if bytes > MAX_RASTER_BYTES {
        return Err(CaptureError::ImageError(format!(
            "Magnified PDF page requires {bytes} bytes, above the 512 MiB safety limit"
        )));
    }
    Ok((width, height))
}

pub fn resolve_pdf_page_layout(
    viewport_width: u32,
    viewport_height: u32,
    origin_x: i32,
    origin_y: i32,
    content_bounds: Option<CanvasExportRect>,
    config: &PdfExportConfig,
) -> Result<PdfPageLayout, CaptureError> {
    if viewport_width == 0 || viewport_height == 0 {
        return Err(CaptureError::ImageError(
            "Board PDF export requires a configured non-empty surface".to_string(),
        ));
    }

    let viewport_source = CanvasExportRect {
        x: origin_x as f64,
        y: origin_y as f64,
        width: viewport_width as f64,
        height: viewport_height as f64,
    };

    let source_rect = match config.fit {
        PdfFitMode::FitContentToPage => content_bounds
            .map(|bounds| pad_source_rect(bounds, config.content_source_padding))
            .unwrap_or(viewport_source),
        PdfFitMode::FitViewportToPage | PdfFitMode::Viewport => viewport_source,
    };
    let (page_width, page_height) =
        resolve_page_size(viewport_width, viewport_height, source_rect, config);
    let destination_rect = match config.fit {
        PdfFitMode::Viewport => CanvasExportRect {
            x: 0.0,
            y: 0.0,
            width: source_rect.width,
            height: source_rect.height,
        },
        PdfFitMode::FitViewportToPage | PdfFitMode::FitContentToPage => {
            fit_rect(source_rect, page_width, page_height)
        }
    };

    Ok(PdfPageLayout {
        page_width,
        page_height,
        source_rect,
        destination_rect,
    })
}

fn pad_source_rect(source: CanvasExportRect, padding: f64) -> CanvasExportRect {
    let padding = if padding.is_finite() && padding > 0.0 {
        padding
    } else {
        0.0
    };
    CanvasExportRect {
        x: source.x - padding,
        y: source.y - padding,
        width: source.width + padding * 2.0,
        height: source.height + padding * 2.0,
    }
}

fn validate_page_size(width: f64, height: f64) -> Result<(), CaptureError> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(CaptureError::ImageError(
            "Board PDF export requires a configured non-empty surface".to_string(),
        ));
    }
    Ok(())
}

fn resolve_page_size(
    viewport_width: u32,
    viewport_height: u32,
    source_rect: CanvasExportRect,
    config: &PdfExportConfig,
) -> (f64, f64) {
    let (base_width, base_height) = match config.page_size {
        PdfPageSize::Viewport => (viewport_width as f64, viewport_height as f64),
        PdfPageSize::A4 => (A4_WIDTH, A4_HEIGHT),
        PdfPageSize::Letter => (LETTER_WIDTH, LETTER_HEIGHT),
        PdfPageSize::Custom => (config.custom_width, config.custom_height),
    };

    let natural_landscape = base_width >= base_height;
    let requested_landscape = match config.orientation {
        PdfOrientation::Auto => {
            if config.page_size == PdfPageSize::Viewport {
                natural_landscape
            } else {
                source_rect.width >= source_rect.height
            }
        }
        PdfOrientation::Portrait => false,
        PdfOrientation::Landscape => true,
    };

    if requested_landscape == natural_landscape {
        (base_width, base_height)
    } else {
        (base_height, base_width)
    }
}

fn fit_rect(source: CanvasExportRect, page_width: f64, page_height: f64) -> CanvasExportRect {
    let scale = (page_width / source.width).min(page_height / source.height);
    let width = source.width * scale;
    let height = source.height * scale;
    CanvasExportRect {
        x: (page_width - width) / 2.0,
        y: (page_height - height) / 2.0,
        width,
        height,
    }
}

#[cfg(test)]
mod tests;
