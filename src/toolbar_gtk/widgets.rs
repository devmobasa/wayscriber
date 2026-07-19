//! Shared widget constructors for the GTK toolbars.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4_layer_shell::{KeyboardMode, LayerShell};

use super::GtkToolbarFeedback;
use super::bridge::FeedbackPublisher;
use super::icons::{IconPainter, IconWidget};
use crate::config::ToolbarRebindModifier;
use crate::draw::Color;
use crate::ui::theme::{ACCENT_RGB, Rgba, rgba, set_color};
use crate::ui::toolbar::ToolbarEvent;

pub(super) use crate::ui::theme::toolbar::{COLOR_SWATCH_HAIRLINE, COLOR_SWATCH_HAIRLINE_DARK};
/// Filled (dragged) portion of the slider track: the accent at reduced
/// alpha so it stays quieter than the knob (same tint as
/// COLOR_SEGMENT_ACTIVE).
const COLOR_TRACK_FILL: Rgba = rgba(ACCENT_RGB, 0.55);

/// Sender the view hands to every control closure. Clones share the configured
/// rebind chord and modifier state captured from the actual GTK click.
#[derive(Clone)]
pub(super) struct FeedbackSender {
    sink: Rc<dyn FeedbackSink>,
    rebind_modifier: Rc<Cell<ToolbarRebindModifier>>,
    backend_rebind_active: Rc<Cell<bool>>,
    click_rebind_requested: Rc<Cell<bool>>,
    click_in_progress: Rc<Cell<bool>>,
}

pub(super) trait FeedbackSink {
    fn publish(&self, feedback: GtkToolbarFeedback) -> bool;
}

impl FeedbackSink for FeedbackPublisher {
    fn publish(&self, feedback: GtkToolbarFeedback) -> bool {
        self.publish(feedback).is_ok()
    }
}

#[cfg(test)]
impl FeedbackSink for std::sync::mpsc::Sender<GtkToolbarFeedback> {
    fn publish(&self, feedback: GtkToolbarFeedback) -> bool {
        self.send(feedback).is_ok()
    }
}

impl FeedbackSender {
    pub(super) fn new(sink: impl FeedbackSink + 'static) -> Self {
        Self {
            sink: Rc::new(sink),
            rebind_modifier: Rc::new(Cell::new(ToolbarRebindModifier::default())),
            backend_rebind_active: Rc::new(Cell::new(false)),
            click_rebind_requested: Rc::new(Cell::new(false)),
            click_in_progress: Rc::new(Cell::new(false)),
        }
    }

    pub(super) fn set_rebind_state(&self, modifier: ToolbarRebindModifier, active: bool) {
        self.rebind_modifier.set(modifier);
        self.backend_rebind_active.set(active);
        if active {
            self.click_rebind_requested.set(true);
        } else if !self.click_in_progress.get() {
            self.click_rebind_requested.set(false);
        }
    }

    fn capture_click_modifiers(&self, state: gtk4::gdk::ModifierType) {
        self.click_in_progress.set(true);
        if self.rebind_modifier.get().matches(
            state.contains(gtk4::gdk::ModifierType::CONTROL_MASK),
            state.contains(gtk4::gdk::ModifierType::SHIFT_MASK),
            state.contains(gtk4::gdk::ModifierType::ALT_MASK),
        ) {
            self.click_rebind_requested.set(true);
        }
    }

    fn finish_pointer_click(&self) {
        self.click_in_progress.set(false);
        if !self.backend_rebind_active.get() {
            self.click_rebind_requested.set(false);
        }
    }

    pub(super) fn send(&self, feedback: GtkToolbarFeedback) -> Result<(), ()> {
        self.sink.publish(feedback).then_some(()).ok_or(())
    }
}

pub(super) fn send_event(sender: &FeedbackSender, event: ToolbarEvent) {
    let rebind_requested = sender.click_rebind_requested.replace(false);
    sender.click_in_progress.set(false);
    let _ = sender.send(GtkToolbarFeedback::Event {
        event,
        rebind_requested,
    });
}

/// Fixed-size button so GTK widths match the deterministic layout plan.
pub(super) fn sized_button(width: f64, height: f64) -> gtk4::Button {
    let button = gtk4::Button::new();
    button.set_size_request(width.round() as i32, height.round() as i32);
    button.set_valign(gtk4::Align::Center);
    // Pointer activation must not move keyboard focus away from the canvas:
    // the GTK bars use a separate Wayland connection and cannot receive the
    // application's shortcuts on its behalf.
    button.set_focusable(false);
    button.connect_clicked(release_window_keyboard_focus);
    button
}

