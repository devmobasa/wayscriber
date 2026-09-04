//! Icon drawing functions for the toolbar UI.
//!
//! All toolbar icons are procedural Cairo paths. The `svg` module name is kept
//! for the tool icon call sites that used to render embedded SVG files.

mod actions;
mod controls;
mod history;
mod security;
pub(crate) mod svg;
mod tools;
mod zoom;

pub use actions::*;
pub use controls::*;
pub use history::*;
pub use security::*;
pub use tools::*;
pub use zoom::*;

pub(crate) type ToolbarIconPainter = fn(&cairo::Context, f64, f64, f64);

/// Paint inputs of the micro-mode chip that vary with live state.
pub(crate) struct MicroChipStyle {
    /// Ring stroke color: the current drawing color.
    pub ring_color: (f64, f64, f64, f64),
    /// Ring stroke width, from `model::micro_ring_width`.
    pub ring_width: f64,
    /// Glyph color (theme icon color for the builtin bars, the resolved
    /// CSS color for GTK).
    pub icon_color: (f64, f64, f64, f64),
    /// Hover highlight behind the glyph.
    pub hovered: bool,
}

/// The 44px micro-mode chip both frontends draw: a round panel-token disc
/// with a hairline edge, a ring stroked in the current color whose width
/// follows stroke thickness, and the active tool's glyph in the middle.
/// Shared here so the two frontends cannot drift visually.
pub(crate) fn draw_micro_chip(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    glyph: ToolbarIconPainter,
    style: &MicroChipStyle,
) {
    use crate::ui::theme::set_color;
    use crate::ui::theme::toolbar::{
        COLOR_ICON_HOVER_BG, COLOR_PANEL_BACKGROUND, COLOR_PANEL_BORDER,
    };
    use std::f64::consts::TAU;

    let cx = x + size / 2.0;
    let cy = y + size / 2.0;
    let radius = size / 2.0 - 1.0;

    // Round panel disc + hairline edge (the chip's "pill").
    set_color(ctx, COLOR_PANEL_BACKGROUND);
    ctx.arc(cx, cy, radius, 0.0, TAU);
    let _ = ctx.fill();
    if style.hovered {
        set_color(ctx, COLOR_ICON_HOVER_BG);
        ctx.arc(cx, cy, radius, 0.0, TAU);
        let _ = ctx.fill();
    }
    set_color(ctx, COLOR_PANEL_BORDER);
    ctx.set_line_width(1.0);
    ctx.arc(cx, cy, radius - 0.5, 0.0, TAU);
    let _ = ctx.stroke();

    // Current-color ring, inset so the widest ring stays inside the disc.
    let ring_radius = radius - 1.5 - style.ring_width / 2.0;
    ctx.set_source_rgba(
        style.ring_color.0,
        style.ring_color.1,
        style.ring_color.2,
        style.ring_color.3,
    );
    ctx.set_line_width(style.ring_width);
    ctx.arc(cx, cy, ring_radius.max(1.0), 0.0, TAU);
    let _ = ctx.stroke();

    // Active tool glyph, sized to sit inside the ring.
    let icon_size = (ring_radius.max(1.0) * 2.0 - style.ring_width - 6.0).max(8.0);
    ctx.set_source_rgba(
        style.icon_color.0,
        style.icon_color.1,
        style.icon_color.2,
        style.icon_color.3,
    );
    glyph(ctx, cx - icon_size / 2.0, cy - icon_size / 2.0, icon_size);
}

pub(crate) fn top_toolbar_icon_painter(
    icon: crate::ui::toolbar::model::TopToolbarIcon,
) -> ToolbarIconPainter {
    use crate::ui::toolbar::model::{SemanticToolIcon as T, TopToolbarIcon as I};

    match icon {
        I::Restore => draw_icon_restore,
        I::Drag => draw_icon_drag,
        I::ShapePicker => draw_icon_shape_picker,
        I::Text => draw_icon_text,
        I::StickyNote => draw_icon_note,
        I::Screenshot => draw_icon_screenshot,
        I::Ocr => draw_icon_ocr,
        I::Highlight => draw_icon_highlight,
        I::ClearCanvas => draw_icon_clear,
        I::Undo => draw_icon_undo,
        I::Redo => draw_icon_redo,
        I::Pin => draw_icon_pin,
        I::Unpin => draw_icon_unpin,
        I::Overflow => draw_icon_more,
        I::Minimize => draw_icon_minimize,
        I::Canvas => draw_icon_layers,
        I::Session => draw_icon_session,
        I::Settings => draw_icon_sliders,
        I::About => draw_icon_info,
        I::LayoutSimple => draw_icon_layout_simple,
        I::LayoutRegular => draw_icon_layout_regular,
        I::LayoutAdvanced => draw_icon_layout_advanced,
        I::Tool(T::Select) => draw_icon_select,
        I::Tool(T::Pen) => draw_icon_pen,
        I::Tool(T::Line) => draw_icon_line,
        I::Tool(T::Rect) => draw_icon_rect,
        I::Tool(T::Circle) => draw_icon_circle,
        I::Tool(T::Triangle) => draw_icon_triangle,
        I::Tool(T::Parallelogram) => draw_icon_parallelogram,
        I::Tool(T::Rhombus) => draw_icon_rhombus,
        I::Tool(T::Polygon) => draw_icon_polygon,
        I::Tool(T::FreeformPolygon) => draw_icon_freeform_polygon,
        I::Tool(T::Arrow) => draw_icon_arrow,
        I::Tool(T::Blur) => draw_icon_blur,
        I::Tool(T::Spotlight) => draw_icon_spotlight,
        I::Tool(T::Marker) => draw_icon_marker,
        I::Tool(T::Highlight) => draw_icon_highlight,
        I::Tool(T::StepMarker) => draw_icon_step_marker,
        I::Tool(T::Eraser) => draw_icon_eraser,
    }
}

