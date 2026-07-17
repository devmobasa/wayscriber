//! GTK top-strip popover lifecycle and content.
//!
//! Keeps shapes and overflow popovers synchronized without rebuilding content
//! on unrelated snapshot changes.

use super::*;

impl TopBar {
    pub(super) fn hide_popovers_for_window_hide(&self) {
        self.shapes_expected_open.set(false);
        self.overflow_expected_open.set(false);
        if let Some(popover) = self.shapes_popover.as_ref()
            && popover.is_visible()
        {
            popover.popdown();
        }
        if let Some(popover) = self.overflow_popover.as_ref()
            && popover.is_visible()
        {
            popover.popdown();
        }
    }

    /// Popovers are independent native Wayland surfaces, so their parent's
    /// opacity does not affect them. Keep any open popover mapped and commit
    /// opacity zero alongside the top toolbar rather than starting a popup
    /// close animation during capture.
    pub(super) fn set_popovers_capture_transparent(&self, transparent: bool) {
        for popover in [self.shapes_popover.as_ref(), self.overflow_popover.as_ref()]
            .into_iter()
            .flatten()
        {
            if transparent && !popover.is_visible() {
                continue;
            }
            super::super::set_capture_transparent(popover, transparent);
            popover.set_can_target(!transparent);
            if let Some(surface) = popover.surface() {
                if transparent {
                    let empty = gtk4::cairo::Region::create();
                    surface.set_input_region(Some(&empty));
                } else {
                    surface.set_input_region(None);
                }
            }
        }
    }

    pub(in crate::toolbar_gtk::view) fn capture_popover_targets(
        &self,
    ) -> Vec<(&'static str, gtk4::Widget)> {
        [
            ("top-shapes-popover", self.shapes_popover.as_ref()),
            ("top-overflow-popover", self.overflow_popover.as_ref()),
        ]
        .into_iter()
        .filter_map(|(name, popover)| {
            let popover = popover?;
            (popover.is_visible() && popover.is_mapped())
                .then(|| (name, popover.clone().upcast::<gtk4::Widget>()))
        })
        .collect()
    }

    /// Keep the popovers' contents and open state in line with the snapshot.
    /// Open state only changes when the snapshot flag differs from what the
    /// popover shows.
    pub(super) fn sync_popovers(&mut self, snapshot: &ToolbarSnapshot, plan: &TopStripPlan) {
        let scale = effective_scale(snapshot);
        let use_icons = snapshot.use_icons || plan.compact;
        let (btn_w, btn_h) = if plan.compact {
            (COMPACT_BUTTON, COMPACT_BUTTON)
        } else if use_icons {
            (ICON_BUTTON, ICON_BUTTON)
        } else {
            (TEXT_BUTTON_W, TEXT_BUTTON_H)
        };
        let button_size = (btn_w * scale, btn_h * scale);
        let icon_size = ICON_SIZE * scale;

        if let Some(popover) = self.shapes_popover.clone() {
            let open = snapshot.shape_picker_open && model::top_shape_picker_visible(snapshot);
            if open {
                // Only rebuild the grid when its inputs changed; a rebuild
                // resets hover and cancels an in-flight press.
                let content_key = (
                    snapshot.active_tool,
                    snapshot.tool_override,
                    snapshot.fill_enabled,
                    snapshot.polygon_sides,
                );
                if self.shapes_content_key.get() != Some(content_key) {
                    popover.set_child(Some(&self.build_shapes_popover_content(
                        snapshot,
                        button_size,
                        icon_size,
                        use_icons,
                        scale,
                    )));
                    self.shapes_content_key.set(Some(content_key));
                }
            }
            self.shapes_expected_open.set(open);
            if open && !popover.is_visible() {
                popover.popup();
            } else if !open && popover.is_visible() {
                popover.popdown();
            }
        }

        if let Some(popover) = self.overflow_popover.clone() {
            let open = snapshot.top_overflow_open
                && plan.dropped_tools.len() + plan.dropped_utilities.len() > 0;
            if open {
                let content_key = (
                    snapshot.active_tool,
                    snapshot.tool_override,
                    snapshot.text_active,
                    snapshot.note_active,
                    snapshot.any_highlight_active,
                );
                if self.overflow_content_key.get() != Some(content_key) {
                    popover.set_child(Some(&self.build_overflow_popover_content(
                        snapshot,
                        plan,
                        button_size,
                        icon_size,
                        use_icons,
                        scale,
                    )));
                    self.overflow_content_key.set(Some(content_key));
                }
            }
            self.overflow_expected_open.set(open);
            if open && !popover.is_visible() {
                popover.popup();
            } else if !open && popover.is_visible() {
                popover.popdown();
            }
        } else {
            self.overflow_expected_open.set(false);
        }
    }

