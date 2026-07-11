//! Shared widget constructors for the GTK toolbars.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;

use super::GtkToolbarFeedback;
use super::icons::{IconPainter, IconWidget};
use crate::draw::Color;
use crate::ui::toolbar::ToolbarEvent;

/// Sender the view hands to every control closure.
pub(super) type FeedbackSender = std::sync::mpsc::Sender<GtkToolbarFeedback>;

pub(super) fn send_event(sender: &FeedbackSender, event: ToolbarEvent) {
    let _ = sender.send(GtkToolbarFeedback::Event(event));
}

/// Fixed-size button so GTK widths match the deterministic layout plan.
pub(super) fn sized_button(width: f64, height: f64) -> gtk4::Button {
    let button = gtk4::Button::new();
    button.set_size_request(width.round() as i32, height.round() as i32);
    button.set_valign(gtk4::Align::Center);
    button.set_focusable(true);
    button
}

pub(super) struct IconButton {
    pub(super) button: gtk4::Button,
    pub(super) icon: IconWidget,
}

pub(super) fn icon_button(
    painter: IconPainter,
    button_size: (f64, f64),
    icon_size: f64,
    tooltip: &str,
) -> IconButton {
    let button = sized_button(button_size.0, button_size.1);
    let icon = IconWidget::new(painter, icon_size);
    button.set_child(Some(&icon.area));
    button.set_tooltip_text(Some(tooltip));
    IconButton { button, icon }
}

pub(super) fn text_button(label: &str, button_size: (f64, f64), tooltip: &str) -> gtk4::Button {
    let button = sized_button(button_size.0, button_size.1);
    button.set_label(label);
    button.set_tooltip_text(Some(tooltip));
    button
}

/// Toggle the CSS class marking the active tool / selected value.
pub(super) fn set_active_class(widget: &impl IsA<gtk4::Widget>, active: bool) {
    let widget = widget.as_ref();
    if active {
        widget.add_css_class("active");
    } else {
        widget.remove_css_class("active");
    }
}

/// A color swatch button: the fill and the selection ring are drawn with
/// Cairo exactly like the built-in bars draw them.
pub(super) struct SwatchButton {
    pub(super) button: gtk4::Button,
    color: Rc<Cell<(f64, f64, f64, f64)>>,
    selected: Rc<Cell<bool>>,
    area: gtk4::DrawingArea,
}

impl SwatchButton {
    pub(super) fn new(color: Color, selected: bool, diameter: f64, tooltip: &str) -> Self {
        let button = sized_button(diameter, diameter);
        button.add_css_class("swatch");
        button.set_tooltip_text(Some(tooltip));
        let color_cell = Rc::new(Cell::new((color.r, color.g, color.b, color.a)));
        let selected_cell = Rc::new(Cell::new(selected));
        let area = gtk4::DrawingArea::new();
        let size = diameter.round() as i32;
        area.set_content_width(size);
        area.set_content_height(size);
        area.set_can_target(false);
        let draw_color = color_cell.clone();
        let draw_selected = selected_cell.clone();
        area.set_draw_func(move |_, ctx, width, height| {
            let size = width.min(height) as f64;
            let center = size / 2.0;
            let (r, g, b, a) = draw_color.get();
            // Fill circle with a subtle outline; selected swatches get the
            // white ring the built-in bars use.
            ctx.arc(
                center,
                center,
                center - 2.0,
                0.0,
                std::f64::consts::PI * 2.0,
            );
            ctx.set_source_rgba(r, g, b, a);
            let _ = ctx.fill_preserve();
            if draw_selected.get() {
                ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
                ctx.set_line_width(2.0);
            } else {
                ctx.set_source_rgba(1.0, 1.0, 1.0, 0.25);
                ctx.set_line_width(1.0);
            }
            let _ = ctx.stroke();
        });
        button.set_child(Some(&area));
        Self {
            button,
            color: color_cell,
            selected: selected_cell,
            area,
        }
    }

    pub(super) fn set_selected(&self, selected: bool) {
        if self.selected.get() != selected {
            self.selected.set(selected);
            self.area.queue_draw();
        }
    }

    pub(super) fn set_color(&self, color: Color) {
        let value = (color.r, color.g, color.b, color.a);
        if self.color.get() != value {
            self.color.set(value);
            self.area.queue_draw();
        }
    }
}
