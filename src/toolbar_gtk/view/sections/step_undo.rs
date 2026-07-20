//! Step Undo/Redo section: the "Step buttons" / "Delay sliders" toggles,
//! the per-direction step rows (multi-step button, minus / count / plus,
//! delay slider), and the global undo/redo delay sliders.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection, ToolbarSnapshot, model};

use super::super::super::icons::{IconPainter, IconWidget};
use super::super::super::widgets::{SliderRow, icon_button, send_event, sized_button};
use super::{SectionCtx, section_card};

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let snapshot = ctx.snapshot;
    if snapshot.side_section_hidden(ToolbarSideSection::StepUndo) || !snapshot.show_step_section {
        return None;
    }
    let card = section_card(
        ctx,
        ToolbarSideSection::StepUndo,
        ToolbarSideSection::StepUndo.label(),
    );
    populate(ctx, &card.body);
    Some(card.root.upcast())
}

/// The section's controls for the top strip's Canvas popover: the same
/// toggles, step rows, and delay sliders without the collapsible-card
/// chrome. Callers gate on `show_step_section`; liveness comes from the
/// popover host's content-key rebuild, so the updaters go to a scratch list.
pub(in crate::toolbar_gtk) fn build_popover_content(ctx: &mut SectionCtx) -> gtk4::Box {
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, ctx.px(6.0));
    populate(ctx, &body);
    body
}

fn populate(ctx: &mut SectionCtx, body: &gtk4::Box) {
    let snapshot = ctx.snapshot;
    // Both flags live in the structure/content key, so a toggle rebuilds the
    // section; the checkbox state never needs an updater.
    body.append(&toggle_checkbox(
        ctx,
        "Step buttons",
        "Step buttons: undo/redo several strokes at once.",
        snapshot.custom_section_enabled,
        ToolbarEvent::ToggleCustomSection,
    ));
    body.append(&toggle_checkbox(
        ctx,
        "Delay sliders",
        "Delay sliders: undo/redo delays.",
        snapshot.show_delay_sliders,
        ToolbarEvent::ToggleDelaySliders,
    ));

    if snapshot.custom_section_enabled {
        custom_row(ctx, body, true);
        custom_row(ctx, body, false);
    }
    if snapshot.show_delay_sliders {
        delay_sliders(ctx, body);
    }
}

fn toggle_checkbox(
    ctx: &SectionCtx,
    label: &str,
    tooltip: &str,
    active: bool,
    event: fn(bool) -> ToolbarEvent,
) -> gtk4::CheckButton {
    let check = gtk4::CheckButton::with_label(label);
    check.set_active(active);
    check.set_tooltip_text(Some(tooltip));
    let sender = ctx.feedback.clone();
    check.connect_toggled(move |check| {
        send_event(&sender, event(check.is_active()));
    });
    check
}

/// One Step Undo/Redo row: the multi-step button, the minus / "N steps" /
/// plus cluster, and the per-direction delay slider underneath.
fn custom_row(ctx: &mut SectionCtx, body: &gtk4::Box, is_undo: bool) {
    let snapshot = ctx.snapshot;
    let row_h = ctx.sz(26.0);
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(6.0));

    let tooltip = if is_undo { "Step undo" } else { "Step redo" };
    let step_button = if ctx.use_icons {
        let button = sized_button(ctx.sz(42.0), row_h);
        let icon = IconWidget::new(
            if is_undo {
                toolbar_icons::draw_icon_step_undo
            } else {
                toolbar_icons::draw_icon_step_redo
            },
            ctx.sz(20.0),
        );
        button.set_child(Some(&icon.area));
        button
    } else {
        // The built-in row draws the label left-aligned with a 10px inset.
        let button = sized_button(ctx.sz(90.0), row_h);
        let text = gtk4::Label::new(Some(if is_undo { "Step Undo" } else { "Step Redo" }));
        text.set_xalign(0.0);
        text.set_margin_start(ctx.px(10.0));
        button.set_child(Some(&text));
        button
    };
    step_button.set_tooltip_text(Some(tooltip));
    let sender = ctx.feedback.clone();
    step_button.connect_clicked(move |_| {
        send_event(
            &sender,
            if is_undo {
                ToolbarEvent::CustomUndo
            } else {
                ToolbarEvent::CustomRedo
            },
        );
    });
    row.append(&step_button);

    // The nudge events read the live count so a click after a backend
    // update never over- or under-shoots.
    let steps = Rc::new(Cell::new(if is_undo {
        snapshot.custom_undo_steps
    } else {
        snapshot.custom_redo_steps
    }));
    let cluster = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(4.0));

    let minus = nudge_button(
        ctx,
        toolbar_icons::draw_icon_minus,
        if is_undo {
            "Decrease undo steps"
        } else {
            "Decrease redo steps"
        },
    );
    let minus_sender = ctx.feedback.clone();
    let minus_steps = steps.clone();
    minus.connect_clicked(move |_| {
        let next = minus_steps.get().saturating_sub(1).max(1);
        send_event(&minus_sender, set_steps_event(is_undo, next));
    });
    cluster.append(&minus);

    let steps_label = gtk4::Label::new(Some(&steps_text(steps.get())));
    steps_label.set_size_request(ctx.px(54.0), -1);
    cluster.append(&steps_label);

    let plus = nudge_button(
        ctx,
        toolbar_icons::draw_icon_plus,
        if is_undo {
            "Increase undo steps"
        } else {
            "Increase redo steps"
        },
    );
    let plus_sender = ctx.feedback.clone();
    let plus_steps = steps.clone();
    plus.connect_clicked(move |_| {
        let next = plus_steps.get().saturating_add(1);
        send_event(&plus_sender, set_steps_event(is_undo, next));
    });
    cluster.append(&plus);
    row.append(&cluster);
    body.append(&row);

    let spec = model::ToolbarSliderSpec::DELAY_SECONDS;
    let initial_secs = row_delay_secs(snapshot, is_undo);
    let slider_sender = ctx.feedback.clone();
    let slider = SliderRow::new(
        ctx.scale,
        (spec.min, spec.max),
        initial_secs,
        format_secs,
        move |value| {
            send_event(
                &slider_sender,
                if is_undo {
                    ToolbarEvent::SetCustomUndoDelay(value)
                } else {
                    ToolbarEvent::SetCustomRedoDelay(value)
                },
            );
        },
    );
    slider
        .root
        .set_tooltip_text(Some(&row_delay_tooltip(is_undo, initial_secs)));
    body.append(&slider.root);

    ctx.updaters.push(Box::new(move |snapshot| {
        let count = if is_undo {
            snapshot.custom_undo_steps
        } else {
            snapshot.custom_redo_steps
        };
        steps.set(count);
        steps_label.set_text(&steps_text(count));
        let secs = row_delay_secs(snapshot, is_undo);
        slider.set_value(secs);
        slider
            .root
            .set_tooltip_text(Some(&row_delay_tooltip(is_undo, secs)));
    }));
}