#[cfg(test)]
mod painter_tests {
    use super::*;
    use cairo::{Context, Format, ImageSurface};

    type IconPainter = fn(&Context, f64, f64, f64);

    /// Sizes the chrome actually asks for, including the 18px compact bar
    /// where thin strokes are most likely to round away to nothing.
    const SIZES: [i32; 5] = [18, 20, 22, 24, 28];

    /// Every public painter. `svg.rs` covers the newer family through its own
    /// `render_*` entry points; this covers the shipped surface callers use,
    /// including the older proportional-style painters that had no coverage.
    const PAINTERS: [(&str, IconPainter); 64] = [
        ("arrow", draw_icon_arrow),
        ("blur", draw_icon_blur),
        ("board", draw_icon_board),
        ("chevron_down", draw_icon_chevron_down),
        ("chevron_left", draw_icon_chevron_left),
        ("chevron_right", draw_icon_chevron_right),
        ("circle", draw_icon_circle),
        ("clear", draw_icon_clear),
        ("close", draw_icon_close),
        ("copy", draw_icon_copy),
        ("delay", draw_icon_delay),
        ("drag", draw_icon_drag),
        ("eraser", draw_icon_eraser),
        ("eyedropper", draw_icon_eyedropper),
        ("file", draw_icon_file),
        ("fill", draw_icon_fill),
        ("freeform_polygon", draw_icon_freeform_polygon),
        ("freeze", draw_icon_freeze),
        ("grid", draw_icon_grid),
        ("highlight", draw_icon_highlight),
        ("highlight_ring", draw_icon_highlight_ring),
        ("info", draw_icon_info),
        ("layers", draw_icon_layers),
        ("layout_advanced", draw_icon_layout_advanced),
        ("layout_regular", draw_icon_layout_regular),
        ("layout_simple", draw_icon_layout_simple),
        ("line", draw_icon_line),
        ("lock", draw_icon_lock),
        ("marker", draw_icon_marker),
        ("minimize", draw_icon_minimize),
        ("minus", draw_icon_minus),
        ("more", draw_icon_more),
        ("note", draw_icon_note),
        ("ocr", draw_icon_ocr),
        ("parallelogram", draw_icon_parallelogram),
        ("paste", draw_icon_paste),
        ("pen", draw_icon_pen),
        ("pencil", draw_icon_pencil),
        ("pin", draw_icon_pin),
        ("plus", draw_icon_plus),
        ("polygon", draw_icon_polygon),
        ("rect", draw_icon_rect),
        ("refresh", draw_icon_refresh),
        ("restore", draw_icon_restore),
        ("rhombus", draw_icon_rhombus),
        ("save", draw_icon_save),
        ("screenshot", draw_icon_screenshot),
        ("search", draw_icon_search),
        ("select", draw_icon_select),
        ("session", draw_icon_session),
        ("settings", draw_icon_settings),
        ("shape_picker", draw_icon_shape_picker),
        ("sliders", draw_icon_sliders),
        ("spotlight", draw_icon_spotlight),
        ("step_marker", draw_icon_step_marker),
        ("text", draw_icon_text),
        ("triangle", draw_icon_triangle),
        ("unfreeze", draw_icon_unfreeze),
        ("unlock", draw_icon_unlock),
        ("unpin", draw_icon_unpin),
        ("visibility", draw_icon_visibility),
        ("zoom_in", draw_icon_zoom_in),
        ("zoom_out", draw_icon_zoom_out),
        ("zoom_reset", draw_icon_zoom_reset),
    ];

    #[test]
    fn every_public_icon_paints_something_at_every_chrome_size() {
        for (name, paint) in PAINTERS {
            for size in SIZES {
                let surface = ImageSurface::create(Format::ARgb32, size, size).expect("surface");
                let ctx = Context::new(&surface).expect("context");
                ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                paint(&ctx, 0.0, 0.0, f64::from(size));
                surface.flush();
                let mut painted = false;
                surface
                    .with_data(|pixels| {
                        painted = pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0);
                    })
                    .expect("surface data");
                assert!(painted, "{name} painted nothing at {size}px");
            }
        }
    }
}
