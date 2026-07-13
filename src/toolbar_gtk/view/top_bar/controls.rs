//! GTK top-strip control factories.
//!
//! Owns construction and state-updater wiring for toolbar buttons and chrome.

use super::popovers::attach_escape_dismiss;
use super::*;

type UtilitySpec = (
    Action,
    crate::toolbar_gtk::icons::IconPainter,
    &'static str,
    String,
    ToolbarEvent,
    Option<fn(&ToolbarSnapshot) -> bool>,
);

impl TopBar {
    pub(super) fn tool_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        tool: Tool,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        show_badge: bool,
    ) -> gtk4::Button {
        let tooltip = tool_tooltip(snapshot, tool);
        let button = if use_icons {
            icon_button(tool_icon_painter(tool), button_size, icon_size, &tooltip).button
        } else {
            text_button(tool_label(tool), button_size, &tooltip)
        };
        if show_badge {
            add_shortcut_badge(&button, snapshot.binding_hints.badge_for_tool(tool));
        }
        let sender = self.feedback.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::SelectTool(tool));
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            let active = snapshot.active_tool == tool || snapshot.tool_override == Some(tool);
            set_active_class(&handle, active);
        }));
        button
    }

    /// Shapes picker button: the family icon opens the grid and per-tool
    /// option rows; individual shapes keep their own icons inside the popover.
    pub(super) fn shapes_picker_button(
        &mut self,
        _snapshot: &ToolbarSnapshot,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
    ) -> gtk4::Button {
        let button = if use_icons {
            icon_button(
                toolbar_icons::draw_icon_shape_picker,
                button_size,
                icon_size,
                "Shapes",
            )
            .button
        } else {
            text_button("Shapes", button_size, "Shapes")
        };
        let sender = self.feedback.clone();
        let expected = self.shapes_expected_open.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::ToggleShapePicker(!expected.get()));
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            let active = snapshot.shape_picker_open
                || model::current_shape_tool(snapshot.active_tool, snapshot.tool_override)
                    .is_some();
            set_active_class(&handle, active);
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
            ToolbarEvent::ToggleShapePicker(false),
        );
        let sender = self.feedback.clone();
        let expected = self.shapes_expected_open.clone();
        popover.connect_closed(move |_| {
            if expected.get() {
                send_event(&sender, ToolbarEvent::ToggleShapePicker(false));
            }
        });
        self.shapes_popover = Some(popover);
        button
    }

    pub(super) fn utility_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        utility: model::TopUtilityButton,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        show_badge: bool,
    ) -> Option<gtk4::Button> {
        let (action, painter, label, tooltip, event, active): UtilitySpec = match utility {
            model::TopUtilityButton::Text => (
                Action::EnterTextMode,
                toolbar_icons::draw_icon_text,
                action_short_label(Action::EnterTextMode),
                action_tooltip(snapshot, Action::EnterTextMode),
                ToolbarEvent::EnterTextMode,
                Some(|snapshot| snapshot.text_active),
            ),
            model::TopUtilityButton::StickyNote => (
                Action::EnterStickyNoteMode,
                toolbar_icons::draw_icon_note,
                action_short_label(Action::EnterStickyNoteMode),
                action_tooltip(snapshot, Action::EnterStickyNoteMode),
                ToolbarEvent::EnterStickyNoteMode,
                Some(|snapshot| snapshot.note_active),
            ),
            model::TopUtilityButton::Screenshot => (
                Action::CaptureSelection,
                toolbar_icons::draw_icon_screenshot,
                "Shot",
                action_tooltip(snapshot, Action::CaptureSelection),
                ToolbarEvent::CaptureScreenshot,
                None,
            ),
            model::TopUtilityButton::Highlight => (
                Action::ToggleHighlightTool,
                toolbar_icons::draw_icon_highlight,
                "Highlight",
                action_tooltip(snapshot, Action::ToggleHighlightTool),
                // The click handler recomputes the toggle from live state.
                ToolbarEvent::ToggleAllHighlight(true),
                Some(|snapshot| snapshot.any_highlight_active),
            ),
            model::TopUtilityButton::ClearCanvas | model::TopUtilityButton::IconMode => {
                return None;
            }
        };
        let button = if use_icons {
            icon_button(painter, button_size, icon_size, &tooltip).button
        } else {
            text_button(label, button_size, &tooltip)
        };
        if show_badge {
            add_shortcut_badge(&button, snapshot.binding_hints.badge_for_action(action));
        }
        let sender = self.feedback.clone();
        if utility == model::TopUtilityButton::Highlight {
            // Highlight toggles off the *current* state rather than firing a
            // fixed event.
            let active_state = Rc::new(Cell::new(snapshot.any_highlight_active));
            let click_state = active_state.clone();
            button.connect_clicked(move |_| {
                send_event(
                    &sender,
                    ToolbarEvent::ToggleAllHighlight(!click_state.get()),
                );
            });
            let handle = button.clone();
            self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                active_state.set(snapshot.any_highlight_active);
                set_active_class(&handle, snapshot.any_highlight_active);
            }));
        } else {
            button.connect_clicked(move |_| {
                send_event(&sender, event.clone());
            });
            if let Some(is_active) = active {
                let handle = button.clone();
                self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                    set_active_class(&handle, is_active(snapshot));
                }));
            }
        }
        Some(button)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn history_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        painter: crate::toolbar_gtk::icons::IconPainter,
        action: Action,
        event: ToolbarEvent,
        available: fn(&ToolbarSnapshot) -> bool,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        show_badge: bool,
    ) -> gtk4::Button {
        let tooltip = action_tooltip(snapshot, action);
        let button = if use_icons {
            icon_button(painter, button_size, icon_size, &tooltip).button
        } else {
            text_button(action_short_label(action), button_size, &tooltip)
        };
        if show_badge {
            add_shortcut_badge(&button, snapshot.binding_hints.badge_for_action(action));
        }
        let sender = self.feedback.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            handle.set_sensitive(available(snapshot));
        }));
        button
    }

    pub(super) fn pin_button(&mut self, snapshot: &ToolbarSnapshot, size: f64) -> gtk4::Button {
        let button = sized_button(size, size);
        button.add_css_class("chrome");
        let icon = IconWidget::new(
            if snapshot.top_pinned {
                toolbar_icons::draw_icon_pin
            } else {
                toolbar_icons::draw_icon_unpin
            },
            size * 0.62,
        );
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        let pinned = Rc::new(Cell::new(snapshot.top_pinned));
        let click_pinned = pinned.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::PinTopToolbar(!click_pinned.get()));
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            pinned.set(snapshot.top_pinned);
            icon.set_painter(if snapshot.top_pinned {
                toolbar_icons::draw_icon_pin
            } else {
                toolbar_icons::draw_icon_unpin
            });
            if snapshot.top_pinned {
                handle.add_css_class("pinned");
                handle.set_tooltip_text(Some("Pinned: opens at startup (click to disable)"));
            } else {
                handle.remove_css_class("pinned");
                handle.set_tooltip_text(Some("Pin: click to open at startup"));
            }
        }));
        button
    }

    pub(super) fn overflow_button(
        &mut self,
        snapshot: &ToolbarSnapshot,
        _plan: &TopStripPlan,
        size: f64,
        _use_icons: bool,
    ) -> gtk4::Button {
        let button = sized_button(size, size);
        button.add_css_class("chrome");
        button.set_tooltip_text(Some("More tools"));
        let icon = IconWidget::new(toolbar_icons::draw_icon_more, size * 0.7);
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        let expected = self.overflow_expected_open.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::ToggleTopOverflow(!expected.get()));
        });
        let handle = button.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            set_active_class(&handle, snapshot.top_overflow_open);
        }));
        let _ = snapshot;

        let popover = gtk4::Popover::new();
        popover.set_parent(&button);
        popover.set_position(gtk4::PositionType::Bottom);
        // See the shapes popover: dismissal stays with the backend policy.
        popover.set_autohide(false);
        attach_escape_dismiss(
            &popover,
            &self.feedback,
            ToolbarEvent::ToggleTopOverflow(false),
        );
        let sender = self.feedback.clone();
        let expected = self.overflow_expected_open.clone();
        popover.connect_closed(move |_| {
            if expected.get() {
                send_event(&sender, ToolbarEvent::ToggleTopOverflow(false));
            }
        });
        self.overflow_popover = Some(popover);
        button
    }

    pub(super) fn minimize_button(&mut self, size: f64) -> gtk4::Button {
        let button = sized_button(size, size);
        button.add_css_class("chrome");
        button.add_css_class("minimize");
        button.set_tooltip_text(Some("Minimize (leaves a restore tab)"));
        let icon = IconWidget::new(toolbar_icons::draw_icon_minimize, size * 0.6);
        button.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::SetTopMinimized(true));
        });
        button
    }
}
