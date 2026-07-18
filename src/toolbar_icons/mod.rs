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