/// Global Undo/Redo delay sliders: the built-in card puts both labels on
/// one line and stacks the two full-width sliders beneath.
fn delay_sliders(ctx: &mut SectionCtx, body: &gtk4::Box) {
    let snapshot = ctx.snapshot;
    let labels = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(10.0));
    labels.set_homogeneous(true);
    let undo_label = gtk4::Label::new(Some(&global_delay_label(true, snapshot.undo_all_delay_ms)));
    undo_label.add_css_class("hint");
    undo_label.set_xalign(0.0);
    labels.append(&undo_label);
    let redo_label = gtk4::Label::new(Some(&global_delay_label(false, snapshot.redo_all_delay_ms)));
    redo_label.add_css_class("hint");
    redo_label.set_xalign(0.0);
    labels.append(&redo_label);
    body.append(&labels);

    let undo_slider = global_delay_slider(ctx, true, snapshot.undo_all_delay_ms);
    body.append(&undo_slider.root);
    let redo_slider = global_delay_slider(ctx, false, snapshot.redo_all_delay_ms);
    body.append(&redo_slider.root);

    ctx.updaters.push(Box::new(move |snapshot| {
        undo_label.set_text(&global_delay_label(true, snapshot.undo_all_delay_ms));
        undo_slider.set_value(snapshot.undo_all_delay_ms as f64 / 1000.0);
        undo_slider
            .root
            .set_tooltip_text(Some(&global_delay_tooltip(
                true,
                snapshot.undo_all_delay_ms,
            )));
        redo_label.set_text(&global_delay_label(false, snapshot.redo_all_delay_ms));
        redo_slider.set_value(snapshot.redo_all_delay_ms as f64 / 1000.0);
        redo_slider
            .root
            .set_tooltip_text(Some(&global_delay_tooltip(
                false,
                snapshot.redo_all_delay_ms,
            )));
    }));
}

fn global_delay_slider(ctx: &SectionCtx, is_undo: bool, delay_ms: u64) -> SliderRow {
    let spec = model::ToolbarSliderSpec::DELAY_SECONDS;
    let sender = ctx.feedback.clone();
    let slider = SliderRow::new(
        ctx.scale,
        (spec.min, spec.max),
        delay_ms as f64 / 1000.0,
        format_secs,
        move |value| {
            send_event(
                &sender,
                if is_undo {
                    ToolbarEvent::SetUndoDelay(value)
                } else {
                    ToolbarEvent::SetRedoDelay(value)
                },
            );
        },
    );
    slider
        .root
        .set_tooltip_text(Some(&global_delay_tooltip(is_undo, delay_ms)));
    slider
}

fn nudge_button(ctx: &SectionCtx, painter: IconPainter, tooltip: &str) -> gtk4::Button {
    icon_button(painter, (ctx.sz(26.0), ctx.sz(26.0)), ctx.sz(14.0), tooltip).button
}

fn set_steps_event(is_undo: bool, steps: usize) -> ToolbarEvent {
    if is_undo {
        ToolbarEvent::SetCustomUndoSteps(steps)
    } else {
        ToolbarEvent::SetCustomRedoSteps(steps)
    }
}

fn format_secs(value: f64) -> String {
    format!("{value:.1}s")
}

fn steps_text(steps: usize) -> String {
    format!("{steps} steps")
}

fn row_delay_secs(snapshot: &ToolbarSnapshot, is_undo: bool) -> f64 {
    let delay_ms = if is_undo {
        snapshot.custom_undo_delay_ms
    } else {
        snapshot.custom_redo_delay_ms
    };
    delay_ms as f64 / 1000.0
}

fn row_delay_tooltip(is_undo: bool, secs: f64) -> String {
    if is_undo {
        format!("Undo step delay: {secs:.1}s (drag)")
    } else {
        format!("Redo step delay: {secs:.1}s (drag)")
    }
}

fn global_delay_label(is_undo: bool, delay_ms: u64) -> String {
    let secs = delay_ms as f64 / 1000.0;
    if is_undo {
        format!("Undo delay: {secs:.1}s")
    } else {
        format!("Redo delay: {secs:.1}s")
    }
}

fn global_delay_tooltip(is_undo: bool, delay_ms: u64) -> String {
    let secs = delay_ms as f64 / 1000.0;
    if is_undo {
        format!("Undo-all delay: {secs:.1}s (drag)")
    } else {
        format!("Redo-all delay: {secs:.1}s (drag)")
    }
}
