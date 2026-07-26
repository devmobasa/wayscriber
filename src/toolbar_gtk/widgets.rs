//! Shared widget constructors for the GTK toolbars.

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

/// Sender the view hands to every control closure. Clones publish ordered
/// intents to the one feedback-mailbox owner; no widget owns a state mirror.
#[derive(Clone)]
pub(super) struct FeedbackSender {
    publisher: FeedbackPublisher,
}

impl FeedbackSender {
    pub(super) fn new(publisher: FeedbackPublisher) -> Self {
        Self { publisher }
    }

    pub(super) fn set_rebind_state(
        &self,
        modifier: ToolbarRebindModifier,
        active: bool,
    ) -> Result<(), ()> {
        self.publisher
            .set_rebind_state(modifier, active)
            .map_err(|_| ())
    }

    fn capture_click_modifiers(&self, state: gtk4::gdk::ModifierType) {
        let _ = self.publisher.capture_click_modifiers(
            state.contains(gtk4::gdk::ModifierType::CONTROL_MASK),
            state.contains(gtk4::gdk::ModifierType::SHIFT_MASK),
            state.contains(gtk4::gdk::ModifierType::ALT_MASK),
        );
    }

    fn finish_pointer_click(&self) {
        let _ = self.publisher.finish_pointer_click();
    }

    pub(super) fn send(&self, feedback: GtkToolbarFeedback) -> Result<(), ()> {
        self.publisher.publish(feedback).map_err(|_| ())
    }
}

#[cfg(test)]
pub(super) fn test_feedback_channel() -> (FeedbackSender, super::bridge::TestMailbox) {
    let mailbox = super::bridge::publisher_channel();
    (FeedbackSender::new(mailbox.publisher()), mailbox)
}

pub(super) fn send_event(sender: &FeedbackSender, event: ToolbarEvent) {
    let _ = sender.publisher.publish_event(event);
}

/// Secondary-click recolor gesture for a quick-color swatch: right-clicking
/// the swatch opens the picker popup bound to that palette slot, so accepting
/// it rewrites the swatch instead of only the active tool's color. A GTK
/// `Button` activates on the primary button alone, so the gesture is explicit;
/// it claims the sequence so no ancestor controller also reacts to the press.
pub(super) fn install_quick_color_recolor(
    button: &gtk4::Button,
    index: usize,
    feedback: &FeedbackSender,
) {
    let gesture = gtk4::GestureClick::new();
    gesture.set_button(gtk4::gdk::BUTTON_SECONDARY);
    let feedback = feedback.clone();
    gesture.connect_pressed(move |gesture, _, _, _| {
        gesture.set_state(gtk4::EventSequenceState::Claimed);
        send_event(&feedback, ToolbarEvent::EditQuickColor { index });
    });
    button.add_controller(gesture);
}

