//! Slider interaction retains the Cairo geometry and the active-drag update barrier.
use super::rounded_rect_path;
use crate::ui::theme::{ACCENT_RGB, Rgba, rgba, set_color};
use crate::ui::toolbar::model::{OpacityPaint, ToolbarSliderSpec};
use gtk4::prelude::*;
use std::{cell::Cell, rc::Rc};

const COLOR_TRACK_FILL: Rgba = rgba(ACCENT_RGB, 0.55);

/// Custom slider matching the built-in track + knob (a `DrawingArea` with
/// a drag gesture), so a live backend update never fights an in-flight
/// drag: incoming values are ignored while `dragging` is set.
pub(in crate::toolbar_gtk) struct SliderRow {
    pub(in crate::toolbar_gtk) root: gtk4::Box,
    value_label: gtk4::Label,
    area: gtk4::DrawingArea,
    state: Rc<SliderState>,
    format: fn(f64) -> String,
}

struct SliderState {
    spec: ToolbarSliderSpec,
    value: Cell<f64>,
    dragging: Cell<bool>,
    /// Set for the marker opacity slider: its track fades from clear to
    /// solid in the stroke color instead of filling with the accent.
    opacity_paint: Cell<Option<OpacityPaint>>,
}

impl SliderRow {
    /// `on_change` fires continuously during a drag with the new value.
    pub(in crate::toolbar_gtk) fn new(
        scale: f64,
        name: &str,
        spec: ToolbarSliderSpec,
        initial: f64,
        format: fn(f64) -> String,
        on_change: impl Fn(f64) + 'static,
    ) -> Self {
        let root = gtk4::Box::new(gtk4::Orientation::Horizontal, (6.0 * scale).round() as i32);
        // Backend/config values are valid throughout the continuous range and
        // stay visible exactly as stored. Snapping begins only when the user
        // interacts with the slider.
        let initial = spec.clamp(initial);
        let state = Rc::new(SliderState {
            spec,
            value: Cell::new(initial),
            dragging: Cell::new(false),
            opacity_paint: Cell::new(None),
        });

        let area = gtk4::DrawingArea::builder()
            .accessible_role(gtk4::AccessibleRole::Slider)
            .focusable(true)
            .focus_on_click(false)
            .build();
        area.update_property(&[
            gtk4::accessible::Property::Label(name),
            gtk4::accessible::Property::ValueMin(spec.min),
            gtk4::accessible::Property::ValueMax(spec.max),
            gtk4::accessible::Property::Orientation(gtk4::Orientation::Horizontal),
        ]);
        update_accessible_value(&area, initial, format);
        area.set_content_height((16.0 * scale).round() as i32);
        area.set_hexpand(true);
        area.set_valign(gtk4::Align::Center);
        let draw_state = state.clone();
        area.set_draw_func(move |widget, ctx, width, height| {
            let w = width as f64;
            let h = height as f64;
            let track_h = (h * 0.5).min(8.0);
            let track_y = (h - track_h) / 2.0;
            let radius = track_h / 2.0;
            let t = draw_state.spec.t_from_value(draw_state.value.get());
            if widget.has_focus() {
                set_color(ctx, crate::toolbar_gtk::css::ACCENT);
                ctx.set_line_width(1.5);
                rounded_rect_path(ctx, 1.0, 1.0, w - 2.0, h - 2.0, 3.0);
                let _ = ctx.stroke();
            }
            if let Some(paint) = draw_state.opacity_paint.get() {
                crate::toolbar_icons::draw_opacity_track(
                    ctx,
                    (0.0, 0.0, w, h),
                    paint.rgb,
                    paint.alpha_stops,
                );
            } else {
                // Track
                rounded_rect_path(ctx, 0.0, track_y, w, track_h, radius);
                set_color(ctx, crate::toolbar_gtk::css::TRACK_BACKGROUND);
                let _ = ctx.fill();
                // Filled portion (accent at reduced alpha)
                rounded_rect_path(ctx, 0.0, track_y, (w * t).max(track_h), track_h, radius);
                set_color(ctx, COLOR_TRACK_FILL);
                let _ = ctx.fill();
                crate::toolbar_icons::draw_slider_ticks(
                    ctx,
                    (0.0, 0.0, w, h),
                    draw_state.spec.tick_positions(),
                );
            }
            // Knob
            let knob_r = (h / 2.0).min(7.0);
            let knob_x = knob_r + t * (w - knob_r * 2.0);
            ctx.arc(knob_x, h / 2.0, knob_r, 0.0, std::f64::consts::PI * 2.0);
            set_color(ctx, crate::toolbar_gtk::css::TRACK_KNOB);
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
            let value = drag_state.spec.value_from_t(t);
            drag_state.value.set(value);
            begin_label.set_text(&format(value));
            begin_start.set((x, value));
            update_accessible_value(&drag_area, value, format);
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
            let value = update_state.spec.value_from_t(t);
            update_state.value.set(value);
            update_label.set_text(&format(value));
            update_accessible_value(&update_area, value, format);
            update_area.queue_draw();
            update_change(value);
        });
        let end_state = state.clone();
        let end_change = change.clone();
        drag.connect_drag_end(move |_, _, _| {
            end_state.dragging.set(false);
            end_change(end_state.value.get());
        });
        let cancel_state = state.clone();
        drag.connect_cancel(move |_, _| cancel_state.dragging.set(false));
        area.add_controller(drag);

        let key = gtk4::EventControllerKey::new();
        let key_state = state.clone();
        let key_area = area.clone();
        let key_label = value_label.clone();
        let key_change = change.clone();
        key.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                if !key_state.dragging.get() {
                    if let Some(popover) = key_area
                        .ancestor(gtk4::Popover::static_type())
                        .and_downcast::<gtk4::Popover>()
                    {
                        // Dismiss the grabbed native before changing layer focus.
                        popover.popdown();
                    } else {
                        super::release_window_keyboard_focus(&key_area);
                    }
                }
                return gtk4::glib::Propagation::Stop;
            }
            let Some(value) = keyboard_value(key_state.spec, key_state.value.get(), key) else {
                return gtk4::glib::Propagation::Proceed;
            };
            if !key_state.dragging.get() {
                key_state.value.set(value);
                key_label.set_text(&format(value));
                update_accessible_value(&key_area, value, format);
                key_area.queue_draw();
                key_change(value);
            }
            gtk4::glib::Propagation::Stop
        });
        area.add_controller(key);
        area.connect_has_focus_notify(|area| area.queue_draw());

        // One step per wheel notch, like the arrow keys; GTK's discrete
        // controller folds high-resolution and touchpad travel into notches.
        let scroll = gtk4::EventControllerScroll::new(
            gtk4::EventControllerScrollFlags::VERTICAL | gtk4::EventControllerScrollFlags::DISCRETE,
        );
        let scroll_state = state.clone();
        let scroll_area = area.clone();
        let scroll_label = value_label.clone();
        let scroll_change = change.clone();
        scroll.connect_scroll(move |_, _, dy| {
            if scroll_state.dragging.get() {
                return gtk4::glib::Propagation::Stop;
            }
            let Some(value) = wheel_value(scroll_state.spec, scroll_state.value.get(), dy) else {
                return gtk4::glib::Propagation::Stop;
            };

            scroll_state.value.set(value);
            scroll_label.set_text(&format(value));
            update_accessible_value(&scroll_area, value, format);
            scroll_area.queue_draw();
            scroll_change(value);
            gtk4::glib::Propagation::Stop
        });
        area.add_controller(scroll);

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

    /// Show an inline readout in a fixed slot immediately after the track.
    /// Other slider rows keep their natural five-character, right-aligned
    /// readout; the style pill instead mirrors the built-in toolbar's track +
    /// readout geometry.
    pub(in crate::toolbar_gtk) fn configure_inline_readout(&self, visible: bool, width: i32) {
        self.value_label.set_visible(visible);
        if visible {
            self.value_label.set_width_chars(-1);
            self.value_label.set_size_request(width, -1);
            self.value_label.set_xalign(0.0);
        }
    }

    /// Paint the track as the marker opacity fade (`Some`) or the plain
    /// accent fill (`None`); redraws only when the paint changes.
    pub(in crate::toolbar_gtk) fn set_opacity_paint(&self, paint: Option<OpacityPaint>) {
        if self.state.opacity_paint.replace(paint) != paint {
            self.area.queue_draw();
        }
    }

    /// Applies a backend value unless the user is mid-drag.
    pub(in crate::toolbar_gtk) fn set_value(&self, value: f64) {
        if self.state.dragging.get() {
            return;
        }
        let clamped = self.state.spec.clamp(value);
        if (self.state.value.get() - clamped).abs() > f64::EPSILON {
            self.state.value.set(clamped);
            self.area.queue_draw();
        }
        self.value_label.set_text(&(self.format)(clamped));
        update_accessible_value(&self.area, clamped, self.format);
    }
}

