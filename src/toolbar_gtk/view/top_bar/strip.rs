//! GTK top-strip assembly.
//!
//! Owns the ordered strip layout and width-degradation presentation while
//! delegating individual controls to the control factory methods.

use super::*;

impl TopBar {
    pub(super) fn build_minimized(&mut self, snapshot: &ToolbarSnapshot) {
        let scale = effective_scale(snapshot);
        // A GTK toplevel never shrinks on its own; reset the default size
        // or the tab keeps the full strip's width. The panel padding is
        // dropped so the tab hugs the 64x24 builtin footprint.
        self.window.set_default_size(
            (MINIMIZED_SIZE.0 * scale).round() as i32,
            (MINIMIZED_SIZE.1 * scale).round() as i32,
        );
        self.root.add_css_class("minimized");
        let restore = sized_button(MINIMIZED_SIZE.0 * scale, MINIMIZED_SIZE.1 * scale);
        restore.add_css_class("chrome");
        restore.set_tooltip_text(Some("Show toolbar"));
        let icon = IconWidget::new(
            toolbar_icons::draw_icon_restore,
            (MINIMIZED_SIZE.1 * 0.75 * scale).min(18.0 * scale),
        );
        restore.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        restore.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::SetTopMinimized(false));
        });
        self.root.append(&restore);
    }

    // The running spec-unit `x` mirrors the builtin builder walk; the last
    // increments are intentionally kept even where nothing reads them so
    // the two walks stay line-for-line comparable.
    #[allow(unused_assignments)]
    pub(super) fn build_strip(&mut self, snapshot: &ToolbarSnapshot, plan: &TopStripPlan) {
        self.root.remove_css_class("minimized");
        // GTK toplevels retain their previous default width across widget-tree
        // rebuilds. Reset it from the shared natural-size calculation so a
        // narrower layout (notably `simple`) does not keep the regular strip's
        // empty trailing area. Height remains content-driven for GTK popovers.
        self.window
            .set_default_size(top_default_width(snapshot), -1);
        let scale = effective_scale(snapshot);
        let is_simple = snapshot.layout_mode == ToolbarLayoutMode::Simple;
        let use_icons = snapshot.use_icons || plan.compact;
        let gap = if plan.compact { COMPACT_GAP } else { GAP };
        let (btn_w, btn_h) = if plan.compact {
            (COMPACT_BUTTON, COMPACT_BUTTON)
        } else if use_icons {
            (ICON_BUTTON, ICON_BUTTON)
        } else {
            (TEXT_BUTTON_W, TEXT_BUTTON_H)
        };
        let sz = |value: f64| value * scale;
        let px = |value: f64| (value * scale).round() as i32;

        let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        bar.set_margin_start(px(if plan.compact {
            COMPACT_START_X
        } else {
            START_X
        }));
        bar.set_margin_end(0);
        self.root.append(&bar);

        // Running spec-unit x, mirroring the builtin builder walk; used to
        // align the contextual ring row under the Highlight button.
        let mut x = if plan.compact {
            COMPACT_START_X
        } else {
            START_X
        };
        let mut highlight_x: Option<f64> = None;

        let append_gap = |bar: &gtk4::Box, widget: &gtk4::Widget, gap_units: f64| {
            widget.set_margin_end(px(gap_units).max(0));
            bar.append(widget);
        };
        let push_divider = |bar: &gtk4::Box, x: &mut f64| {
            let span = if plan.compact { 3.0 } else { DIVIDER_SPAN };
            let divider = gtk4::Separator::new(gtk4::Orientation::Vertical);
            divider.set_margin_top(px(6.0));
            divider.set_margin_bottom(px(6.0));
            divider.set_margin_start(px((span - 1.0) / 2.0));
            divider.set_margin_end(px((span - 1.0) / 2.0) + px(gap));
            bar.append(&divider);
            *x += span + gap;
        };

        // --- Drag grip -----------------------------------------------------
        if model::toolbar_item_visible(snapshot, crate::config::toolbar_item_ids::TOP_CHROME_DRAG) {
            let grip = IconWidget::new(toolbar_icons::draw_icon_drag, sz(HANDLE_SIZE));
            grip.area.set_can_target(true);
            grip.area.add_css_class("drag-handle");
            grip.area.set_tooltip_text(Some("Drag toolbar"));
            grip.area.set_valign(gtk4::Align::Center);
            grip.area.set_cursor_from_name(Some("grab"));
            self.attach_move_drag(&grip.area);
            append_gap(&bar, grip.area.as_ref(), gap);
            x += HANDLE_SIZE + gap;
        }

        // --- Tool groups: pens | shapes --------------------------------------
        let mut previous_group: Option<model::TopToolGroup> = None;
        let mut tool_drawn = false;
        for tool in model::visible_top_tool_buttons(is_simple, snapshot) {
            if plan.dropped_tools.contains(&tool) {
                continue;
            }
            let group = model::top_tool_group(tool);
            if let Some(previous) = previous_group
                && previous != group
            {
                push_divider(&bar, &mut x);
            }
            previous_group = Some(group);
            let button = self.tool_button(
                snapshot,
                tool,
                (sz(btn_w), sz(btn_h)),
                sz(ICON_SIZE),
                use_icons,
                !plan.compact,
            );
            append_gap(&bar, button.as_ref(), gap);
            x += btn_w + gap;
            tool_drawn = true;
        }

        // --- Shapes picker ----------------------------------------------------
        if model::top_shape_picker_visible(snapshot) {
            if previous_group == Some(model::TopToolGroup::Pens) {
                push_divider(&bar, &mut x);
            }
            let button = self.shapes_picker_button(
                snapshot,
                (sz(btn_w), sz(btn_h)),
                sz(ICON_SIZE),
                use_icons,
            );
            append_gap(&bar, button.as_ref(), gap);
            x += btn_w + gap;
            tool_drawn = true;
        }

        // --- Annotation utilities (Clear pulled out below) --------------------
        let utilities: Vec<model::TopUtilityButton> =
            model::visible_top_utility_buttons(snapshot, is_simple, snapshot.use_icons)
                .into_iter()
                .filter(|button| *button != model::TopUtilityButton::ClearCanvas)
                .filter(|button| !plan.dropped_utilities.contains(button))
                .collect();
        let clear_visible =
            model::visible_top_utility_buttons(snapshot, is_simple, snapshot.use_icons)
                .contains(&model::TopUtilityButton::ClearCanvas);
        if !utilities.is_empty() && tool_drawn {
            push_divider(&bar, &mut x);
        }
        for utility in utilities {
            if utility == model::TopUtilityButton::Highlight {
                highlight_x = Some(x);
            }
            if let Some(button) = self.utility_button(
                snapshot,
                utility,
                (sz(btn_w), sz(btn_h)),
                sz(ICON_SIZE),
                use_icons,
                !plan.compact,
            ) {
                append_gap(&bar, button.as_ref(), gap);
                x += btn_w + gap;
            }
        }

        // --- Quick colors + current-color chip --------------------------------
        if model::toolbar_item_visible(
            snapshot,
            crate::config::toolbar_item_ids::TOP_GROUP_QUICK_COLORS,
        ) {
            push_divider(&bar, &mut x);
            let show_swatch_badge_row = !plan.compact
                && snapshot
                    .quick_colors
                    .rendered_entries()
                    .iter()
                    .take(plan.swatch_count)
                    .enumerate()
                    .any(|(index, _)| snapshot.binding_hints.quick_color_badge(index).is_some());
            for (index, entry) in snapshot
                .quick_colors
                .rendered_entries()
                .iter()
                .take(plan.swatch_count)
                .enumerate()
            {
                let entry_color = entry.color;
                let action = crate::config::QuickColorPalette::action_for_index(index);
                let binding =
                    action.and_then(|action| snapshot.binding_hints.binding_for_action(action));
                let tooltip = format_binding_label(&entry.label, binding);
                let swatch = SwatchButton::new(
                    entry_color,
                    entry_color == snapshot.color,
                    sz(SWATCH_SIZE),
                    &tooltip,
                );
                let sender = self.feedback.clone();
                swatch.button.connect_clicked(move |_| {
                    send_event(
                        &sender,
                        ToolbarEvent::SetQuickColor {
                            color: entry_color,
                            action,
                        },
                    );
                });
                let is_last = index + 1
                    == plan
                        .swatch_count
                        .min(snapshot.quick_colors.rendered_entries().len());
                let badge = (!plan.compact)
                    .then(|| snapshot.binding_hints.quick_color_badge(index))
                    .flatten();
                let swatch_root = if !show_swatch_badge_row {
                    swatch.button.clone().upcast()
                } else {
                    swatch_with_shortcut(&swatch.button, badge, sz(SWATCH_SIZE), sz(10.0))
                };
                append_gap(&bar, &swatch_root, if is_last { gap } else { SWATCH_GAP });
                x += SWATCH_SIZE + if is_last { gap } else { SWATCH_GAP };
                self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                    swatch.set_selected(entry_color == snapshot.color);
                }));
            }
            let chip = SwatchButton::new(snapshot.color, true, sz(CHIP_SIZE), "Color picker");
            let sender = self.feedback.clone();
            chip.button.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::OpenColorPickerPopup);
            });
            append_gap(&bar, chip.button.as_ref(), gap);
            x += CHIP_SIZE + gap;
            self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                chip.set_color(snapshot.color);
            }));
        }

        // --- History -----------------------------------------------------------
        let undo_visible = model::toolbar_item_visible(
            snapshot,
            crate::config::toolbar_item_ids::TOP_UTILITY_UNDO,
        );
        let redo_visible = model::toolbar_item_visible(
            snapshot,
            crate::config::toolbar_item_ids::TOP_UTILITY_REDO,
        );
        if undo_visible || redo_visible {
            push_divider(&bar, &mut x);
        }
        if undo_visible {
            let button = self.history_button(
                snapshot,
                toolbar_icons::draw_icon_undo,
                Action::Undo,
                ToolbarEvent::Undo,
                |snapshot| snapshot.undo_available,
                (sz(btn_w), sz(btn_h)),
                sz(ICON_SIZE),
                use_icons,
                !plan.compact,
            );
            append_gap(&bar, button.as_ref(), gap);
            x += btn_w + gap;
        }
        if redo_visible {
            let button = self.history_button(
                snapshot,
                toolbar_icons::draw_icon_redo,
                Action::Redo,
                ToolbarEvent::Redo,
                |snapshot| snapshot.redo_available,
                (sz(btn_w), sz(btn_h)),
                sz(ICON_SIZE),
                use_icons,
                !plan.compact,
            );
            append_gap(&bar, button.as_ref(), gap);
            x += btn_w + gap;
        }

        // --- Destructive Clear, isolated by a double gap -------------------------
        if clear_visible {
            let button = if use_icons {
                let icon = icon_button(
                    toolbar_icons::draw_icon_clear,
                    (sz(btn_w), sz(btn_h)),
                    sz(ICON_SIZE),
                    &action_tooltip(snapshot, Action::ClearCanvas),
                );
                icon.button
            } else {
                text_button(
                    action_short_label(Action::ClearCanvas),
                    (sz(btn_w), sz(btn_h)),
                    &action_tooltip(snapshot, Action::ClearCanvas),
                )
            };
            if !plan.compact {
                add_shortcut_badge(
                    &button,
                    snapshot.binding_hints.badge_for_action(Action::ClearCanvas),
                );
            }
            button.add_css_class("destructive");
            button.set_margin_start(px(gap));
            let sender = self.feedback.clone();
            button.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::ClearCanvas);
            });
            append_gap(&bar, button.as_ref(), gap);
        }

        // --- Right-aligned chrome -----------------------------------------------
        let chrome_size = if plan.compact {
            COMPACT_CHROME
        } else {
            PIN_BUTTON_SIZE
        };
        let chrome_gap = if plan.compact {
            COMPACT_GAP
        } else {
            PIN_BUTTON_GAP
        };
        let chrome = gtk4::Box::new(gtk4::Orientation::Horizontal, px(chrome_gap));
        chrome.set_margin_end(px(if plan.compact {
            COMPACT_MARGIN_RIGHT
        } else {
            PIN_MARGIN_RIGHT
        }));
        chrome.set_valign(gtk4::Align::Center);
        if model::toolbar_item_visible(snapshot, crate::config::toolbar_item_ids::TOP_CHROME_PIN) {
            chrome.append(&self.pin_button(snapshot, sz(chrome_size)));
        }
        if plan.show_overflow {
            chrome.append(&self.overflow_button(snapshot, plan, sz(chrome_size), use_icons));
        }
        if model::toolbar_item_visible(snapshot, crate::config::toolbar_item_ids::TOP_CHROME_CLOSE)
        {
            chrome.append(&self.minimize_button(sz(chrome_size)));
        }
        bar.append(&chrome);

        // --- Contextual highlight ring row ----------------------------------------
        if ring_row_active(snapshot, plan)
            && let Some(ring_x) = highlight_x
        {
            let ring = gtk4::CheckButton::with_label("Ring");
            ring.add_css_class("mini");
            ring.set_tooltip_text(Some("Highlight ring"));
            ring.set_active(snapshot.highlight_tool_ring_enabled);
            ring.set_halign(gtk4::Align::Start);
            ring.set_margin_start(px(ring_x));
            ring.set_margin_top(px(2.0));
            let sender = self.feedback.clone();
            let syncing = Rc::new(Cell::new(false));
            let toggle_sync = syncing.clone();
            ring.connect_toggled(move |check| {
                if !toggle_sync.get() {
                    send_event(
                        &sender,
                        ToolbarEvent::ToggleHighlightToolRing(check.is_active()),
                    );
                }
            });
            let ring_handle = ring.clone();
            self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                if ring_handle.is_active() != snapshot.highlight_tool_ring_enabled {
                    syncing.set(true);
                    ring_handle.set_active(snapshot.highlight_tool_ring_enabled);
                    syncing.set(false);
                }
            }));
            self.root.append(&ring);
        }
    }
}
