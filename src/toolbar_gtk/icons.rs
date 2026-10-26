//! Icon widgets that reuse the built-in Cairo icon painters.
//!
//! Every glyph the built-in bars draw comes from `crate::toolbar_icons`
//! functions of the shape `fn(&cairo::Context, x, y, size)` that stroke or
//! fill with the context's current source. Wrapping them in a
//! `DrawingArea` whose source is the widget's CSS color keeps the two
//! frontends pixel-identical and lets GTK state (hover/active/disabled)
//! recolor them for free.

use std::cell::Cell;
use std::ptr;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::toolbar_icons;

pub(super) type IconPainter = toolbar_icons::ToolbarIconPainter;

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
                f64::from(color.red()),
                f64::from(color.green()),
                f64::from(color.blue()),
                f64::from(color.alpha()),
            );
            let size = f64::from(width.min(height)).max(1.0);
            let x = (f64::from(width) - size) / 2.0;
            let y = (f64::from(height) - size) / 2.0;
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
        if !ptr::fn_addr_eq(self.painter.get(), painter) {
            self.painter.set(painter);
            self.area.queue_draw();
        }
    }
}