    fn build_shapes_popover_content(
        &self,
        snapshot: &ToolbarSnapshot,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        scale: f64,
    ) -> gtk4::Box {
        let is_simple = snapshot.layout_mode == ToolbarLayoutMode::Simple;
        let gap = (GAP * scale).round() as i32;
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, gap);

        for row in model::visible_shape_picker_rows(snapshot, is_simple) {
            let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, gap);
            for tool in row {
                if !model::tool_visible(snapshot, tool) {
                    continue;
                }
                let tooltip = tool_tooltip(snapshot, tool);
                let button = if use_icons {
                    icon_button(tool_icon_painter(tool), button_size, icon_size, &tooltip).button
                } else {
                    text_button(tool_label(tool), button_size, &tooltip)
                };
                add_shortcut_badge(&button, snapshot.binding_hints.badge_for_tool(tool));
                set_active_class(
                    &button,
                    snapshot.active_tool == tool || snapshot.tool_override == Some(tool),
                );
                let sender = self.feedback.clone();
                button.connect_clicked(move |_| {
                    send_event(&sender, ToolbarEvent::SelectTool(tool));
                });
                row_box.append(&button);
            }
            content.append(&row_box);
        }

        // Option rows: Fill and polygon sides live inside the popover, so
        // using them must not close it (GTK popovers keep inside clicks).
        let fill_tool_active =
            model::fill_tool_active(snapshot.active_tool, snapshot.tool_override);
        if fill_tool_active && model::top_fill_visible(snapshot) {
            let fill = gtk4::CheckButton::with_label(action_short_label(Action::ToggleFill));
            fill.set_tooltip_text(Some(&action_tooltip(snapshot, Action::ToggleFill)));
            // set_active runs before connect_toggled, so every later
            // toggle is user input and forwards unconditionally.
            fill.set_active(snapshot.fill_enabled);
            let sender = self.feedback.clone();
            fill.connect_toggled(move |check| {
                send_event(&sender, ToolbarEvent::ToggleFill(check.is_active()));
            });
            content.append(&fill);
        }
        if snapshot.active_tool == Tool::RegularPolygon
            || snapshot.tool_override == Some(Tool::RegularPolygon)
        {
            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, gap);
            let side_button = (24.0 * scale, 24.0 * scale);
            let minus = text_button("−", side_button, "Fewer sides");
            let sender = self.feedback.clone();
            minus.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::NudgePolygonSides(-1));
            });
            let label = gtk4::Label::new(Some(&format!("{} sides", snapshot.polygon_sides)));
            label.set_hexpand(true);
            let plus = text_button("+", side_button, "More sides");
            let sender = self.feedback.clone();
            plus.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::NudgePolygonSides(1));
            });
            row.append(&minus);
            row.append(&label);
            row.append(&plus);
            row.set_size_request((160.0 * scale).round() as i32, -1);
            content.append(&row);
        }
        content
    }

    #[allow(clippy::too_many_arguments)]
    fn build_overflow_popover_content(
        &self,
        snapshot: &ToolbarSnapshot,
        plan: &TopStripPlan,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        scale: f64,
    ) -> gtk4::Grid {
        let gap = (GAP * scale).round() as i32;
        let grid = gtk4::Grid::new();
        grid.set_row_spacing(gap as u32);
        grid.set_column_spacing(gap as u32);
        let dropped_count = plan.dropped_tools.len() + plan.dropped_utilities.len();
        let cols = dropped_count.clamp(1, 5) as i32;
        let mut index = 0i32;
        let mut attach = |widget: &gtk4::Button| {
            grid.attach(widget, index % cols, index / cols, 1, 1);
            index += 1;
        };
        for tool in &plan.dropped_tools {
            let tool = *tool;
            let tooltip = tool_tooltip(snapshot, tool);
            let button = if use_icons {
                icon_button(tool_icon_painter(tool), button_size, icon_size, &tooltip).button
            } else {
                text_button(tool_label(tool), button_size, &tooltip)
            };
            add_shortcut_badge(&button, snapshot.binding_hints.badge_for_tool(tool));
            set_active_class(
                &button,
                snapshot.active_tool == tool || snapshot.tool_override == Some(tool),
            );
            let sender = self.feedback.clone();
            button.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::SelectTool(tool));
            });
            attach(&button);
        }
        for utility in &plan.dropped_utilities {
            let (action, painter, label, event, active): (
                Action,
                crate::toolbar_gtk::icons::IconPainter,
                &str,
                ToolbarEvent,
                bool,
            ) = match utility {
                model::TopUtilityButton::Text => (
                    Action::EnterTextMode,
                    toolbar_icons::draw_icon_text,
                    action_short_label(Action::EnterTextMode),
                    ToolbarEvent::EnterTextMode,
                    snapshot.text_active,
                ),
                model::TopUtilityButton::StickyNote => (
                    Action::EnterStickyNoteMode,
                    toolbar_icons::draw_icon_note,
                    action_short_label(Action::EnterStickyNoteMode),
                    ToolbarEvent::EnterStickyNoteMode,
                    snapshot.note_active,
                ),
                model::TopUtilityButton::Screenshot => (
                    Action::CaptureSelection,
                    toolbar_icons::draw_icon_screenshot,
                    "Shot",
                    ToolbarEvent::CaptureScreenshot,
                    false,
                ),
                model::TopUtilityButton::Highlight => (
                    Action::ToggleHighlightTool,
                    toolbar_icons::draw_icon_highlight,
                    "Highlight",
                    ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active),
                    snapshot.any_highlight_active,
                ),
                model::TopUtilityButton::ClearCanvas | model::TopUtilityButton::IconMode => {
                    continue;
                }
            };
            let tooltip = action_tooltip(snapshot, action);
            let button = if use_icons {
                icon_button(painter, button_size, icon_size, &tooltip).button
            } else {
                text_button(label, button_size, &tooltip)
            };
            add_shortcut_badge(&button, snapshot.binding_hints.badge_for_action(action));
            set_active_class(&button, active);
            let sender = self.feedback.clone();
            button.connect_clicked(move |_| {
                send_event(&sender, event.clone());
            });
            attach(&button);
        }
        grid
    }
}

/// Without an autohide grab, Escape needs explicit wiring to dismiss an
/// open popover through the backend state.
pub(super) fn attach_escape_dismiss(
    popover: &gtk4::Popover,
    feedback: &FeedbackSender,
    dismiss: ToolbarEvent,
) {
    let key = gtk4::EventControllerKey::new();
    let sender = feedback.clone();
    key.connect_key_pressed(move |_, keyval, _, _| {
        if keyval == gtk4::gdk::Key::Escape {
            send_event(&sender, dismiss.clone());
            return gtk4::glib::Propagation::Stop;
        }
        gtk4::glib::Propagation::Proceed
    });
    popover.add_controller(key);
}
