//! GTK top-strip control factories.
//!
//! Owns construction and state-updater wiring for toolbar buttons and chrome.

use super::popovers::attach_escape_dismiss;
use super::*;

pub(super) fn event_for_toggle_state(
    control: model::TopToolbarControl,
    next_active: bool,
) -> ToolbarEvent {
    match control {
        model::TopToolbarControl::ShapePicker => ToolbarEvent::ToggleShapePicker(next_active),
        model::TopToolbarControl::Utility(model::TopToolbarUtility::Highlight) => {
            ToolbarEvent::ToggleAllHighlight(next_active)
        }
        model::TopToolbarControl::Pin => ToolbarEvent::PinTopToolbar(next_active),
        model::TopToolbarControl::Overflow => ToolbarEvent::ToggleTopOverflow(next_active),
        model::TopToolbarControl::HighlightRing => {
            ToolbarEvent::ToggleHighlightToolRing(next_active)
        }
        _ => unreachable!("non-toggle control in GTK toggle adapter"),
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
        assert!(matches!(control, model::TopToolbarControl::Tool(_)));
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
        control: model::TopToolbarControl,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
    ) -> gtk4::Button {
        assert_eq!(control, model::TopToolbarControl::ShapePicker);
        let tooltip = control.tooltip(snapshot);
        let label = control.label(snapshot);
        let button = if use_icons {
            icon_button(
                top_toolbar_icon_painter(control.icon(snapshot).expect("shape-picker icon")),
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
        let expected = self.shapes_expected_open.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, event_for_toggle_state(control, !expected.get()));
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
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
            event_for_toggle_state(control, false),
        );
        let sender = self.feedback.clone();
        let expected = self.shapes_expected_open.clone();
        popover.connect_closed(move |_| {
            if expected.get() {
                send_event(&sender, event_for_toggle_state(control, false));
            }
        });
        self.shapes_popover = Some(popover);
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
            _ => unreachable!("utility adapter received non-utility control"),
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
        assert!(matches!(
            control,
            model::TopToolbarControl::Undo | model::TopToolbarControl::Redo
        ));
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
        let button = if use_icons {
            icon_button(
                top_toolbar_icon_painter(control.icon(snapshot).expect("action icon")),
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
            add_shortcut_badge(&button, badge.as_deref());
        }
        let sender = self.feedback.clone();
        if matches!(
            control,
            model::TopToolbarControl::Utility(model::TopToolbarUtility::Highlight)
        ) {
            let active = Rc::new(Cell::new(control.active(snapshot)));
            let click_active = active.clone();
            button.connect_clicked(move |_| {
                send_event(
                    &sender,
                    event_for_toggle_state(control, !click_active.get()),
                );
            });
            let handle = button.clone();
            self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                active.set(control.active(snapshot));
                set_active_class(&handle, control.active(snapshot));
                handle.set_sensitive(control.enabled(snapshot));
            }));
            return button;
        }
        let event = control.event(snapshot);
        button.connect_clicked(move |_| send_event(&sender, event.clone()));
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            set_active_class(&handle, control.active(snapshot));
            handle.set_sensitive(control.enabled(snapshot));
        }));
        button
    }

    pub(super) fn pin_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        control: model::TopToolbarControl,
        size: f64,
    ) -> gtk4::Button {
        assert_eq!(control, model::TopToolbarControl::Pin);
        let button = sized_button(size, size);
        set_control_widget_id(&button, control);
        button.add_css_class("chrome");
        let accessible_label = control.accessible_label(snapshot);
        button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        let icon = IconWidget::new(
            top_toolbar_icon_painter(control.icon(snapshot).expect("pin icon")),
            size * 0.62,
        );
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        let pinned = Rc::new(Cell::new(snapshot.top_pinned));
        let click_pinned = pinned.clone();
        button.connect_clicked(move |_| {
            send_event(
                &sender,
                event_for_toggle_state(control, !click_pinned.get()),
            );
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            pinned.set(snapshot.top_pinned);
            icon.set_painter(top_toolbar_icon_painter(
                control.icon(snapshot).expect("pin icon"),
            ));
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

    pub(super) fn overflow_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        control: model::TopToolbarControl,
        size: f64,
    ) -> gtk4::Button {
        assert_eq!(control, model::TopToolbarControl::Overflow);
        let button = sized_button(size, size);
        set_control_widget_id(&button, control);
        button.add_css_class("chrome");
        let accessible_label = control.accessible_label(snapshot);
        button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        button.set_tooltip_text(Some(&control.tooltip(snapshot)));
        let icon = IconWidget::new(
            top_toolbar_icon_painter(control.icon(snapshot).expect("overflow icon")),
            size * 0.7,
        );
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        let expected = self.overflow_expected_open.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, event_for_toggle_state(control, !expected.get()));
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
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
            event_for_toggle_state(control, false),
        );
        let sender = self.feedback.clone();
        let expected = self.overflow_expected_open.clone();
        popover.connect_closed(move |_| {
            if expected.get() {
                send_event(&sender, event_for_toggle_state(control, false));
            }
        });
        self.overflow_popover = Some(popover);
        button
    }

    pub(super) fn minimize_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        control: model::TopToolbarControl,
        size: f64,
    ) -> gtk4::Button {
        assert_eq!(control, model::TopToolbarControl::Minimize);
        let button = sized_button(size, size);
        set_control_widget_id(&button, control);
        button.add_css_class("chrome");
        button.add_css_class("minimize");
        let accessible_label = control.accessible_label(snapshot);
        button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        button.set_tooltip_text(Some(&control.tooltip(snapshot)));
        let icon = IconWidget::new(
            top_toolbar_icon_painter(control.icon(snapshot).expect("minimize icon")),
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
