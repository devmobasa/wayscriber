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
        I::Highlight => draw_icon_highlight,
        I::ClearCanvas => draw_icon_clear,
        I::Undo => draw_icon_undo,
        I::Redo => draw_icon_redo,
        I::Pin => draw_icon_pin,
        I::Unpin => draw_icon_unpin,
        I::Overflow => draw_icon_more,
        I::Minimize => draw_icon_minimize,
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
        I::Tool(T::Marker) => draw_icon_marker,
        I::Tool(T::Highlight) => draw_icon_highlight,
        I::Tool(T::StepMarker) => draw_icon_step_marker,
        I::Tool(T::Eraser) => draw_icon_eraser,
    }
}
