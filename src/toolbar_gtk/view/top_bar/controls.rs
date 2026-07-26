//! GTK top-strip control factories.
//!
//! Owns construction and state-updater wiring for toolbar buttons and chrome.

use super::popovers::{attach_closed_dismissal, attach_escape_dismiss};
use super::*;

use crate::ui::theme::set_color;
use crate::ui::theme::toolbar::{
    COLOR_SWATCH_HAIRLINE, COLOR_SWATCH_HAIRLINE_DARK, COLOR_TEXT_SECONDARY,
    PRESET_SLOT_ICON_RATIO, PRESET_SLOT_SWATCH_INSET, PRESET_SLOT_SWATCH_RADIUS,
    PRESET_SLOT_SWATCH_RATIO,
};

use super::super::super::widgets::rounded_rect_path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ToggleControl {
    ShapePicker,
    Highlight,
    Pin,
    Overflow,
    HighlightRing,
}

impl ToggleControl {
    pub(super) fn event(self, next_active: bool) -> ToolbarEvent {
        match self {
            Self::ShapePicker => ToolbarEvent::ToggleShapePicker(next_active),
            Self::Highlight => ToolbarEvent::ToggleAllHighlight(next_active),
            Self::Pin => ToolbarEvent::PinTopToolbar(next_active),
            Self::Overflow => ToolbarEvent::ToggleTopOverflow(next_active),
            Self::HighlightRing => ToolbarEvent::ToggleHighlightToolRing(next_active),
        }
    }
}

