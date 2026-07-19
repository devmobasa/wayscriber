//! GTK top-strip popover lifecycle and content.
//!
//! Keeps shapes and overflow popovers synchronized without rebuilding content
//! on unrelated snapshot changes.

use super::*;
use crate::toolbar_gtk::css::CAPTURE_TRANSPARENT_CLASS;

pub(super) fn set_popover_capture_transparent(
    popover: &gtk4::Popover,
    capture_surface: &CaptureSurfaceContent,
    transparent: bool,
    input_enabled: bool,
) {
    if transparent {
        popover.add_css_class(CAPTURE_TRANSPARENT_CLASS);
    } else {
        popover.remove_css_class(CAPTURE_TRANSPARENT_CLASS);
    }
    capture_surface.set_transparent(transparent);
    set_popover_input_enabled(popover, input_enabled);
}

fn set_popover_input_enabled(popover: &gtk4::Popover, enabled: bool) {
    popover.set_can_target(enabled);
    if let Some(surface) = popover.surface() {
        if enabled {
            surface.set_input_region(None);
        } else {
            let empty = gtk4::cairo::Region::create();
            surface.set_input_region(Some(&empty));
        }
    }
    popover.queue_draw();
}

impl TopBar {
    pub(super) fn hide_popovers_for_window_hide(&self) {
        self.shapes_expected_open.set(false);
        self.overflow_expected_open.set(false);
        self.session_expected_open.set(false);
        self.settings_expected_open.set(false);
        for popover in [
            self.shapes_popover.as_ref(),
            self.overflow_popover.as_ref(),
            self.session_popover.as_ref(),
            self.settings_popover.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if popover.is_visible() {
                popover.popdown();
            }
        }
    }

    /// Popovers are independent native Wayland surfaces, so their parent's
    /// opacity does not affect them. Keep any open popover mapped and replace
    /// its content with the same transparent proof node as the toolbar rather
    /// than starting a popup close animation during capture.
    pub(super) fn set_popovers_capture_transparent(&self, transparent: bool) {
        for (popover, capture_surface) in [
            (
                self.shapes_popover.as_ref(),
                self.shapes_capture_surface.as_ref(),
            ),
            (
                self.overflow_popover.as_ref(),
                self.overflow_capture_surface.as_ref(),
            ),
            (
                self.session_popover.as_ref(),
                self.session_capture_surface.as_ref(),
            ),
            (
                self.settings_popover.as_ref(),
                self.settings_capture_surface.as_ref(),
            ),
        ] {
            let (Some(popover), Some(capture_surface)) = (popover, capture_surface) else {
                continue;
            };
            if transparent && !popover.is_visible() {
                continue;
            }
            set_popover_capture_transparent(popover, capture_surface, transparent, !transparent);
        }
    }

