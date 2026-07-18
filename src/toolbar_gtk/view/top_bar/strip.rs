//! GTK top-strip assembly.
//!
//! Owns the ordered strip layout and width-degradation presentation while
//! delegating individual controls to the control factory methods.

use super::*;

pub(super) fn top_toolbar_spec(
    snapshot: &ToolbarSnapshot,
    plan: &TopStripPlan,
) -> model::TopToolbarSpec {
    model::TopToolbarSpec::build(snapshot, plan)
}

pub(super) fn quick_color_badge_row_visible(
    snapshot: &ToolbarSnapshot,
    spec: &model::TopToolbarSpec,
) -> bool {
    spec.strip().iter().any(|node| {
        matches!(
            node,
            model::TopToolbarNode::Control(model::TopToolbarControl::QuickColor(index))
                if snapshot.binding_hints.quick_color_badge(*index).is_some()
        )
    })
}

impl TopBar {
    pub(super) fn build_minimized(&mut self, snapshot: &ToolbarSnapshot, plan: &TopStripPlan) {
        let spec = top_toolbar_spec(snapshot, plan);
        let control = match spec.strip() {
            [model::TopToolbarNode::Control(control)] => *control,
            _ => unreachable!("minimized specification contains one restore control"),
        };
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
        set_control_widget_id(&restore, control);
        restore.add_css_class("chrome");
        let accessible_label = control.accessible_label(snapshot);
        restore.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        restore.set_tooltip_text(Some(&control.tooltip(snapshot)));
        let icon = IconWidget::new(
            top_toolbar_icon_painter(control.icon(snapshot).expect("restore icon")),
            (MINIMIZED_SIZE.1 * 0.75 * scale).min(18.0 * scale),
        );
        restore.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        let event = control.event(snapshot);
        restore.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        self.root.append(&restore);
    }