impl TopBar {
    pub(super) fn tool_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        control: model::TopToolbarControl,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        show_badge: bool,
    ) -> gtk4::Button {
        self.action_button(
            snapshot,
            control,
            button_size,
            icon_size,
            use_icons,
            show_badge,
        )
    }

    /// Shapes picker button: the family icon opens the grid and per-tool
    /// option rows; individual shapes keep their own icons inside the popover.
    pub(super) fn shapes_picker_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
    ) -> gtk4::Button {
        let control = model::TopToolbarControl::ShapePicker;
        let tooltip = control.tooltip(snapshot);
        let label = control.label(snapshot);
        let button = if use_icons {
            icon_button(
                top_toolbar_icon_painter(model::TopToolbarIcon::ShapePicker),
                button_size,
                icon_size,
                &tooltip,
            )
            .button
        } else {
            text_button(&label, button_size, &tooltip)
        };
        set_control_widget_id(&button, control);
        let accessible_label = control.accessible_label(snapshot);
        button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        let sender = self.feedback.clone();
        button.connect_clicked(move |button| {
            send_event(
                &sender,
                ToggleControl::ShapePicker.event(!button.has_css_class("active")),
            );
        });
        let handle = button.clone();
        self.updaters.push(Box::new(move |snapshot| {
            set_active_class(&handle, control.active(snapshot));
        }));

        let popover = gtk4::Popover::new();
        popover.set_parent(&button);
        popover.set_position(gtk4::PositionType::Bottom);
        // No autohide grab: the built-in dismissal policy already closes
        // the picker through the snapshot round-trip (any other toolbar
        // event or a canvas press), so an outside click both dismisses AND
        // activates the control under it — one click, like the builtin.
        popover.set_autohide(false);
        attach_escape_dismiss(
            &popover,
            &self.feedback,
            ToggleControl::ShapePicker.event(false),
        );
        attach_closed_dismissal(
            &popover,
            &self.feedback,
            ToggleControl::ShapePicker.event(false),
        );
        let capture_surface = CaptureSurfaceContent::empty();
        popover.set_child(Some(capture_surface.widget()));
        self.shapes_popover = Some(popover);
        self.shapes_capture_surface = Some(capture_surface);
        button
    }

    pub(super) fn utility_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        control: model::TopToolbarControl,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        show_badge: bool,
    ) -> Option<gtk4::Button> {
        match control {
            model::TopToolbarControl::Utility(_) => Some(self.action_button(
                snapshot,
                control,
                button_size,
                icon_size,
                use_icons,
                show_badge,
            )),
            _ => None,
        }
    }

    pub(super) fn history_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        control: model::TopToolbarControl,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        show_badge: bool,
    ) -> gtk4::Button {
        self.action_button(
            snapshot,
            control,
            button_size,
            icon_size,
            use_icons,
            show_badge,
        )
    }

    pub(super) fn action_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        control: model::TopToolbarControl,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        show_badge: bool,
    ) -> gtk4::Button {
        let tooltip = control.tooltip(snapshot);
        let label = control.label(snapshot);
        let icon = control.icon(snapshot);
        let button = if use_icons && let Some(icon) = icon {
            icon_button(
                top_toolbar_icon_painter(icon),
                button_size,
                icon_size,
                &tooltip,
            )
            .button
        } else {
            text_button(&label, button_size, &tooltip)
        };
        set_control_widget_id(&button, control);
        let accessible_label = control.accessible_label(snapshot);
        button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        if show_badge {
            let badge = control.shortcut_badge(snapshot);
            add_button_shortcut_hint(&button, badge.as_deref(), use_icons);
        }
        set_active_class(&button, control.active(snapshot));
        let sender = self.feedback.clone();
        if matches!(
            control,
            model::TopToolbarControl::Utility(model::TopToolbarUtility::Highlight)
        ) {
            button.connect_clicked(move |button| {
                send_event(
                    &sender,
                    ToggleControl::Highlight.event(!button.has_css_class("active")),
                );
            });
            let handle = button.clone();
            self.updaters.push(Box::new(move |snapshot| {
                set_active_class(&handle, control.active(snapshot));
                handle.set_sensitive(control.enabled(snapshot));
            }));
            return button;
        }
        let event = control.event(snapshot);
        button.connect_clicked(move |_| send_event(&sender, event.clone()));
        let handle = button.clone();
        self.updaters.push(Box::new(move |snapshot| {
            set_active_class(&handle, control.active(snapshot));
            handle.set_sensitive(control.enabled(snapshot));
        }));
        button
    }

    /// A presets-island slot button: a filled slot draws the saved tool
    /// glyph in the neutral foreground with the preset color as a separate
    /// corner swatch in a DrawingArea child, so a dark preset color never
    /// renders the glyph invisible against the slot body (the side-palette
    /// convention). An empty slot shows its 1-based number as a plain label.
    /// Clicking applies a filled slot or saves the current setup into an empty
    /// one (the shared spec owns the event).
    pub(super) fn preset_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        index: usize,
        button_size: (f64, f64),
    ) -> gtk4::Button {
        let control = model::TopToolbarControl::Preset(index);
        let button = sized_button(button_size.0, button_size.1);
        set_control_widget_id(&button, control);
        button.add_css_class("preset");
        let accessible_label = control.accessible_label(snapshot);
        button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        button.set_tooltip_text(Some(&control.tooltip(snapshot)));
        set_active_class(&button, control.active(snapshot));

        if let Some(preset) = model::preset_slot(snapshot, index) {
            // Filled slot: the saved tool glyph in the neutral foreground plus
            // the preset color as a separate corner swatch (a DrawingArea
            // painting the shared Cairo icon painter over the full slot face).
            let painter = top_toolbar_icon_painter(model::TopToolbarIcon::Tool(
                model::semantic_icon_for_tool(preset.tool),
            ));
            let (r, g, b, a) = (
                preset.color.r,
                preset.color.g,
                preset.color.b,
                preset.color.a,
            );
            let scale = effective_scale(snapshot);
            let area = gtk4::DrawingArea::new();
            area.set_content_width(button_size.0.round().max(1.0) as i32);
            area.set_content_height(button_size.1.round().max(1.0) as i32);
            area.set_halign(gtk4::Align::Fill);
            area.set_valign(gtk4::Align::Fill);
            area.set_can_target(false);
            area.set_draw_func(move |_, ctx, width, height| {
                let (fw, fh) = (width as f64, height as f64);
                let short = fw.min(fh).max(1.0);
                // Neutral glyph, centered.
                set_color(ctx, COLOR_TEXT_SECONDARY);
                let icon = (short * PRESET_SLOT_ICON_RATIO).round();
                painter(ctx, (fw - icon) / 2.0, (fh - icon) / 2.0, icon);
                // Preset color as a separate bottom-right corner swatch, with a
                // luminance-driven hairline so black and white presets both
                // stay defined against the slot body.
                let sw = (short * PRESET_SLOT_SWATCH_RATIO).round();
                let inset = PRESET_SLOT_SWATCH_INSET * scale;
                let radius = PRESET_SLOT_SWATCH_RADIUS * scale;
                let sx = fw - sw - inset;
                let sy = fh - sw - inset;
                // A translucent preset paints at its own alpha over the
                // checkerboard, so the slot previews what it will apply.
                let swatch_path =
                    |ctx: &cairo::Context| rounded_rect_path(ctx, sx, sy, sw, sw, radius);
                crate::ui::checkerboard_behind(ctx, a, swatch_path);
                ctx.set_source_rgba(r, g, b, a);
                swatch_path(ctx);
                let _ = ctx.fill();
                let luminance = 0.299 * r + 0.587 * g + 0.114 * b;
                set_color(
                    ctx,
                    if luminance < 0.3 {
                        COLOR_SWATCH_HAIRLINE_DARK
                    } else {
                        COLOR_SWATCH_HAIRLINE
                    },
                );
                ctx.set_line_width(1.0);
                rounded_rect_path(ctx, sx, sy, sw, sw, radius);
                let _ = ctx.stroke();
            });
            button.set_child(Some(&area));
        } else {
            // Empty slot: the 1-based slot number.
            button.set_label(&control.label(snapshot));
        }

        let sender = self.feedback.clone();
        let event = control.event(snapshot);
        button.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        let handle = button.clone();
        self.updaters.push(Box::new(move |snapshot| {
            set_active_class(&handle, control.active(snapshot));
        }));
        button
    }

    pub(super) fn pin_button(&mut self, snapshot: &ToolbarSnapshot, size: f64) -> gtk4::Button {
        let control = model::TopToolbarControl::Pin;
        let button = sized_button(size, size);
        set_control_widget_id(&button, control);
        button.add_css_class("chrome");
        let accessible_label = control.accessible_label(snapshot);
        button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        let icon = IconWidget::new(
            top_toolbar_icon_painter(if snapshot.top_pinned {
                model::TopToolbarIcon::Pin
            } else {
                model::TopToolbarIcon::Unpin
            }),
            size * 0.62,
        );
        button.set_child(Some(&icon.area));
        if snapshot.top_pinned {
            button.add_css_class("pinned");
        }
        let sender = self.feedback.clone();
        button.connect_clicked(move |button| {
            send_event(
                &sender,
                ToggleControl::Pin.event(!button.has_css_class("pinned")),
            );
        });
        let handle = button.clone();
        self.updaters.push(Box::new(move |snapshot| {
            icon.set_painter(top_toolbar_icon_painter(if snapshot.top_pinned {
                model::TopToolbarIcon::Pin
            } else {
                model::TopToolbarIcon::Unpin
            }));
            if snapshot.top_pinned {
                handle.add_css_class("pinned");
            } else {
                handle.remove_css_class("pinned");
            }
            let accessible_label = control.accessible_label(snapshot);
            handle.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
            handle.set_tooltip_text(Some(&control.tooltip(snapshot)));
        }));
        button
    }

    /// Overflow ⋯ toggle: a regular strip-sized icon button in the history
    /// island (no round chrome styling) anchoring the overflow popover.
    pub(super) fn overflow_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        button_size: (f64, f64),
        icon_size: f64,
    ) -> gtk4::Button {
        let control = model::TopToolbarControl::Overflow;
        let button = sized_button(button_size.0, button_size.1);
        set_control_widget_id(&button, control);
        let accessible_label = control.accessible_label(snapshot);
        button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        button.set_tooltip_text(Some(&control.tooltip(snapshot)));
        let icon = IconWidget::new(
            top_toolbar_icon_painter(model::TopToolbarIcon::Overflow),
            icon_size,
        );
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        button.connect_clicked(move |button| {
            send_event(
                &sender,
                ToggleControl::Overflow.event(!button.has_css_class("active")),
            );
        });
        let handle = button.clone();
        self.updaters.push(Box::new(move |snapshot| {
            set_active_class(&handle, control.active(snapshot));
        }));
        let popover = gtk4::Popover::new();
        popover.set_parent(&button);
        popover.set_position(gtk4::PositionType::Bottom);
        // See the shapes popover: dismissal stays with the backend policy.
        popover.set_autohide(false);
        attach_escape_dismiss(
            &popover,
            &self.feedback,
            ToggleControl::Overflow.event(false),
        );
        attach_closed_dismissal(
            &popover,
            &self.feedback,
            ToggleControl::Overflow.event(false),
        );
        let capture_surface = CaptureSurfaceContent::empty();
        popover.set_child(Some(capture_surface.widget()));
        self.overflow_popover = Some(popover);
        self.overflow_capture_surface = Some(capture_surface);

        // The Canvas/Session/Settings popovers anchor to the same ⋯ toggle
        // their menu entries live in; the entries themselves render inside
        // the overflow popover from the shared spec.
        let (canvas_popover, canvas_capture) =
            self.menu_popover(&button, ToolbarEvent::ToggleCanvasPopover(false));
        self.canvas_popover = Some(canvas_popover);
        self.canvas_capture_surface = Some(canvas_capture);
        let (session_popover, session_capture) =
            self.menu_popover(&button, ToolbarEvent::ToggleSessionPopover(false));
        self.session_popover = Some(session_popover);
        self.session_capture_surface = Some(session_capture);
        let (settings_popover, settings_capture) =
            self.menu_popover(&button, ToolbarEvent::ToggleSettingsPopover(false));
        self.settings_popover = Some(settings_popover);
        self.settings_capture_surface = Some(settings_capture);
        button
    }

    /// One overflow-anchored Canvas/Session/Settings popover following the shapes/
    /// overflow pattern: no autohide grab (the backend dismissal policy owns
    /// click-away), Escape wired explicitly, `closed` echoing user dismissal.
    fn menu_popover(
        &self,
        parent: &gtk4::Button,
        dismiss: ToolbarEvent,
    ) -> (gtk4::Popover, CaptureSurfaceContent) {
        let popover = gtk4::Popover::new();
        popover.set_parent(parent);
        popover.set_position(gtk4::PositionType::Bottom);
        popover.set_autohide(false);
        attach_escape_dismiss(&popover, &self.feedback, dismiss.clone());
        attach_closed_dismissal(&popover, &self.feedback, dismiss);
        let capture_surface = CaptureSurfaceContent::empty();
        popover.set_child(Some(capture_surface.widget()));
        (popover, capture_surface)
    }

    /// The chrome island's About entry. Styled like the other chrome buttons,
    /// but it opens a window instead of changing the toolbar, so it carries no
    /// active state.
    pub(super) fn about_button(&mut self, snapshot: &ToolbarSnapshot, size: f64) -> gtk4::Button {
        let control = model::TopToolbarControl::About;
        let button = sized_button(size, size);
        set_control_widget_id(&button, control);
        button.add_css_class("chrome");
        button.add_css_class("about");
        let accessible_label = control.accessible_label(snapshot);
        button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        button.set_tooltip_text(Some(&control.tooltip(snapshot)));
        let icon = IconWidget::new(
            top_toolbar_icon_painter(model::TopToolbarIcon::About),
            size * 0.6,
        );
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        let event = control.event(snapshot);
        button.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        button
    }

    pub(super) fn minimize_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        size: f64,
    ) -> gtk4::Button {
        let control = model::TopToolbarControl::Minimize;
        let button = sized_button(size, size);
        set_control_widget_id(&button, control);
        button.add_css_class("chrome");
        button.add_css_class("minimize");
        let accessible_label = control.accessible_label(snapshot);
        button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        button.set_tooltip_text(Some(&control.tooltip(snapshot)));
        let icon = IconWidget::new(
            top_toolbar_icon_painter(model::TopToolbarIcon::Minimize),
            size * 0.6,
        );
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        let event = control.event(snapshot);
        button.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        button
    }
}