/// Request layer-shell keyboard focus only while an editable field owns GTK
/// focus, then return it to the canvas when editing finishes.
pub(super) fn keyboard_on_demand_for_entry(entry: &gtk4::Entry) {
    entry.connect_has_focus_notify(|entry| {
        if entry.has_focus() {
            set_entry_keyboard_mode(entry, true);
            // Pointer focus positions the caret after focus is assigned. Wait
            // until that click finishes so the field starts as one atomic
            // value that typing or copying can replace/read immediately.
            let weak = entry.downgrade();
            gtk4::glib::idle_add_local_once(move || {
                if let Some(entry) = weak.upgrade()
                    && entry.has_focus()
                {
                    entry.select_region(0, -1);
                }
            });
        } else {
            release_window_keyboard_focus(entry);
        }
    });
    entry.connect_activate(|entry| {
        release_entry_keyboard_focus(entry);
    });

    let key = gtk4::EventControllerKey::new();
    key.connect_key_pressed(|controller, keyval, _, _| {
        if keyval == gtk4::gdk::Key::Escape {
            if let Some(entry) = controller.widget().and_downcast::<gtk4::Entry>() {
                release_entry_keyboard_focus(&entry);
            }
            return gtk4::glib::Propagation::Stop;
        }
        gtk4::glib::Propagation::Proceed
    });
    entry.add_controller(key);

    // GtkText normally opens its context menu with Copy disabled when the
    // caret has no selection. A hex color is one atomic value, so make an
    // unselected secondary click target the whole token while preserving any
    // deliberate partial selection the user already made.
    let context_click = gtk4::GestureClick::new();
    context_click.set_button(gtk4::gdk::BUTTON_SECONDARY);
    context_click.set_propagation_phase(gtk4::PropagationPhase::Capture);
    context_click.connect_pressed(|gesture, _, _, _| {
        let Some(entry) = gesture.widget().and_downcast::<gtk4::Entry>() else {
            return;
        };
        if entry.selection_bounds().is_none() {
            entry.select_region(0, -1);
        }
    });
    entry.add_controller(context_click);
}

fn release_entry_keyboard_focus(entry: &gtk4::Entry) {
    release_window_keyboard_focus(entry);
}

fn set_entry_keyboard_mode(entry: &gtk4::Entry, editing: bool) {
    if let Some(window) = entry.root().and_downcast::<gtk4::Window>() {
        window.set_keyboard_mode(if editing {
            KeyboardMode::OnDemand
        } else {
            KeyboardMode::None
        });
    }
}

fn release_window_keyboard_focus(widget: &impl IsA<gtk4::Widget>) {
    if let Some(window) = widget.root().and_downcast::<gtk4::Window>() {
        window.set_keyboard_mode(KeyboardMode::None);
        gtk4::prelude::GtkWindowExt::set_focus(&window, None::<&gtk4::Widget>);
        // Interactivity is double-buffered. Re-arm OnDemand after the
        // focus-dropping commit so a later click can focus an entry without
        // making this surface retain the current keyboard focus.
        let weak = window.downgrade();
        gtk4::glib::idle_add_local_once(move || {
            if let Some(window) = weak.upgrade() {
                window.set_keyboard_mode(KeyboardMode::OnDemand);
            }
        });
    }
}

