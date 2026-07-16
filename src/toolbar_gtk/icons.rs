//! Icon widgets that reuse the built-in Cairo icon painters.
//!
//! Every glyph the built-in bars draw comes from `crate::toolbar_icons`
//! functions of the shape `fn(&cairo::Context, x, y, size)` that stroke or
//! fill with the context's current source. Wrapping them in a
//! `DrawingArea` whose source is the widget's CSS color keeps the two
//! frontends pixel-identical and lets GTK state (hover/active/disabled)
//! recolor them for free.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::input::Tool;
use crate::toolbar_icons;
use crate::ui::toolbar::model;

pub(super) type IconPainter = toolbar_icons::ToolbarIconPainter;

/// Painter for a tool's semantic icon; mirrors the built-in mapping.
pub(super) fn tool_icon_painter(tool: Tool) -> IconPainter {
    toolbar_icons::top_toolbar_icon_painter(model::TopToolbarIcon::Tool(
        model::semantic_icon_for_tool(tool),
    ))
}

/// Handle to an icon widget whose painter can be swapped when live toolbar
/// state changes the represented action (for example pin/unpin).
#[derive(Clone)]
pub(super) struct IconWidget {
    pub(super) area: gtk4::DrawingArea,
    painter: Rc<Cell<IconPainter>>,
}

impl IconWidget {
    pub(super) fn new(painter: IconPainter, icon_size: f64) -> Self {
        let area = gtk4::DrawingArea::new();
        let size = icon_size.round().max(1.0) as i32;
        area.set_content_width(size);
        area.set_content_height(size);
        area.set_halign(gtk4::Align::Center);
        area.set_valign(gtk4::Align::Center);
        area.set_can_target(false);
        let cell = Rc::new(Cell::new(painter));
        let draw_cell = cell.clone();
        area.set_draw_func(move |area, ctx, width, height| {
            let color = area.color();
            ctx.set_source_rgba(
                color.red() as f64,
                color.green() as f64,
                color.blue() as f64,
                color.alpha() as f64,
            );
            let size = (width.min(height) as f64).max(1.0);
            let x = (width as f64 - size) / 2.0;
            let y = (height as f64 - size) / 2.0;
            (draw_cell.get())(ctx, x, y, size);
        });
        Self {
            area,
            painter: cell,
        }
    }

    pub(super) fn set_painter(&self, painter: IconPainter) {
        // Address equality is only an optimization: merged functions draw
        // identical output, so a false match never skips a needed redraw.
        if !std::ptr::fn_addr_eq(self.painter.get(), painter) {
            self.painter.set(painter);
            self.area.queue_draw();
        }
    }
}