fn update_accessible_value(area: &gtk4::DrawingArea, value: f64, format: fn(f64) -> String) {
    area.update_property(&[
        gtk4::accessible::Property::ValueNow(value),
        gtk4::accessible::Property::ValueText(&format(value)),
    ]);
}

/// How a navigation key moves a focused slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SliderStep {
    Down,
    Up,
    PageDown,
    PageUp,
    Min,
    Max,
}

/// The keys a focused slider owns, keypad variants included. The key relay
/// reads this same map, so the keys it leaves to a slider are exactly the
/// keys the slider acts on.
fn slider_step_for_key(key: gtk4::gdk::Key) -> Option<SliderStep> {
    use gtk4::gdk::Key;

    Some(match key {
        Key::Left | Key::Down | Key::KP_Left | Key::KP_Down => SliderStep::Down,
        Key::Right | Key::Up | Key::KP_Right | Key::KP_Up => SliderStep::Up,
        Key::Page_Down | Key::KP_Page_Down => SliderStep::PageDown,
        Key::Page_Up | Key::KP_Page_Up => SliderStep::PageUp,
        Key::Home | Key::KP_Home => SliderStep::Min,
        Key::End | Key::KP_End => SliderStep::Max,
        _ => return None,
    })
}

pub(super) fn is_slider_navigation_key(key: gtk4::gdk::Key) -> bool {
    slider_step_for_key(key).is_some()
}