/// Drop keyboard ownership when GTK focuses any non-entry control. Buttons
/// also release from their clicked handler because they are non-focusable and
/// may leave a previously focused entry as the logical focus widget.
pub(super) fn install_shortcut_focus_policy(window: &gtk4::Window, feedback: &FeedbackSender) {
    window.connect_focus_widget_notify(|window| {
        let Some(focus) = gtk4::prelude::GtkWindowExt::focus(window) else {
            return;
        };
        let entry_focused =
            focus.is::<gtk4::Entry>() || focus.ancestor(gtk4::Entry::static_type()).is_some();
        if !entry_focused {
            release_window_keyboard_focus(window);
        }
    });

    // A layer surface can gain compositor keyboard focus even when the
    // clicked widget itself is not focusable (for example the drag grip).
    // Inspect the pointer target after every click and keep focus only for
    // the editable hex field.
    let click = gtk4::GestureClick::new();
    click.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let click_feedback = feedback.clone();
    click.connect_pressed(move |gesture, _, _, _| {
        click_feedback.capture_click_modifiers(gesture.current_event_state());
    });
    let release_feedback = feedback.clone();
    click.connect_released(move |_, _, _, _| {
        let release_feedback = release_feedback.clone();
        gtk4::glib::idle_add_local_once(move || release_feedback.finish_pointer_click());
    });
    let weak = window.downgrade();
    click.connect_released(move |_, _, x, y| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let entry_clicked = window
            .pick(x, y, gtk4::PickFlags::DEFAULT)
            .is_some_and(|target| {
                target.is::<gtk4::Entry>() || target.ancestor(gtk4::Entry::static_type()).is_some()
            });
        if !entry_clicked {
            release_window_keyboard_focus(&window);
        }
    });
    window.add_controller(click);

    let drag = gtk4::GestureDrag::new();
    drag.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let entry_drag = Rc::new(Cell::new(false));
    let begin_entry_drag = entry_drag.clone();
    let weak = window.downgrade();
    drag.connect_drag_begin(move |_, x, y| {
        let entry_target = weak
            .upgrade()
            .and_then(|window| window.pick(x, y, gtk4::PickFlags::DEFAULT))
            .is_some_and(|target| {
                target.is::<gtk4::Entry>() || target.ancestor(gtk4::Entry::static_type()).is_some()
            });
        begin_entry_drag.set(entry_target);
    });
    let end_entry_drag = entry_drag.clone();
    let weak = window.downgrade();
    drag.connect_drag_end(move |_, _, _| {
        if !end_entry_drag.replace(false)
            && let Some(window) = weak.upgrade()
        {
            release_window_keyboard_focus(&window);
        }
    });
    let weak = window.downgrade();
    drag.connect_cancel(move |_, _| {
        if !entry_drag.replace(false)
            && let Some(window) = weak.upgrade()
        {
            release_window_keyboard_focus(&window);
        }
    });
    window.add_controller(drag);
}

pub(super) struct IconButton {
    pub(super) button: gtk4::Button,
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
    IconButton { button }
}

pub(super) fn text_button(label: &str, button_size: (f64, f64), tooltip: &str) -> gtk4::Button {
    let button = sized_button(button_size.0, button_size.1);
    button.set_label(label);
    button.set_tooltip_text(Some(tooltip));
    button
}

/// Add a compact, non-interactive shortcut badge inside a fixed-size button.
/// The caller has already filtered out modifier chords that would not fit.
pub(super) fn add_shortcut_badge(button: &gtk4::Button, badge: Option<&str>) {
    let Some(badge) = badge else {
        return;
    };
    let overlay = gtk4::Overlay::new();
    if let Some(child) = button.child() {
        button.set_child(None::<&gtk4::Widget>);
        overlay.set_child(Some(&child));
    }
    let label = gtk4::Label::new(Some(badge));
    label.add_css_class("shortcut-badge");
    label.set_can_target(false);
    label.set_halign(gtk4::Align::End);
    label.set_valign(gtk4::Align::Start);
    label.set_margin_top(2);
    label.set_margin_end(2);
    overlay.add_overlay(&label);
    button.set_child(Some(&overlay));
}