    pub(super) fn build_strip(&mut self, snapshot: &ToolbarSnapshot, plan: &TopStripPlan) {
        let spec = top_toolbar_spec(snapshot, plan);
        self.root.remove_css_class("minimized");
        // GTK toplevels retain their previous default width across widget-tree
        // rebuilds. Reset it from the shared natural-size calculation so a
        // narrower layout (notably `simple`) does not keep the regular strip's
        // empty trailing area. Height remains content-driven for GTK popovers.
        self.window
            .set_default_size(top_default_width(snapshot), -1);
        let scale = effective_scale(snapshot);
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

        // Running spec-unit x used to align the contextual ring row under
        // the Highlight button selected by the shared specification.
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
        let push_divider = |bar: &gtk4::Box, x: &mut f64, kind: model::TopToolbarDivider| {
            let span = if plan.compact { 3.0 } else { DIVIDER_SPAN };
            let divider = gtk4::Separator::new(gtk4::Orientation::Vertical);
            set_semantic_widget_id(&divider, kind.id());
            divider.set_margin_top(px(6.0));
            divider.set_margin_bottom(px(6.0));
            divider.set_margin_start(px((span - 1.0) / 2.0));
            divider.set_margin_end(px((span - 1.0) / 2.0) + px(gap));
            bar.append(&divider);
            *x += span + gap;
        };

        let quick_color_count = spec
            .strip()
            .iter()
            .filter(|node| {
                matches!(
                    node,
                    model::TopToolbarNode::Control(model::TopToolbarControl::QuickColor(_))
                )
            })
            .count();
        let show_swatch_badge_row = quick_color_badge_row_visible(snapshot, &spec);
        let mut quick_color_seen = 0usize;

        for node in spec.strip() {
            match *node {
                model::TopToolbarNode::Divider(divider) => push_divider(&bar, &mut x, divider),
                model::TopToolbarNode::Control(control) => match control {
                    model::TopToolbarControl::DragHandle => {
                        let grip = IconWidget::new(
                            top_toolbar_icon_painter(control.icon(snapshot).expect("drag icon")),
                            sz(HANDLE_SIZE),
                        );
                        set_control_widget_id(&grip.area, control);
                        grip.area.set_can_target(true);
                        grip.area.add_css_class("drag-handle");
                        let accessible_label = control.accessible_label(snapshot);
                        grip.area
                            .update_property(&[gtk4::accessible::Property::Label(
                                &accessible_label,
                            )]);
                        grip.area.set_tooltip_text(Some(&control.tooltip(snapshot)));
                        grip.area.set_valign(gtk4::Align::Center);
                        grip.area.set_cursor_from_name(Some("grab"));
                        self.attach_move_drag(&grip.area);
                        append_gap(&bar, grip.area.as_ref(), gap);
                        x += HANDLE_SIZE + gap;
                    }
                    model::TopToolbarControl::Tool(_) => {
                        let button = self.tool_button(
                            snapshot,
                            control,
                            (sz(btn_w), sz(btn_h)),
                            sz(ICON_SIZE),
                            use_icons,
                            !plan.compact,
                        );
                        append_gap(&bar, button.as_ref(), gap);
                        x += btn_w + gap;
                    }
                    model::TopToolbarControl::ShapePicker => {
                        let button = self.shapes_picker_button(
                            snapshot,
                            control,
                            (sz(btn_w), sz(btn_h)),
                            sz(ICON_SIZE),
                            use_icons,
                        );
                        append_gap(&bar, button.as_ref(), gap);
                        x += btn_w + gap;
                    }
                    model::TopToolbarControl::Utility(_) => {
                        if matches!(
                            control,
                            model::TopToolbarControl::Utility(model::TopToolbarUtility::Highlight)
                        ) {
                            highlight_x = Some(x);
                        }
                        if let Some(button) = self.utility_button(
                            snapshot,
                            control,
                            (sz(btn_w), sz(btn_h)),
                            sz(ICON_SIZE),
                            use_icons,
                            !plan.compact,
                        ) {
                            append_gap(&bar, button.as_ref(), gap);
                            x += btn_w + gap;
                        }
                    }
                    model::TopToolbarControl::QuickColor(index) => {
                        let entry_color = snapshot.quick_colors.rendered_entries()[index].color;
                        let swatch = SwatchButton::new(
                            entry_color,
                            control.active(snapshot),
                            sz(SWATCH_SIZE),
                            &control.tooltip(snapshot),
                        );
                        let accessible_label = control.accessible_label(snapshot);
                        swatch
                            .button
                            .update_property(&[gtk4::accessible::Property::Label(
                                &accessible_label,
                            )]);
                        let sender = self.feedback.clone();
                        let event = control.event(snapshot);
                        swatch.button.connect_clicked(move |_| {
                            send_event(&sender, event.clone());
                        });
                        quick_color_seen += 1;
                        let is_last = quick_color_seen == quick_color_count;
                        let badge = control.shortcut_badge(snapshot);
                        let swatch_root = if !show_swatch_badge_row {
                            swatch.button.clone().upcast()
                        } else {
                            swatch_with_shortcut(
                                &swatch.button,
                                badge.as_deref(),
                                sz(SWATCH_SIZE),
                                sz(10.0),
                            )
                        };
                        set_control_widget_id(&swatch_root, control);
                        append_gap(&bar, &swatch_root, if is_last { gap } else { SWATCH_GAP });
                        x += SWATCH_SIZE + if is_last { gap } else { SWATCH_GAP };
                        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                            swatch.set_selected(entry_color == snapshot.color);
                        }));
                    }
                    model::TopToolbarControl::CurrentColor => {
                        let chip = SwatchButton::new(
                            snapshot.color,
                            control.active(snapshot),
                            sz(CHIP_SIZE),
                            &control.tooltip(snapshot),
                        );
                        set_control_widget_id(&chip.button, control);
                        let accessible_label = control.accessible_label(snapshot);
                        chip.button
                            .update_property(&[gtk4::accessible::Property::Label(
                                &accessible_label,
                            )]);
                        let sender = self.feedback.clone();
                        let event = control.event(snapshot);
                        chip.button.connect_clicked(move |_| {
                            send_event(&sender, event.clone());
                        });
                        append_gap(&bar, chip.button.as_ref(), gap);
                        x += CHIP_SIZE + gap;
                        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                            chip.set_color(snapshot.color);
                        }));
                    }
                    model::TopToolbarControl::Undo | model::TopToolbarControl::Redo => {
                        let button = self.history_button(
                            snapshot,
                            control,
                            (sz(btn_w), sz(btn_h)),
                            sz(ICON_SIZE),
                            use_icons,
                            !plan.compact,
                        );
                        append_gap(&bar, button.as_ref(), gap);
                        x += btn_w + gap;
                    }
                    model::TopToolbarControl::ClearCanvas => {
                        let button = self.action_button(
                            snapshot,
                            control,
                            (sz(btn_w), sz(btn_h)),
                            sz(ICON_SIZE),
                            use_icons,
                            !plan.compact,
                        );
                        button.add_css_class("destructive");
                        button.set_margin_start(px(gap));
                        append_gap(&bar, button.as_ref(), gap);
                        x += btn_w + gap * 2.0;
                    }
                    model::TopToolbarControl::Restore
                    | model::TopToolbarControl::Pin
                    | model::TopToolbarControl::Overflow
                    | model::TopToolbarControl::Minimize
                    | model::TopToolbarControl::HighlightRing => {
                        unreachable!("control belongs outside the main strip")
                    }
                },
            }
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
        for control in spec.chrome().iter().copied() {
            match control {
                model::TopToolbarControl::Pin => {
                    chrome.append(&self.pin_button(snapshot, control, sz(chrome_size)));
                }
                model::TopToolbarControl::Overflow => {
                    chrome.append(&self.overflow_button(snapshot, control, sz(chrome_size)));
                }
                model::TopToolbarControl::Minimize => {
                    chrome.append(&self.minimize_button(snapshot, control, sz(chrome_size)));
                }
                _ => unreachable!("non-chrome control in chrome specification"),
            }
        }
        bar.append(&chrome);

        // --- Contextual highlight ring row ----------------------------------------
        if let Some(control) = spec.contextual().first().copied()
            && let Some(ring_x) = highlight_x
        {
            let ring = gtk4::CheckButton::with_label(&control.label(snapshot));
            set_control_widget_id(&ring, control);
            let accessible_label = control.accessible_label(snapshot);
            ring.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
            ring.add_css_class("mini");
            ring.set_tooltip_text(Some(&control.tooltip(snapshot)));
            ring.set_active(control.active(snapshot));
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
                        super::controls::event_for_toggle_state(control, check.is_active()),
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