/// The value one wheel movement of `dy` notches reaches (positive `dy`
/// scrolls down and lowers it), or `None` for travel under half a notch.
fn wheel_value(spec: ToolbarSliderSpec, value: f64, dy: f64) -> Option<f64> {
    let notches = -dy.round();
    if notches == 0.0 {
        return None;
    }

    Some(spec.step_value(value, notches))
}

fn keyboard_value(spec: ToolbarSliderSpec, value: f64, key: gtk4::gdk::Key) -> Option<f64> {
    Some(match slider_step_for_key(key)? {
        SliderStep::Down => spec.step_value(value, -1.0),
        SliderStep::Up => spec.step_value(value, 1.0),
        SliderStep::PageDown => spec.step_value(value, -10.0),
        SliderStep::PageUp => spec.step_value(value, 10.0),
        SliderStep::Min => spec.min,
        SliderStep::Max => spec.max,
    })
}

/// Called by the isolated GTK widget test after initialization.
#[cfg(test)]
pub(super) fn assert_widget_contract() {
    let changes = Rc::new(std::cell::RefCell::new(Vec::new()));
    let observed = changes.clone();
    let slider = SliderRow::new(
        1.0,
        "Thickness",
        ToolbarSliderSpec::THICKNESS,
        4.5,
        |value| format!("{value} pt"),
        move |value| observed.borrow_mut().push(value),
    );
    assert!(slider.area.is_focusable());
    assert_eq!(slider.area.accessible_role(), gtk4::AccessibleRole::Slider);
    for property in [
        gtk4::AccessibleProperty::Label,
        gtk4::AccessibleProperty::ValueMin,
        gtk4::AccessibleProperty::ValueMax,
        gtk4::AccessibleProperty::ValueNow,
        gtk4::AccessibleProperty::ValueText,
    ] {
        assert!(gtk4::test_accessible_has_property(&slider.area, property));
    }
    assert!(!super::focus_change_releases_keyboard(
        slider.area.upcast_ref()
    ));
    let controllers = slider.area.observe_controllers();
    let key = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .find_map(|item| item.downcast::<gtk4::EventControllerKey>().ok())
        .unwrap();
    key.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk4::gdk::Key::Right,
            &0u32,
            &gtk4::gdk::ModifierType::empty(),
        ],
    );
    // An off-grid 4.5 steps up to the adjacent whole value, 5, not 6.
    assert_eq!(slider.state.value.get(), 5.0);
    assert_eq!(changes.borrow().as_slice(), &[5.0]);
    let window = gtk4::Window::new();
    window.set_child(Some(&slider.root));
    gtk4::prelude::GtkWindowExt::set_focus(&window, Some(&slider.area));
    let escape = || {
        key.emit_by_name::<bool>(
            "key-pressed",
            &[
                &gtk4::gdk::Key::Escape,
                &0u32,
                &gtk4::gdk::ModifierType::empty(),
            ],
        )
    };
    slider.state.dragging.set(true);
    assert!(escape());
    assert!(gtk4::prelude::GtkWindowExt::focus(&window).is_some());
    slider.set_value(10.0);
    assert_eq!(slider.state.value.get(), 5.0);
    slider.state.dragging.set(false);
    assert!(escape());
    assert!(gtk4::prelude::GtkWindowExt::focus(&window).is_none());
    assert_eq!(slider.state.value.get(), 5.0);
    window.set_child(None::<&gtk4::Widget>);
    let anchor = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    window.set_child(Some(&anchor));
    let popover = gtk4::Popover::new();
    popover.set_parent(&anchor);
    popover.set_child(Some(&slider.root));
    popover.set_visible(true);
    assert!(escape());
    assert!(
        !popover.is_visible(),
        "Escape dismisses the slider's popover"
    );
    popover.set_child(None::<&gtk4::Widget>);
    popover.unparent();
    slider.set_value(6.25);
    assert_eq!(slider.state.value.get(), 6.25);
    assert_eq!(
        changes.borrow().as_slice(),
        &[5.0],
        "backend updates emit no user event"
    );
}

