use super::layout::normalized_rect;
use super::types::{RegionCutDragVisual, RegionCutPreviewVisual};
use crate::capture::CutAxis;

pub(super) fn paint_cut_preview(ctx: &cairo::Context, preview: RegionCutPreviewVisual<'_>) {
    let pixels = preview.pixels;
    let Ok(width) = i32::try_from(pixels.width()) else {
        return;
    };
    let Ok(height) = i32::try_from(pixels.height()) else {
        return;
    };
    if width <= 0 || height <= 0 {
        return;
    }
    let (x, y, display_width, display_height) = normalized_rect(preview.display);
    if display_width <= 0.0 || display_height <= 0.0 {
        return;
    }
    // SAFETY: Cairo borrows `pixels.data` for this surface. The buffer is
    // owned by the Review preview and stays alive until the surface is
    // dropped at the end of this function. The API wants `*mut u8` even
    // though this path only reads pixels; we never write through the
    // pointer, and no other alias mutates the buffer while Cairo holds it.
    let surface = unsafe {
        cairo::ImageSurface::create_for_data_unsafe(
            pixels.data().as_ptr() as *mut u8,
            cairo::Format::ARgb32,
            width,
            height,
            pixels.stride(),
        )
    };
    let Ok(surface) = surface else {
        return;
    };
    let _ = ctx.save();
    ctx.rectangle(x, y, display_width, display_height);
    ctx.clip();
    ctx.translate(x, y);
    ctx.scale(
        display_width / f64::from(pixels.width()),
        display_height / f64::from(pixels.height()),
    );
    // Place the surface in the translated/scaled user space, matching the
    // frozen-backdrop path: the CTM maps one source pixel onto one displayed
    // output pixel, and nearest-neighbor keeps cut seams crisp.
    if ctx.set_source_surface(&surface, 0.0, 0.0).is_ok() {
        ctx.source().set_filter(cairo::Filter::Nearest);
        ctx.source().set_extend(cairo::Extend::None);
        let _ = ctx.paint();
    }
    let _ = ctx.restore();
}

pub(super) fn draw_cut_drag(ctx: &cairo::Context, drag: RegionCutDragVisual) {
    let (x, y, width, height) = normalized_rect(drag.band);
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    ctx.set_source_rgba(0.05, 0.08, 0.14, 0.48);
    ctx.rectangle(x, y, width, height);
    let _ = ctx.fill();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.92);
    ctx.set_line_width(1.0);
    match drag.axis {
        CutAxis::Columns => {
            ctx.move_to(x + 0.5, y);
            ctx.line_to(x + 0.5, y + height);
            ctx.move_to(x + width - 0.5, y);
            ctx.line_to(x + width - 0.5, y + height);
        }
        CutAxis::Rows => {
            ctx.move_to(x, y + 0.5);
            ctx.line_to(x + width, y + 0.5);
            ctx.move_to(x, y + height - 0.5);
            ctx.line_to(x + width, y + height - 0.5);
        }
    }
    let _ = ctx.stroke();
}
