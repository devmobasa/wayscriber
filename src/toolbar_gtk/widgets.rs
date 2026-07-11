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

/// Custom slider matching the built-in track + knob (a `DrawingArea` with
/// a drag gesture), so a live backend update never fights an in-flight
/// drag: incoming values are ignored while `dragging` is set.
pub(super) struct SliderRow {
    pub(super) root: gtk4::Box,
    value_label: gtk4::Label,
    area: gtk4::DrawingArea,
    state: Rc<SliderState>,
    format: fn(f64) -> String,
}

pub(super) struct SliderState {
    min: f64,
    max: f64,
    value: Cell<f64>,
    dragging: Cell<bool>,
}

impl SliderRow {
    /// `on_change` fires continuously during a drag with the new value.
    pub(super) fn new(
        scale: f64,
        (min, max): (f64, f64),
        initial: f64,
        format: fn(f64) -> String,
        on_change: impl Fn(f64) + 'static,
    ) -> Self {
        let root = gtk4::Box::new(gtk4::Orientation::Horizontal, (6.0 * scale).round() as i32);
        let state = Rc::new(SliderState {
            min,
            max,
            value: Cell::new(initial.clamp(min, max)),
            dragging: Cell::new(false),
        });

        let area = gtk4::DrawingArea::new();
        area.set_content_height((16.0 * scale).round() as i32);
        area.set_hexpand(true);
        area.set_valign(gtk4::Align::Center);
        let draw_state = state.clone();
        area.set_draw_func(move |_, ctx, width, height| {
            let w = width as f64;
            let h = height as f64;
            let track_h = (h * 0.5).min(8.0);
            let track_y = (h - track_h) / 2.0;
            let radius = track_h / 2.0;
            let t = ((draw_state.value.get() - draw_state.min) / (draw_state.max - draw_state.min))
                .clamp(0.0, 1.0);
            // Track
            rounded_rect_path(ctx, 0.0, track_y, w, track_h, radius);
            ctx.set_source_rgba(0.5, 0.5, 0.6, 0.6);
            let _ = ctx.fill();
            // Filled portion
            rounded_rect_path(ctx, 0.0, track_y, (w * t).max(track_h), track_h, radius);
            ctx.set_source_rgba(0.3, 0.55, 1.0, 0.55);
            let _ = ctx.fill();
            // Knob
            let knob_r = (h / 2.0).min(7.0);
            let knob_x = knob_r + t * (w - knob_r * 2.0);
            ctx.arc(knob_x, h / 2.0, knob_r, 0.0, std::f64::consts::PI * 2.0);
            ctx.set_source_rgba(0.3, 0.55, 1.0, 0.9);
            let _ = ctx.fill();
        });

        let drag = gtk4::GestureDrag::new();
        let drag_state = state.clone();
        let drag_area = area.clone();
        let start_value = Rc::new(Cell::new((0.0f64, 0.0f64)));
        let begin_start = start_value.clone();
        drag.connect_drag_begin(move |gesture, x, _| {
            drag_state.dragging.set(true);
            // Jump the knob to the pressed position, like the built-in track.
            let width = gesture.widget().map(|w| w.width()).unwrap_or(1).max(1) as f64;
            let t = (x / width).clamp(0.0, 1.0);
            let value = drag_state.min + t * (drag_state.max - drag_state.min);
            drag_state.value.set(value);
            begin_start.set((x, value));
            drag_area.queue_draw();
        });
        let update_state = state.clone();
        let update_area = area.clone();
        let update_start = start_value.clone();
        let change = Rc::new(on_change);
        let update_change = change.clone();
        drag.connect_drag_update(move |gesture, dx, _| {
            let width = gesture.widget().map(|w| w.width()).unwrap_or(1).max(1) as f64;
            let (sx, _) = update_start.get();
            let t = ((sx + dx) / width).clamp(0.0, 1.0);
            let value = update_state.min + t * (update_state.max - update_state.min);
            update_state.value.set(value);
            update_area.queue_draw();
            update_change(value);
        });
        let end_state = state.clone();
        let end_change = change.clone();
        drag.connect_drag_end(move |_, _, _| {
            end_state.dragging.set(false);
            end_change(end_state.value.get());
        });
        area.add_controller(drag);

        let value_label = gtk4::Label::new(Some(&format(initial)));
        value_label.set_width_chars(5);
        value_label.set_xalign(1.0);

        root.append(&area);
        root.append(&value_label);
        Self {
            root,
            value_label,
            area,
            state,
            format,
        }
    }

    /// Applies a backend value unless the user is mid-drag.
    pub(super) fn set_value(&self, value: f64) {
        if self.state.dragging.get() {
            return;
        }
        let clamped = value.clamp(self.state.min, self.state.max);
        if (self.state.value.get() - clamped).abs() > f64::EPSILON {
            self.state.value.set(clamped);
            self.area.queue_draw();
        }
        self.value_label.set_text(&(self.format)(clamped));
    }
}

pub(super) fn rounded_rect_path(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    use std::f64::consts::PI;
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    ctx.new_path();
    ctx.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    ctx.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    ctx.arc(x + r, y + h - r, r, PI / 2.0, PI);
    ctx.arc(x + r, y + r, r, PI, 3.0 * PI / 2.0);
    ctx.close_path();
}