/// Escape on a Tab-focused slider must release the slider, not reach the
/// overlay, where no open menu would route it to Exit. The relay leaves the
/// press to the slider and only tells the overlay, which arms its guard.
#[cfg(test)]
pub(super) fn assert_focused_slider_escape_stays_local() {
    let (tx, rx) = std::sync::mpsc::channel();
    let slider = SliderRow::new(
        1.0,
        "Thickness",
        ToolbarSliderSpec::THICKNESS,
        4.0,
        |value| format!("{value} px"),
        |_| {},
    );
    let window = gtk4::Window::new();
    window.set_child(Some(&slider.root));
    super::install_key_relay(&window, &super::FeedbackSender::new(tx));
    let relay = super::key_relay_controller(&window).expect("the window relays keys");
    gtk4::prelude::GtkWindowExt::set_focus(&window, Some(&slider.area));

    let handled = relay.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk4::gdk::Key::Escape,
            &0u32,
            &gtk4::gdk::ModifierType::empty(),
        ],
    );

    assert!(!handled, "Escape goes on to the focused slider");
    assert_eq!(
        rx.try_recv(),
        Ok(crate::toolbar_gtk::GtkToolbarFeedback::EscapeDismissed {
            released_keyboard: true
        }),
        "a bar slider drops the toolbar keyboard, so the overlay takes it back"
    );
    assert!(
        rx.try_recv().is_err(),
        "no key is relayed, so the overlay routes no Exit"
    );
    window.set_child(None::<&gtk4::Widget>);
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The GTK wheel and arrows agree with the numeral and the canvas wheel: a
    /// fractional width steps to its neighbour, never skipping a pixel.
    #[test]
    fn wheel_and_arrows_step_a_fractional_width_to_its_neighbour() {
        use gtk4::gdk::Key;
        let spec = ToolbarSliderSpec::THICKNESS;

        assert_eq!(wheel_value(spec, 30.8, -1.0), Some(31.0));
        assert_eq!(wheel_value(spec, 3.2, 1.0), Some(3.0));
        assert_eq!(keyboard_value(spec, 30.8, Key::Right), Some(31.0));
        assert_eq!(keyboard_value(spec, 30.8, Key::Left), Some(30.0));
    }

    #[test]
    fn a_wheel_notch_steps_the_slider_one_step_and_up_raises_it() {
        let spec = ToolbarSliderSpec::THICKNESS;

        assert_eq!(wheel_value(spec, 10.0, -1.0), Some(11.0));
        assert_eq!(wheel_value(spec, 10.0, 2.0), Some(8.0));
        assert_eq!(wheel_value(spec, 50.0, -1.0), Some(50.0));
        assert_eq!(wheel_value(spec, 10.0, 0.2), None);
        assert_eq!(
            wheel_value(ToolbarSliderSpec::MARKER_OPACITY, 0.4, -1.0).map(|v| (v * 100.0).round()),
            Some(45.0)
        );
    }

    #[test]
    fn keypad_navigation_keys_move_the_slider_like_the_main_block() {
        use gtk4::gdk::Key;
        let spec = ToolbarSliderSpec::THICKNESS;

        for (main, keypad) in [
            (Key::Left, Key::KP_Left),
            (Key::Right, Key::KP_Right),
            (Key::Page_Up, Key::KP_Page_Up),
            (Key::Page_Down, Key::KP_Page_Down),
            (Key::Home, Key::KP_Home),
            (Key::End, Key::KP_End),
        ] {
            assert_eq!(
                keyboard_value(spec, 10.0, keypad),
                keyboard_value(spec, 10.0, main),
                "{keypad:?} matches {main:?}"
            );
            assert!(is_slider_navigation_key(keypad));
        }
        assert!(!is_slider_navigation_key(Key::Escape));
    }

    #[test]
    fn keyboard_uses_shared_snapping_and_clamps_endpoints() {
        use gtk4::gdk::Key;
        let spec = ToolbarSliderSpec::SPOTLIGHT_MAGNIFICATION;
        assert_eq!(keyboard_value(spec, spec.min, Key::Left), Some(spec.min));
        assert_eq!(keyboard_value(spec, spec.max, Key::Up), Some(spec.max));
        assert_eq!(keyboard_value(spec, spec.min, Key::End), Some(spec.max));
        assert_eq!(keyboard_value(spec, spec.max, Key::Home), Some(spec.min));
        let next = keyboard_value(spec, spec.min, Key::Right).unwrap();
        assert_eq!(next, spec.value_from_t(spec.t_from_value(next)));
        assert_eq!(
            keyboard_value(spec, spec.min, Key::Page_Up),
            Some(spec.normalize_value(spec.min + spec.step.unwrap() * 10.0))
        );
        assert_eq!(keyboard_value(spec, spec.min, Key::Escape), None);
    }
}