    pub(in crate::toolbar_gtk::view) fn tooltip_roots(&self) -> Vec<gtk4::Widget> {
        [
            self.shapes_popover.as_ref(),
            self.overflow_popover.as_ref(),
            self.session_popover.as_ref(),
            self.settings_popover.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(|popover| popover.clone().upcast::<gtk4::Widget>())
        .collect()
    }

    pub(in crate::toolbar_gtk::view) fn capture_popover_targets(&self) -> Vec<CaptureProofTarget> {
        [
            (
                "top-shapes-popover",
                self.shapes_popover.as_ref(),
                self.shapes_capture_surface.as_ref(),
            ),
            (
                "top-overflow-popover",
                self.overflow_popover.as_ref(),
                self.overflow_capture_surface.as_ref(),
            ),
            (
                "top-session-popover",
                self.session_popover.as_ref(),
                self.session_capture_surface.as_ref(),
            ),
            (
                "top-settings-popover",
                self.settings_popover.as_ref(),
                self.settings_capture_surface.as_ref(),
            ),
        ]
        .into_iter()
        .filter_map(|(name, popover, capture_surface)| {
            let popover = popover?;
            let capture_surface = capture_surface?;
            (popover.is_visible() && popover.is_mapped())
                .then(|| CaptureProofTarget::new_withdrawable(name, popover, capture_surface))
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
            let open = snapshot.shape_picker_open;
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
                    self.shapes_capture_surface
                        .as_ref()
                        .expect("shapes popover capture surface")
                        .set_content(&self.build_shapes_popover_content(
                            snapshot,
                            button_size,
                            icon_size,
                            use_icons,
                            scale,
                        ));
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
                && model::TopToolbarSpec::overflow_control_count(snapshot, plan) > 0;
            if open {
                let content_key = (
                    snapshot.active_tool,
                    snapshot.tool_override,
                    snapshot.text_active,
                    snapshot.note_active,
                    snapshot.any_highlight_active,
                    snapshot.session_popover_open,
                    snapshot.settings_popover_open,
                );
                if self.overflow_content_key.get() != Some(content_key) {
                    let spec = model::TopToolbarSpec::build(snapshot, plan);
                    self.overflow_capture_surface
                        .as_ref()
                        .expect("overflow popover capture surface")
                        .set_content(&self.build_overflow_popover_content(
                            snapshot,
                            &spec,
                            button_size,
                            icon_size,
                            use_icons,
                            scale,
                        ));
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

        self.sync_session_settings_popovers(snapshot, scale);
    }

    /// Keep the Session/Settings popovers' contents and open state in line
    /// with the snapshot, mirroring the shapes/overflow pattern: content
    /// rebuilds only when the model's snapshot inputs change.
    fn sync_session_settings_popovers(&mut self, snapshot: &ToolbarSnapshot, scale: f64) {
        if let Some(popover) = self.session_popover.clone() {
            let open = snapshot.session_popover_open;
            if open {
                let content_key = SessionMenuContentKey::of(snapshot);
                if self.session_content_key.borrow().as_ref() != Some(&content_key) {
                    self.session_capture_surface
                        .as_ref()
                        .expect("session popover capture surface")
                        .set_content(&self.build_session_popover_content(snapshot, scale));
                    *self.session_content_key.borrow_mut() = Some(content_key);
                }
            }
            self.session_expected_open.set(open);
            if open && !popover.is_visible() {
                popover.popup();
            } else if !open && popover.is_visible() {
                popover.popdown();
            }
        } else {
            self.session_expected_open.set(false);
        }

        if let Some(popover) = self.settings_popover.clone() {
            let open = snapshot.settings_popover_open;
            if open {
                let content_key = SettingsMenuContentKey::of(snapshot);
                if self.settings_content_key.borrow().as_ref() != Some(&content_key) {
                    self.settings_capture_surface
                        .as_ref()
                        .expect("settings popover capture surface")
                        .set_content(&self.build_settings_popover_content(snapshot, scale));
                    *self.settings_content_key.borrow_mut() = Some(content_key);
                }
            }
            self.settings_expected_open.set(open);
            if open && !popover.is_visible() {
                popover.popup();
            } else if !open && popover.is_visible() {
                popover.popdown();
            }
        } else {
            self.settings_expected_open.set(false);
        }
    }

    /// Session popover content: the Session pane's rows (meta labels,
    /// action grid or Save-As confirmation, recents) inside a scrollable
    /// viewport. Updaters register into a scratch list — content-key
    /// rebuilds carry the live state instead.
    pub(super) fn build_session_popover_content(
        &self,
        snapshot: &ToolbarSnapshot,
        scale: f64,
    ) -> gtk4::Widget {
        let mut scratch_updaters = Vec::new();
        let mut ctx = super::super::sections::SectionCtx {
            snapshot,
            feedback: self.feedback.clone(),
            scale,
            use_icons: snapshot.use_icons,
            updaters: &mut scratch_updaters,
        };
        let content = match model::ToolbarSessionModel::for_popover(snapshot) {
            Some(session) => {
                super::super::sections::session_pane::build_popover_content(&mut ctx, &session)
            }
            None => gtk4::Box::new(gtk4::Orientation::Vertical, 0),
        };
        menu_popover_viewport(&content, "top.menu.session.panel", scale, &self.feedback)
    }

    /// Settings popover content: the Settings pane's control set (layout
    /// mode, toggles, buttons, customization sub-panel) inside a scrollable
    /// viewport.
    pub(super) fn build_settings_popover_content(
        &self,
        snapshot: &ToolbarSnapshot,
        scale: f64,
    ) -> gtk4::Widget {
        let mut scratch_updaters = Vec::new();
        let mut ctx = super::super::sections::SectionCtx {
            snapshot,
            feedback: self.feedback.clone(),
            scale,
            use_icons: snapshot.use_icons,
            updaters: &mut scratch_updaters,
        };
        let content = match model::ToolbarSettingsModel::for_popover(snapshot) {
            Some(settings) => {
                super::super::sections::settings_pane::build_popover_content(&mut ctx, &settings)
            }
            None => gtk4::Box::new(gtk4::Orientation::Vertical, 0),
        };
        menu_popover_viewport(&content, "top.menu.settings.panel", scale, &self.feedback)
    }

    pub(super) fn build_shapes_popover_content(
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
        set_semantic_widget_id(&content, "top.shapes.panel");

        for row in model::visible_shape_picker_rows(snapshot, is_simple) {
            let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, gap);
            for tool in row {
                if !model::tool_visible(snapshot, tool) {
                    continue;
                }
                let control = model::TopToolbarControl::Tool(tool);
                let tooltip = control.tooltip(snapshot);
                let label = control.label(snapshot);
                let button = if use_icons {
                    icon_button(
                        top_toolbar_icon_painter(control.icon(snapshot).expect("tool icon")),
                        button_size,
                        icon_size,
                        &tooltip,
                    )
                    .button
                } else {
                    text_button(&label, button_size, &tooltip)
                };
                set_prefixed_control_widget_id(&button, "top.picker.", control);
                let accessible_label = control.accessible_label(snapshot);
                button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
                let badge = control.shortcut_badge(snapshot);
                add_button_shortcut_hint(&button, badge.as_deref(), use_icons);
                set_active_class(&button, control.active(snapshot));
                let sender = self.feedback.clone();
                let event = control.event(snapshot);
                button.connect_clicked(move |_| {
                    send_event(&sender, event.clone());
                });
                row_box.append(&button);
            }
            content.append(&row_box);
        }

        // Option rows: Fill and polygon sides live inside the popover, so
        // using them must not close it (GTK popovers keep inside clicks).
        if model::top_fill_visible(snapshot) {
            let fill = gtk4::CheckButton::with_label(action_short_label(Action::ToggleFill));
            set_semantic_widget_id(
                &fill,
                crate::config::toolbar_item_ids::TOP_UTILITY_FILL.as_str(),
            );
            fill.set_tooltip_text(Some(&model::action_tooltip(snapshot, Action::ToggleFill)));
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
            set_semantic_widget_id(&minus, "top.options.sides-minus");
            let sender = self.feedback.clone();
            minus.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::NudgePolygonSides(-1));
            });
            let label = gtk4::Label::new(Some(&format!("{} sides", snapshot.polygon_sides)));
            label.set_hexpand(true);
            let plus = text_button("+", side_button, "More sides");
            set_semantic_widget_id(&plus, "top.options.sides-plus");
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
    pub(super) fn build_overflow_popover_content(
        &self,
        snapshot: &ToolbarSnapshot,
        spec: &model::TopToolbarSpec,
        button_size: (f64, f64),
        icon_size: f64,
        use_icons: bool,
        scale: f64,
    ) -> gtk4::Grid {
        let gap = (GAP * scale).round() as i32;
        let grid = gtk4::Grid::new();
        set_semantic_widget_id(&grid, "top.overflow.panel");
        grid.set_row_spacing(gap as u32);
        grid.set_column_spacing(gap as u32);
        // The popover is its own GTK native; the window-level capture never
        // sees its clicks, so Shift (instant Clear) and the rebind chord are
        // captured here.
        install_click_modifier_capture(&grid, &self.feedback);
        let dropped_count = spec.overflow().len();
        let cols = dropped_count.clamp(1, 5) as i32;
        let mut index = 0i32;
        let mut attach = |widget: &gtk4::Button| {
            grid.attach(widget, index % cols, index / cols, 1, 1);
            index += 1;
        };
        for control in spec.overflow().iter().copied() {
            let tooltip = control.overflow_tooltip(snapshot);
            let label = control.label(snapshot);
            let button = if use_icons {
                icon_button(
                    top_toolbar_icon_painter(control.icon(snapshot).expect("overflow icon")),
                    button_size,
                    icon_size,
                    &tooltip,
                )
                .button
            } else {
                text_button(&label, button_size, &tooltip)
            };
            set_prefixed_control_widget_id(&button, "top.overflow.", control);
            let accessible_label = control.accessible_label(snapshot);
            button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
            if button_size.0 > COMPACT_BUTTON * scale {
                let badge = control.shortcut_badge(snapshot);
                add_button_shortcut_hint(&button, badge.as_deref(), use_icons);
            }
            if control.role() == model::TopToolbarControlRole::Destructive {
                // Clear inside the menu keeps the red-on-hover treatment.
                button.add_css_class("destructive");
            }
            set_active_class(&button, control.active(snapshot));
            button.set_sensitive(control.enabled(snapshot));
            let sender = self.feedback.clone();
            let event = control.event(snapshot);
            button.connect_clicked(move |_| {
                send_event(&sender, event.clone());
            });
            attach(&button);
        }
        grid
    }
}

/// Wrap Session/Settings popover content in a capped, scrollable viewport
/// (the GTK counterpart of the builtin popovers' internal scrollbar) and
/// install the click-time modifier capture the popover's own GTK native
/// needs for the rebind chord.
fn menu_popover_viewport(
    content: &gtk4::Box,
    semantic_id: &str,
    scale: f64,
    feedback: &FeedbackSender,
) -> gtk4::Widget {
    set_semantic_widget_id(content, semantic_id);
    content.set_size_request((MENU_CONTENT_W * scale).round() as i32, -1);
    install_click_modifier_capture(content, feedback);
    let scroller = gtk4::ScrolledWindow::new();
    scroller.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroller.set_propagate_natural_width(true);
    scroller.set_propagate_natural_height(true);
    scroller.set_max_content_height((MENU_MAX_CONTENT_H * scale).round() as i32);
    scroller.set_child(Some(content));
    scroller.upcast()
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
