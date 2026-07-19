//! GTK style pill (island D): the contextual tool-property row.
//!
//! A fourth `.pill` box appended to the bar root under the islands (the
//! ring-row pattern scaled up). Structure comes from the shared
//! `model::StylePillSpec`; the bar rebuilds when the pill's morph state
//! changes (the control-id list is part of `StructureKey`) while live
//! values (slider positions, swatch selection, numerals, segment actives,
//! reset tooltips) run through stored updaters.

use super::*;

fn format_px(value: f64) -> String {
    format!("{value:.0}px")
}

fn format_percent(value: f64) -> String {
    format!("{:.0}%", value * 100.0)
}

fn format_pt(value: f64) -> String {
    format!("{value:.0}pt")
}

/// Pill button on the shared `sized_button` chassis: non-focusable and
/// releasing window keyboard focus on click, like every other top-bar
/// control. The GTK bars must never retain keyboard focus — the popups the
/// pill opens (color picker, precise entry) live on the overlay surface,
/// which keeps the keyboard.
fn pill_button(label: &str, width: f64, height: f64) -> gtk4::Button {
    let button = sized_button(width, height);
    button.set_label(label);
    button
}

impl TopBar {
    pub(super) fn build_style_pill(&mut self, snapshot: &ToolbarSnapshot, plan: &TopStripPlan) {
        let spec = model::StylePillSpec::build(snapshot, plan);
        if spec.controls().is_empty() {
            return;
        }
        let scale = effective_scale(snapshot);
        let sz = |value: f64| value * scale;
        let px = |value: f64| (value * scale).round() as i32;
        let gap = if plan.compact { COMPACT_GAP } else { GAP };

        let pill = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        pill.add_css_class("pill");
        if plan.compact {
            pill.add_css_class("compact");
        }
        // Named like the islands (`island.<key>`, not `top.`-prefixed) so
        // the contract suite keeps descending into the box while asserting
        // pill membership through the widget ancestry.
        set_semantic_widget_id(&pill, "island.style");
        pill.set_halign(gtk4::Align::Start);
        pill.set_margin_top(px(STYLE_PILL_GAP));

        let append_gap = |bar: &gtk4::Box, widget: &gtk4::Widget, gap_units: f64| {
            widget.set_margin_end(px(gap_units).max(0));
            bar.append(widget);
        };

        let swatch_count = spec
            .controls()
            .iter()
            .filter(|control| matches!(control, model::StylePillControl::QuickSwatch(_)))
            .count();

        for control in spec.controls().iter().copied() {
            match control {
                model::StylePillControl::ColorChip => {
                    let chip = SwatchButton::new(
                        snapshot.color,
                        control.active(snapshot),
                        sz(CHIP_SIZE),
                        control.tooltip(snapshot).as_deref().unwrap_or_default(),
                    );
                    set_semantic_widget_id(&chip.button, control.id().as_ref());
                    let accessible_label = control.label(snapshot);
                    chip.button
                        .update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
                    let sender = self.feedback.clone();
                    let event = control.event(snapshot).expect("color chip event");
                    chip.button.connect_clicked(move |_| {
                        send_event(&sender, event.clone());
                    });
                    chip.button.set_valign(gtk4::Align::Center);
                    append_gap(&pill, chip.button.as_ref(), gap);
                    self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                        chip.set_color(snapshot.color);
                    }));
                }
                model::StylePillControl::QuickSwatch(index) => {
                    let entry_color = snapshot.quick_colors.rendered_entries()[index].color;
                    let swatch = SwatchButton::new(
                        entry_color,
                        control.active(snapshot),
                        sz(SWATCH_SIZE),
                        control.tooltip(snapshot).as_deref().unwrap_or_default(),
                    );
                    set_semantic_widget_id(&swatch.button, control.id().as_ref());
                    let accessible_label = control.label(snapshot);
                    swatch
                        .button
                        .update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
                    let sender = self.feedback.clone();
                    let event = control.event(snapshot).expect("swatch event");
                    swatch.button.connect_clicked(move |_| {
                        send_event(&sender, event.clone());
                    });
                    swatch.button.set_valign(gtk4::Align::Center);
                    let is_last = index + 1 == swatch_count;
                    append_gap(
                        &pill,
                        swatch.button.upcast_ref(),
                        if is_last { gap } else { SWATCH_GAP },
                    );
                    self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                        swatch.set_selected(entry_color == snapshot.color);
                    }));
                }
                model::StylePillControl::ThicknessSlider
                | model::StylePillControl::OpacitySlider
                | model::StylePillControl::FontSizeSlider => {
                    let (slider_spec, value) = control.slider(snapshot).expect("slider control");
                    let format = match control {
                        model::StylePillControl::ThicknessSlider => format_px as fn(f64) -> String,
                        model::StylePillControl::OpacitySlider => format_percent,
                        _ => format_pt,
                    };
                    let sender = self.feedback.clone();
                    let slider = SliderRow::new(
                        scale,
                        (slider_spec.min, slider_spec.max),
                        value,
                        format,
                        move |value| {
                            let event = match control {
                                model::StylePillControl::ThicknessSlider => {
                                    ToolbarEvent::SetThickness(value)
                                }
                                model::StylePillControl::OpacitySlider => {
                                    ToolbarEvent::SetMarkerOpacity(value)
                                }
                                _ => ToolbarEvent::SetFontSize(value),
                            };
                            send_event(&sender, event);
                        },
                    );
                    // The thickness/text-size readouts are distinct numeral
                    // controls; only the opacity slider keeps its built-in
                    // readout.
                    slider.set_value_label_visible(matches!(
                        control,
                        model::StylePillControl::OpacitySlider
                    ));
                    set_semantic_widget_id(&slider.root, control.id().as_ref());
                    slider.root.set_size_request(px(STYLE_SLIDER_W), -1);
                    slider.root.set_valign(gtk4::Align::Center);
                    append_gap(&pill, slider.root.upcast_ref(), gap);
                    self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                        let value = match control {
                            model::StylePillControl::ThicknessSlider => snapshot.thickness,
                            model::StylePillControl::OpacitySlider => snapshot.marker_opacity,
                            _ => snapshot.font_size,
                        };
                        slider.set_value(value);
                    }));
                }
                model::StylePillControl::ThicknessValue
                | model::StylePillControl::FontSizeValue => {
                    // Live numeral button; clicking opens the overlay
                    // precise-entry popup (the shared event path, like the
                    // color chip's overlay picker popup).
                    let button = pill_button(
                        &control.value_text(snapshot).expect("numeral text"),
                        sz(STYLE_VALUE_W),
                        sz(STYLE_ROW_H),
                    );
                    let sender = self.feedback.clone();
                    let event = control.event(snapshot).expect("numeral event");
                    button.connect_clicked(move |_| {
                        send_event(&sender, event.clone());
                    });
                    set_semantic_widget_id(&button, control.id().as_ref());
                    if let Some(tooltip) = control.tooltip(snapshot) {
                        button.set_tooltip_text(Some(&tooltip));
                    }
                    let accessible_label = control.label(snapshot);
                    button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
                    append_gap(&pill, button.upcast_ref(), gap);
                    self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                        button.set_label(&control.value_text(snapshot).expect("numeral text"));
                    }));
                }
                model::StylePillControl::FillToggle | model::StylePillControl::AutoNumberToggle => {
                    let check = gtk4::CheckButton::with_label(control.label(snapshot).as_ref());
                    check.add_css_class("mini");
                    set_semantic_widget_id(&check, control.id().as_ref());
                    if let Some(tooltip) = control.tooltip(snapshot) {
                        check.set_tooltip_text(Some(&tooltip));
                    }
                    check.set_active(control.active(snapshot));
                    check.set_valign(gtk4::Align::Center);
                    let sender = self.feedback.clone();
                    let syncing = Rc::new(Cell::new(false));
                    let toggle_sync = syncing.clone();
                    check.connect_toggled(move |check| {
                        if !toggle_sync.get() {
                            let event = match control {
                                model::StylePillControl::FillToggle => {
                                    ToolbarEvent::ToggleFill(check.is_active())
                                }
                                _ => ToolbarEvent::ToggleArrowLabels(check.is_active()),
                            };
                            send_event(&sender, event);
                        }
                    });
                    append_gap(&pill, check.upcast_ref(), gap);
                    self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                        let active = control.active(snapshot);
                        if check.is_active() != active {
                            syncing.set(true);
                            check.set_active(active);
                            syncing.set(false);
                        }
                    }));
                }
                model::StylePillControl::CounterReset(_) => {
                    let button =
                        pill_button(control.label(snapshot).as_ref(), -1.0, sz(STYLE_ROW_H));
                    set_semantic_widget_id(&button, control.id().as_ref());
                    if let Some(tooltip) = control.tooltip(snapshot) {
                        button.set_tooltip_text(Some(&tooltip));
                    }
                    let sender = self.feedback.clone();
                    let event = control.event(snapshot).expect("reset event");
                    button.connect_clicked(move |_| {
                        send_event(&sender, event.clone());
                    });
                    append_gap(&pill, button.upcast_ref(), gap);
                    let handle = button.clone();
                    self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                        // The next-number tooltip tracks the counter live.
                        if let Some(tooltip) = control.tooltip(snapshot) {
                            handle.set_tooltip_text(Some(&tooltip));
                        }
                    }));
                }
                model::StylePillControl::SelectionCycle(_) => {
                    let button = pill_button(
                        &control.value_text(snapshot).unwrap_or_default(),
                        sz(STYLE_SEL_VALUE_W),
                        sz(STYLE_ROW_H),
                    );
                    set_semantic_widget_id(&button, control.id().as_ref());
                    button.set_sensitive(control.enabled(snapshot));
                    if let Some(tooltip) = control.tooltip(snapshot) {
                        button.set_tooltip_text(Some(&tooltip));
                    }
                    let accessible_label = control.label(snapshot);
                    button.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
                    let sender = self.feedback.clone();
                    let event = control.event(snapshot).expect("selection cycle event");
                    button.connect_clicked(move |_| {
                        send_event(&sender, event.clone());
                    });
                    append_gap(&pill, button.upcast_ref(), gap);
                    self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                        button.set_label(&control.value_text(snapshot).unwrap_or_default());
                        button.set_sensitive(control.enabled(snapshot));
                        button.set_tooltip_text(control.tooltip(snapshot).as_deref());
                    }));
                }
                model::StylePillControl::SelectionStepper(_) => {
                    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, px(2.0));
                    set_semantic_widget_id(&row, control.id().as_ref());
                    row.set_valign(gtk4::Align::Center);
                    let steps = control.steps(snapshot).expect("stepper halves");
                    let mut handles: Vec<gtk4::Button> = Vec::new();
                    let minus = pill_button(steps[0].label, sz(STYLE_STEP_W), sz(STYLE_ROW_H));
                    set_semantic_widget_id(&minus, steps[0].id);
                    minus.set_tooltip_text(Some(&steps[0].tooltip));
                    row.append(&minus);
                    handles.push(minus.clone());
                    let value =
                        gtk4::Label::new(Some(&control.value_text(snapshot).unwrap_or_default()));
                    set_semantic_widget_id(&value, &format!("{}.value", control.id()));
                    value.set_width_request(px(STYLE_SEL_VALUE_W));
                    row.append(&value);
                    let plus = pill_button(steps[1].label, sz(STYLE_STEP_W), sz(STYLE_ROW_H));
                    set_semantic_widget_id(&plus, steps[1].id);
                    plus.set_tooltip_text(Some(&steps[1].tooltip));
                    row.append(&plus);
                    handles.push(plus.clone());
                    for (button, step) in handles.iter().zip(steps.iter()) {
                        button.set_sensitive(control.enabled(snapshot));
                        let sender = self.feedback.clone();
                        let event = step.event.clone();
                        button.connect_clicked(move |_| {
                            send_event(&sender, event.clone());
                        });
                    }
                    append_gap(&pill, row.upcast_ref(), gap);
                    self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                        value.set_label(&control.value_text(snapshot).unwrap_or_default());
                        let enabled = control.enabled(snapshot);
                        for button in &handles {
                            button.set_sensitive(enabled);
                        }
                    }));
                }
                model::StylePillControl::FontFamilySegment
                | model::StylePillControl::EraserModeSegment => {
                    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, px(2.0));
                    set_semantic_widget_id(&row, control.id().as_ref());
                    row.set_valign(gtk4::Align::Center);
                    let segments = control.segments(snapshot).expect("segment halves");
                    let mut handles: Vec<(gtk4::Button, &'static str)> = Vec::new();
                    for segment in &segments {
                        let button = pill_button(segment.label, -1.0, sz(STYLE_TAB_H));
                        // Contract-id parity with the builtin tree, which
                        // materializes the halves as HitArea nodes.
                        set_semantic_widget_id(&button, segment.id);
                        button.add_css_class("tab");
                        button.set_tooltip_text(Some(&segment.tooltip));
                        set_active_class(&button, segment.active);
                        let sender = self.feedback.clone();
                        let event = segment.event.clone();
                        button.connect_clicked(move |_| {
                            send_event(&sender, event.clone());
                        });
                        handles.push((button.clone(), segment.id));
                        row.append(&button);
                    }
                    append_gap(&pill, row.upcast_ref(), gap);
                    self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                        let Some(segments) = control.segments(snapshot) else {
                            return;
                        };
                        for (button, id) in &handles {
                            let active = segments
                                .iter()
                                .find(|segment| segment.id == *id)
                                .is_some_and(|segment| segment.active);
                            set_active_class(button, active);
                        }
                    }));
                }
            }
        }

        self.root.append(&pill);

        // The pill fades with the islands above it.
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            pill.set_opacity(snapshot.top_fade.clamp(0.0, 1.0));
        }));
    }
}