/// Put a quick-color shortcut above its swatch without changing the swatch's
/// horizontal footprint or click target.
pub(super) fn swatch_with_shortcut(
    button: &gtk4::Button,
    badge: Option<&str>,
    width: f64,
    badge_height: f64,
) -> gtk4::Widget {
    let column = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    column.set_size_request(width.round() as i32, -1);
    column.set_valign(gtk4::Align::Center);
    let label = gtk4::Label::new(Some(badge.unwrap_or("")));
    label.set_size_request(-1, badge_height.round() as i32);
    label.add_css_class("shortcut-badge");
    label.add_css_class("above-swatch");
    label.set_can_target(false);
    label.set_halign(gtk4::Align::Center);
    column.append(&label);
    column.append(button);
    column.upcast()
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
            let (r, g, b, a) = draw_color.get();
            // Rounded square with a subtle inner hairline, matching the
            // built-in bars. The fill is inset so the selected accent ring
            // (2px stroke, ~2px gap) fits inside the drawing area.
            ctx.set_source_rgba(r, g, b, a);
            rounded_rect_path(ctx, 4.0, 4.0, size - 8.0, size - 8.0, 4.0);
            let _ = ctx.fill();
            set_color(ctx, COLOR_SWATCH_HAIRLINE);
            ctx.set_line_width(1.0);
            rounded_rect_path(ctx, 4.5, 4.5, size - 9.0, size - 9.0, 3.5);
            let _ = ctx.stroke();
            if draw_selected.get() {
                set_color(ctx, super::css::ACCENT);
                ctx.set_line_width(2.0);
                rounded_rect_path(ctx, 1.0, 1.0, size - 2.0, size - 2.0, 6.0);
                let _ = ctx.stroke();
            }
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
            set_color(ctx, super::css::TRACK_BACKGROUND);
            let _ = ctx.fill();
            // Filled portion (accent at reduced alpha)
            rounded_rect_path(ctx, 0.0, track_y, (w * t).max(track_h), track_h, radius);
            set_color(ctx, COLOR_TRACK_FILL);
            let _ = ctx.fill();
            // Knob
            let knob_r = (h / 2.0).min(7.0);
            let knob_x = knob_r + t * (w - knob_r * 2.0);
            ctx.arc(knob_x, h / 2.0, knob_r, 0.0, std::f64::consts::PI * 2.0);
            set_color(ctx, super::css::TRACK_KNOB);
            let _ = ctx.fill();
        });

        let value_label = gtk4::Label::new(Some(&format(initial)));
        value_label.set_width_chars(5);
        value_label.set_xalign(1.0);

        let drag = gtk4::GestureDrag::new();
        let drag_state = state.clone();
        let drag_area = area.clone();
        let start_value = Rc::new(Cell::new((0.0f64, 0.0f64)));
        let begin_start = start_value.clone();
        let begin_label = value_label.clone();
        drag.connect_drag_begin(move |gesture, x, _| {
            drag_state.dragging.set(true);
            // Jump the knob to the pressed position, like the built-in track.
            let width = gesture.widget().map(|w| w.width()).unwrap_or(1).max(1) as f64;
            let t = (x / width).clamp(0.0, 1.0);
            let value = drag_state.min + t * (drag_state.max - drag_state.min);
            drag_state.value.set(value);
            begin_label.set_text(&format(value));
            begin_start.set((x, value));
            drag_area.queue_draw();
        });
        let update_state = state.clone();
        let update_area = area.clone();
        let update_start = start_value.clone();
        let update_label = value_label.clone();
        let change = Rc::new(on_change);
        let update_change = change.clone();
        drag.connect_drag_update(move |gesture, dx, _| {
            let width = gesture.widget().map(|w| w.width()).unwrap_or(1).max(1) as f64;
            let (sx, _) = update_start.get();
            let t = ((sx + dx) / width).clamp(0.0, 1.0);
            let value = update_state.min + t * (update_state.max - update_state.min);
            update_state.value.set(value);
            update_label.set_text(&format(value));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtk_feedback_carries_rebind_state_once() {
        let (tx, rx) = std::sync::mpsc::channel();
        let sender = FeedbackSender::new(tx);
        sender.set_rebind_state(ToolbarRebindModifier::CtrlShift, true);
        sender.capture_click_modifiers(
            gtk4::gdk::ModifierType::CONTROL_MASK | gtk4::gdk::ModifierType::SHIFT_MASK,
        );
        send_event(&sender, ToolbarEvent::Undo);
        send_event(&sender, ToolbarEvent::Redo);

        assert_eq!(
            rx.recv().unwrap(),
            GtkToolbarFeedback::Event {
                event: ToolbarEvent::Undo,
                rebind_requested: true,
            }
        );
        assert_eq!(
            rx.recv().unwrap(),
            GtkToolbarFeedback::Event {
                event: ToolbarEvent::Redo,
                rebind_requested: false,
            }
        );
    }

    #[test]
    fn backend_modifier_latch_survives_focus_reset_during_click() {
        let (tx, rx) = std::sync::mpsc::channel();
        let sender = FeedbackSender::new(tx);
        sender.set_rebind_state(ToolbarRebindModifier::CtrlShift, true);
        sender.capture_click_modifiers(gtk4::gdk::ModifierType::empty());
        sender.set_rebind_state(ToolbarRebindModifier::CtrlShift, false);

        send_event(&sender, ToolbarEvent::Undo);

        assert_eq!(
            rx.recv().unwrap(),
            GtkToolbarFeedback::Event {
                event: ToolbarEvent::Undo,
                rebind_requested: true,
            }
        );
    }
}