/// The secondary-click gesture installed on a widget, if it has one.
#[cfg(test)]
pub(super) fn secondary_click_gesture(widget: &gtk4::Widget) -> Option<gtk4::GestureClick> {
    let controllers = widget.observe_controllers();
    (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .filter_map(|controller| controller.downcast::<gtk4::GestureClick>().ok())
        .find(|gesture| gesture.button() == gtk4::gdk::BUTTON_SECONDARY)
}

/// Drive a widget's secondary-click gesture the way `emit_clicked` drives a
/// button, so tests can assert the resulting feedback without a compositor
/// delivering real pointer events.
#[cfg(test)]
pub(super) fn emit_secondary_press(widget: &gtk4::Widget) {
    let gesture = secondary_click_gesture(widget).expect("secondary-click gesture");
    gesture.emit_by_name::<()>("pressed", &[&1i32, &0.0f64, &0.0f64]);
}

/// Capture click modifiers on a widget subtree that lives outside the
/// toolbar window's own capture controller (popovers are separate GTK
/// natives, so the window-level gesture never sees their clicks).
pub(super) fn install_click_modifier_capture(
    widget: &impl IsA<gtk4::Widget>,
    feedback: &FeedbackSender,
) {
    let click = gtk4::GestureClick::new();
    click.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let press_feedback = feedback.clone();
    click.connect_pressed(move |gesture, _, _, _| {
        press_feedback.capture_click_modifiers(gesture.current_event_state());
    });
    // Mirror the toolbar window's controller: a click that never activates a
    // button (cancelled, or dragged off it) still has to clear the pending
    // rebind, or the next ordinary click would be treated as a rebind. The idle
    // hop lets the button's own `clicked` handler consume the flag first.
    let release_feedback = feedback.clone();
    click.connect_released(move |_, _, _, _| {
        let release_feedback = release_feedback.clone();
        gtk4::glib::idle_add_local_once(move || release_feedback.finish_pointer_click());
    });
    widget.as_ref().add_controller(click);
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

/// Whether a focus-widget change should drop the toolbar window's keyboard
/// mode. Entries keep it: the editable hex field needs the keys it captures.
/// Focus inside a popover keeps it too, for a different reason: the popover
/// is an xdg_popup holding a keyboard grab on its own surface. Dropping the
/// parent layer surface's keyboard mode there makes the compositor pull
/// keyboard focus from the grabbing popup between a click's press and
/// release, and that broken grab stalls the release — a Settings checkbox
/// then takes seconds to toggle. The grab already returns keyboard focus to
/// the compositor when the popover closes.
fn focus_change_releases_keyboard(focus: &gtk4::Widget) -> bool {
    let entry_focused = focus.ancestor(gtk4::Entry::static_type()).is_some();
    let popover_focused = focus.ancestor(gtk4::Popover::static_type()).is_some();
    !entry_focused && !popover_focused
}

/// Drop keyboard ownership when GTK focuses any non-entry control. Buttons
/// also release from their clicked handler because they are non-focusable and
/// may leave a previously focused entry as the logical focus widget.
pub(super) fn install_shortcut_focus_policy(window: &gtk4::Window, feedback: &FeedbackSender) {
    window.connect_focus_widget_notify(|window| {
        let Some(focus) = gtk4::prelude::GtkWindowExt::focus(window) else {
            return;
        };
        if focus_change_releases_keyboard(&focus) {
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
    let weak = window.downgrade();
    drag.connect_drag_end(move |gesture, _, _| {
        if let Some(window) = weak.upgrade()
            && !gesture_started_on_entry(gesture, &window)
        {
            release_window_keyboard_focus(&window);
        }
    });
    let weak = window.downgrade();
    drag.connect_cancel(move |gesture, _| {
        if let Some(window) = weak.upgrade()
            && !gesture_started_on_entry(gesture, &window)
        {
            release_window_keyboard_focus(&window);
        }
    });
    window.add_controller(drag);
}

fn gesture_started_on_entry(gesture: &gtk4::GestureDrag, window: &gtk4::Window) -> bool {
    gesture
        .start_point()
        .and_then(|(x, y)| window.pick(x, y, gtk4::PickFlags::DEFAULT))
        .is_some_and(|target| {
            target.is::<gtk4::Entry>() || target.ancestor(gtk4::Entry::static_type()).is_some()
        })
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

/// Stack a small caption with the shortcut key under the button's icon
/// (Excalidraw pattern), inside the unchanged button tile. Icon-mode
/// counterpart of the boxed corner badge that text buttons keep.
pub(super) fn add_shortcut_caption_below(button: &gtk4::Button, badge: Option<&str>) {
    let Some(badge) = badge else {
        return;
    };
    let column = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    column.set_halign(gtk4::Align::Center);
    column.set_valign(gtk4::Align::Center);
    if let Some(child) = button.child() {
        button.set_child(None::<&gtk4::Widget>);
        column.append(&child);
    }
    let label = gtk4::Label::new(Some(badge));
    label.add_css_class("shortcut-badge");
    label.add_css_class("below-icon");
    label.set_can_target(false);
    label.set_halign(gtk4::Align::Center);
    column.append(&label);
    button.set_child(Some(&column));
}

/// Shortcut hint for a strip/popover button: icon buttons get the caption
/// under the icon, text buttons keep the boxed corner badge.
pub(super) fn add_button_shortcut_hint(
    button: &gtk4::Button,
    badge: Option<&str>,
    use_icons: bool,
) {
    if use_icons {
        add_shortcut_caption_below(button, badge);
    } else {
        add_shortcut_badge(button, badge);
    }
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
    area: gtk4::DrawingArea,
}

impl SwatchButton {
    pub(super) fn new(color: Color, selected: bool, diameter: f64, tooltip: &str) -> Self {
        let button = sized_button(diameter, diameter);
        button.add_css_class("swatch");
        button.set_tooltip_text(Some(tooltip));
        let area = gtk4::DrawingArea::new();
        let size = diameter.round() as i32;
        area.set_content_width(size);
        area.set_content_height(size);
        area.set_can_target(false);
        set_active_class(&area, selected);
        install_swatch_draw(&area, color);
        button.set_child(Some(&area));
        Self { button, area }
    }

    pub(super) fn set_selected(&self, selected: bool) {
        if self.area.has_css_class("active") != selected {
            set_active_class(&self.area, selected);
            self.area.queue_draw();
        }
    }

    pub(super) fn set_color(&self, color: Color) {
        install_swatch_draw(&self.area, color);
        self.area.queue_draw();
    }
}

fn install_swatch_draw(area: &gtk4::DrawingArea, color: Color) {
    area.set_draw_func(move |area, ctx, width, height| {
        let size = width.min(height) as f64;
        let (r, g, b, a) = (color.r, color.g, color.b, color.a);
        // Rounded square with a subtle inner hairline, matching the built-in
        // bars. The fill is inset so the selected accent ring (2px stroke,
        // ~2px gap) fits inside the drawing area. A translucent color sits on
        // the checkerboard, as the built-in bars paint it.
        let swatch_path =
            |ctx: &cairo::Context| rounded_rect_path(ctx, 4.0, 4.0, size - 8.0, size - 8.0, 4.0);
        crate::ui::checkerboard_behind(ctx, a, swatch_path);
        ctx.set_source_rgba(r, g, b, a);
        swatch_path(ctx);
        let _ = ctx.fill();
        set_color(ctx, COLOR_SWATCH_HAIRLINE);
        ctx.set_line_width(1.0);
        rounded_rect_path(ctx, 4.5, 4.5, size - 9.0, size - 9.0, 3.5);
        let _ = ctx.stroke();
        if area.has_css_class("active") {
            set_color(ctx, super::css::ACCENT);
            ctx.set_line_width(2.0);
            rounded_rect_path(ctx, 1.0, 1.0, size - 2.0, size - 2.0, 6.0);
            let _ = ctx.stroke();
        }
    });
}

/// Custom slider matching the built-in track + knob (a `DrawingArea` with
/// a drag gesture), so a live backend update never fights an in-flight
/// drag: incoming values are ignored while `dragging` is set.
pub(super) struct SliderRow {
    pub(super) root: gtk4::Box,
    value_label: gtk4::Label,
    adjustment: gtk4::Adjustment,
    drag: gtk4::GestureDrag,
    feedback_handler: gtk4::glib::SignalHandlerId,
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
        let adjustment = gtk4::Adjustment::new(initial.clamp(min, max), min, max, 0.0, 0.0, 0.0);

        let area = gtk4::DrawingArea::new();
        area.set_content_height((16.0 * scale).round() as i32);
        area.set_hexpand(true);
        area.set_valign(gtk4::Align::Center);
        let draw_adjustment = adjustment.clone();
        area.set_draw_func(move |_, ctx, width, height| {
            let w = width as f64;
            let h = height as f64;
            let track_h = (h * 0.5).min(8.0);
            let track_y = (h - track_h) / 2.0;
            let radius = track_h / 2.0;
            let t = ((draw_adjustment.value() - draw_adjustment.lower())
                / (draw_adjustment.upper() - draw_adjustment.lower()))
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

        let value_label = gtk4::Label::new(Some(&format(adjustment.value())));
        value_label.set_width_chars(5);
        value_label.set_xalign(1.0);

        let presentation_area = area.clone();
        let presentation_label = value_label.clone();
        adjustment.connect_value_changed(move |adjustment| {
            presentation_label.set_text(&format(adjustment.value()));
            presentation_area.queue_draw();
        });
        let feedback_handler = adjustment.connect_value_changed(move |adjustment| {
            on_change(adjustment.value());
        });

        let drag = gtk4::GestureDrag::new();
        let begin_adjustment = adjustment.clone();
        drag.connect_drag_begin(move |gesture, x, _| {
            // Jump the knob to the pressed position, like the built-in track.
            let width = gesture.widget().map(|w| w.width()).unwrap_or(1).max(1) as f64;
            let t = (x / width).clamp(0.0, 1.0);
            let value = begin_adjustment.lower()
                + t * (begin_adjustment.upper() - begin_adjustment.lower());
            begin_adjustment.set_value(value);
        });
        let update_adjustment = adjustment.clone();
        drag.connect_drag_update(move |gesture, dx, _| {
            let Some((start_x, _)) = gesture.start_point() else {
                return;
            };
            let width = gesture.widget().map(|w| w.width()).unwrap_or(1).max(1) as f64;
            let t = ((start_x + dx) / width).clamp(0.0, 1.0);
            let value = update_adjustment.lower()
                + t * (update_adjustment.upper() - update_adjustment.lower());
            update_adjustment.set_value(value);
        });
        let drag_owner = drag.clone();
        area.add_controller(drag);

        root.append(&area);
        root.append(&value_label);
        Self {
            root,
            value_label,
            adjustment,
            drag: drag_owner,
            feedback_handler,
        }
    }

    /// Hide the built-in readout when a separate numeral control shows the
    /// value (the style pill pairs its sliders with distinct value buttons).
    pub(super) fn set_value_label_visible(&self, visible: bool) {
        self.value_label.set_visible(visible);
    }

    /// Applies a backend value unless the user is mid-drag.
    pub(super) fn set_value(&self, value: f64) {
        if self.drag.is_active() {
            return;
        }
        let clamped = value.clamp(self.adjustment.lower(), self.adjustment.upper());
        if (self.adjustment.value() - clamped).abs() > f64::EPSILON {
            self.adjustment.block_signal(&self.feedback_handler);
            self.adjustment.set_value(clamped);
            self.adjustment.unblock_signal(&self.feedback_handler);
        }
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
mod tests;
