//! GTK top-strip assembly.
//!
//! Owns the ordered strip layout and width-degradation presentation while
//! delegating individual controls to the control factory methods.

use super::*;

/// Micro-chip ring state: `(ring color, spec-unit ring width)`.
type MicroRing = ((f64, f64, f64, f64), f64);

pub(super) fn top_toolbar_spec(
    snapshot: &ToolbarSnapshot,
    plan: &TopStripPlan,
) -> model::TopToolbarSpec {
    model::TopToolbarSpec::build(snapshot, plan)
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
        // The strip mode strips `.panel` from the root (the pills own the
        // background); the minimized tab is a single surface again.
        self.root.add_css_class("panel");
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

    /// Micro-mode top strip: one 44px round chip (active tool glyph inside
    /// a ring stroked in the current color). Clicking restores the full
    /// strip. The drawing itself is shared with the built-in frontend via
    /// `toolbar_icons::draw_micro_chip`.
    pub(super) fn build_micro(&mut self, snapshot: &ToolbarSnapshot, plan: &TopStripPlan) {
        let spec = top_toolbar_spec(snapshot, plan);
        let control = match spec.strip() {
            [model::TopToolbarNode::Control(control)] => *control,
            _ => unreachable!("micro specification contains one chip control"),
        };
        let scale = effective_scale(snapshot);
        self.window.set_default_size(
            (MICRO_SIZE * scale).round() as i32,
            (MICRO_SIZE * scale).round() as i32,
        );
        // The chip paints its own round panel disc; the window root and the
        // button chrome stay transparent (the `.swatch` treatment).
        self.root.remove_css_class("panel");
        self.root.remove_css_class("minimized");
        let chip = sized_button(MICRO_SIZE * scale, MICRO_SIZE * scale);
        set_control_widget_id(&chip, control);
        chip.add_css_class("swatch");
        let accessible_label = control.accessible_label(snapshot);
        chip.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        chip.set_tooltip_text(Some(&control.tooltip(snapshot)));

        let painter = Rc::new(Cell::new(crate::toolbar_icons::top_toolbar_icon_painter(
            control.icon(snapshot).expect("micro chip tool icon"),
        )));
        let ring: Rc<Cell<MicroRing>> = Rc::new(Cell::new((
            (
                snapshot.color.r,
                snapshot.color.g,
                snapshot.color.b,
                snapshot.color.a,
            ),
            model::micro_ring_width(snapshot.thickness),
        )));
        let hovered = Rc::new(Cell::new(false));

        let area = gtk4::DrawingArea::new();
        let size = (MICRO_SIZE * scale).round().max(1.0) as i32;
        area.set_content_width(size);
        area.set_content_height(size);
        area.set_can_target(false);
        let draw_painter = painter.clone();
        let draw_ring = ring.clone();
        let draw_hovered = hovered.clone();
        area.set_draw_func(move |area, ctx, width, height| {
            let color = area.color();
            let (ring_color, ring_width) = draw_ring.get();
            let chip_size = (width.min(height) as f64).max(1.0);
            let x = (width as f64 - chip_size) / 2.0;
            let y = (height as f64 - chip_size) / 2.0;
            crate::toolbar_icons::draw_micro_chip(
                ctx,
                x,
                y,
                chip_size,
                draw_painter.get(),
                &crate::toolbar_icons::MicroChipStyle {
                    ring_color,
                    // Ring width is in spec units; scale with the chip.
                    ring_width: ring_width * (chip_size / MICRO_SIZE),
                    icon_color: (
                        color.red() as f64,
                        color.green() as f64,
                        color.blue() as f64,
                        color.alpha() as f64,
                    ),
                    hovered: draw_hovered.get(),
                },
            );
        });
        chip.set_child(Some(&area));

        let motion = gtk4::EventControllerMotion::new();
        let enter_hovered = hovered.clone();
        let enter_area = area.clone();
        motion.connect_enter(move |_, _, _| {
            enter_hovered.set(true);
            enter_area.queue_draw();
        });
        let leave_hovered = hovered;
        let leave_area = area.clone();
        motion.connect_leave(move |_| {
            leave_hovered.set(false);
            leave_area.queue_draw();
        });
        chip.add_controller(motion);

        let sender = self.feedback.clone();
        let event = control.event(snapshot);
        chip.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        self.root.append(&chip);

        // Live state: tool glyph, ring color, and ring width track the
        // snapshot without rebuilding the chip.
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            let next_painter = crate::toolbar_icons::top_toolbar_icon_painter(
                model::TopToolbarIcon::Tool(model::semantic_icon_for_tool(snapshot.active_tool)),
            );
            let mut dirty = false;
            if !std::ptr::fn_addr_eq(painter.get(), next_painter) {
                painter.set(next_painter);
                dirty = true;
            }
            let next_ring = (
                (
                    snapshot.color.r,
                    snapshot.color.g,
                    snapshot.color.b,
                    snapshot.color.a,
                ),
                model::micro_ring_width(snapshot.thickness),
            );
            if ring.get() != next_ring {
                ring.set(next_ring);
                dirty = true;
            }
            if dirty {
                area.queue_draw();
            }
        }));
    }

    pub(super) fn build_strip(&mut self, snapshot: &ToolbarSnapshot, plan: &TopStripPlan) {
        let spec = top_toolbar_spec(snapshot, plan);
        self.root.remove_css_class("minimized");
        // The window root stays transparent behind the pill islands (only
        // the minimized restore tab re-adds `.panel`); the pills carry the
        // panel background themselves so nothing double-boxes.
        self.root.remove_css_class("panel");
        // GTK toplevels retain their previous default width across widget-tree
        // rebuilds. Reset it from the shared natural-size calculation so a
        // narrower layout (notably `simple`) does not keep the regular strip's
        // empty trailing area. Height remains content-driven for GTK popovers.
        self.window
            .set_default_size(top_default_width(snapshot), -1);
        let scale = effective_scale(snapshot);
        let use_icons = snapshot.use_icons || plan.compact;
        let gap = if plan.compact { COMPACT_GAP } else { GAP };
        let island_gap = if plan.compact {
            COMPACT_ISLAND_GAP
        } else {
            ISLAND_GAP
        };
        let (btn_w, btn_h) = if plan.compact {
            (COMPACT_BUTTON, COMPACT_BUTTON)
        } else if use_icons {
            (ICON_BUTTON, ICON_BUTTON)
        } else {
            (TEXT_BUTTON_W, TEXT_BUTTON_H)
        };
        let sz = |value: f64| value * scale;
        let px = |value: f64| (value * scale).round() as i32;

        // Detached pill islands (spec-derived membership) inside a
        // transparent outer row: tools | presets | history | chrome. Compact
        // plans tighten the pill's inner padding via the `.compact` CSS variant
        // (mirrors the builtin TOP_COMPACT_ISLAND_PAD).
        let style_pill = |widget: &gtk4::Widget| {
            widget.add_css_class("pill");
            if plan.compact {
                widget.add_css_class("compact");
            }
        };
        let outer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        let island_tools = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        style_pill(island_tools.upcast_ref());
        set_island_widget_id(&island_tools, model::TopToolbarIsland::Tools);
        let island_presets = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        style_pill(island_presets.upcast_ref());
        set_island_widget_id(&island_presets, model::TopToolbarIsland::Presets);
        let island_history = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        style_pill(island_history.upcast_ref());
        set_island_widget_id(&island_history, model::TopToolbarIsland::History);
        let island_chrome = gtk4::Box::new(
            gtk4::Orientation::Horizontal,
            px(if plan.compact {
                COMPACT_GAP
            } else {
                PIN_BUTTON_GAP
            }),
        );
        style_pill(island_chrome.upcast_ref());
        set_island_widget_id(&island_chrome, model::TopToolbarIsland::Chrome);

        // Running spec-unit x used to align the contextual ring row under
        // the Highlight button selected by the shared specification.
        //
        // Accounting: x measures from island A's *content* origin, so it
        // starts at 0. The outer island row sits at the window origin (the
        // strip root carries no `.panel` padding), island A is its first
        // child, and both island A and the ring row below are `.pill`s that
        // share the same hairline border and inner leading padding — those
        // offsets cancel, so `margin_start(px(highlight_x))` on the ring row
        // lands its content exactly under the Highlight button's left edge.
        let mut x = 0.0;
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

        for node in spec.strip() {
            let bar = match node.island() {
                model::TopToolbarIsland::History => &island_history,
                model::TopToolbarIsland::Presets => &island_presets,
                _ => &island_tools,
            };
            match *node {
                model::TopToolbarNode::Divider(divider) => push_divider(bar, &mut x, divider),
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
                        append_gap(bar, grip.area.as_ref(), gap);
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
                        append_gap(bar, button.as_ref(), gap);
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
                        append_gap(bar, button.as_ref(), gap);
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
                            append_gap(bar, button.as_ref(), gap);
                            x += btn_w + gap;
                        }
                    }
                    model::TopToolbarControl::Preset(index) => {
                        let button =
                            self.preset_button(snapshot, control, index, (sz(btn_w), sz(btn_h)));
                        append_gap(bar, button.as_ref(), gap);
                        x += btn_w + gap;
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
                        append_gap(bar, button.as_ref(), gap);
                        x += btn_w + gap;
                    }
                    model::TopToolbarControl::Overflow => {
                        // The ⋯ toggle anchors the overflow menu (Clear
                        // first, then width-dropped items) from the history
                        // island.
                        let button = self.overflow_button(
                            snapshot,
                            control,
                            (sz(btn_w), sz(btn_h)),
                            sz(ICON_SIZE),
                        );
                        append_gap(bar, button.as_ref(), gap);
                        x += btn_w + gap;
                    }
                    model::TopToolbarControl::Restore
                    | model::TopToolbarControl::MicroChip
                    | model::TopToolbarControl::Pin
                    | model::TopToolbarControl::Minimize
                    | model::TopToolbarControl::ClearCanvas
                    | model::TopToolbarControl::CanvasMenu
                    | model::TopToolbarControl::SessionMenu
                    | model::TopToolbarControl::SettingsMenu
                    | model::TopToolbarControl::HighlightRing => {
                        unreachable!("control belongs outside the main strip")
                    }
                },
            }
        }

        // --- Right-aligned chrome island ----------------------------------------
        let chrome_size = if plan.compact {
            COMPACT_CHROME
        } else {
            PIN_BUTTON_SIZE
        };
        for control in spec.chrome().iter().copied() {
            match control {
                model::TopToolbarControl::Pin => {
                    island_chrome.append(&self.pin_button(snapshot, control, sz(chrome_size)));
                }
                model::TopToolbarControl::Minimize => {
                    island_chrome.append(&self.minimize_button(snapshot, control, sz(chrome_size)));
                }
                _ => unreachable!("non-chrome control in chrome specification"),
            }
        }

        // Assemble only the populated pills; every pill after the first is
        // separated by the shared inter-island gap.
        for island in [
            &island_tools,
            &island_presets,
            &island_history,
            &island_chrome,
        ] {
            if island.first_child().is_none() {
                continue;
            }
            if outer.first_child().is_some() {
                island.set_margin_start(px(island_gap));
            }
            outer.append(island);
        }
        self.root.append(&outer);

        // Idle fade: the pill islands dim with the snapshot's fade value
        // (1.0 full, 0.55 dimmed, in-between while animating; the backend
        // engine snaps under reduced motion). Continuous opacity, driven
        // per-update, so open popovers and hover state survive.
        let fade_outer = outer.clone();
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            fade_outer.set_opacity(snapshot.top_fade.clamp(0.0, 1.0));
        }));

        // --- Contextual highlight ring row ----------------------------------------
        if let Some(control) = spec.contextual().first().copied()
            && let Some(ring_x) = highlight_x
        {
            let ring = gtk4::CheckButton::with_label(&control.label(snapshot));
            set_control_widget_id(&ring, control);
            let accessible_label = control.accessible_label(snapshot);
            ring.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
            ring.add_css_class("mini");
            // The root is transparent behind the pills, so the contextual
            // ring row carries its own small pill background. It uses the
            // same `.pill` (and compact) padding as island A, which keeps
            // the ring-x accounting above exact.
            style_pill(ring.upcast_ref());
            ring.set_tooltip_text(Some(&control.tooltip(snapshot)));
            ring.set_active(control.active(snapshot));
            ring.set_halign(gtk4::Align::Start);
            // `ring_x` is the Highlight button's offset from island A's
            // content origin (see the x accounting above): the shared pill
            // border + leading padding on this row reproduce the same
            // content inset, so the plain margin aligns the two.
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
                // The contextual ring row fades with the islands above it.
                ring_handle.set_opacity(snapshot.top_fade.clamp(0.0, 1.0));
            }));
            self.root.append(&ring);
        }

        // --- Style pill (island D): contextual tool properties -------------------
        self.build_style_pill(snapshot, plan);
    }
}
