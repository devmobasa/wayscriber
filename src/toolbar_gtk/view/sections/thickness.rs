//! Thickness block: the thickness slider, eraser mode toggle, and polygon
//! sides cards the built-in palette draws together.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::config::Action;
use crate::input::EraserMode;
use crate::label_format::format_binding_label;
use crate::toolbar_icons;
use crate::ui::toolbar::snapshot::ToolContext;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection, model};

use super::super::super::widgets::{SliderRow, icon_button, send_event};
use super::{SectionCtx, scoped_title, section_card};

/// Spec-unit sizes from the built-in side palette (`SIDE_NUDGE_SIZE`,
/// `SIDE_NUDGE_ICON_SIZE`).
const NUDGE_BUTTON_SIZE: f64 = 24.0;
const NUDGE_ICON_SIZE: f64 = 14.0;

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let snapshot = ctx.snapshot;
    let tool_context = ToolContext::from_snapshot(snapshot);
    let block = gtk4::Box::new(gtk4::Orientation::Vertical, ctx.px(12.0));

    if !snapshot.side_section_hidden(ToolbarSideSection::Thickness) {
        block.append(&thickness_card(ctx, &tool_context));
    }
    if tool_context.show_eraser_mode
        && !snapshot.side_section_hidden(ToolbarSideSection::EraserMode)
    {
        block.append(&eraser_mode_card(ctx));
    }
    if tool_context.show_polygon_sides_control
        && !snapshot.side_section_hidden(ToolbarSideSection::PolygonSides)
    {
        block.append(&polygon_sides_card(ctx));
    }
    block.first_child()?;
    Some(block.upcast())
}

fn thickness_card(ctx: &mut SectionCtx, tool_context: &ToolContext) -> gtk4::Widget {
    let snapshot = ctx.snapshot;
    // Generic titles gain the tool scope; specific ones ("Eraser size")
    // already name their target.
    let title = if tool_context.thickness_label == "Thickness" {
        scoped_title(tool_context.thickness_label, snapshot)
    } else {
        tool_context.thickness_label.to_string()
    };
    let card = section_card(ctx, ToolbarSideSection::Thickness, &title);
    if snapshot.side_section_collapsed(ToolbarSideSection::Thickness) {
        return card.root.upcast();
    }

    let spec = model::ToolbarSliderSpec::THICKNESS;
    let nudge_step = spec.step.unwrap_or(1.0);
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(6.0));
    let btn = ctx.sz(NUDGE_BUTTON_SIZE);
    let icon = ctx.sz(NUDGE_ICON_SIZE);

    let minus = icon_button(
        toolbar_icons::draw_icon_minus,
        (btn, btn),
        icon,
        "Decrease thickness",
    );
    let sender = ctx.feedback.clone();
    minus.button.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::NudgeThickness(-nudge_step));
    });
    row.append(&minus.button);

    // `snapshot.thickness` already carries the active target's value
    // (eraser size when `thickness_targets_eraser`), like the built-in.
    let sender = ctx.feedback.clone();
    let slider = SliderRow::new(
        ctx.scale,
        (spec.min, spec.max),
        snapshot.thickness,
        format_px,
        move |value| {
            send_event(&sender, ToolbarEvent::SetThickness(value));
        },
    );
    slider.root.set_hexpand(true);
    row.append(&slider.root);

    let plus = icon_button(
        toolbar_icons::draw_icon_plus,
        (btn, btn),
        icon,
        "Increase thickness",
    );
    let sender = ctx.feedback.clone();
    plus.button.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::NudgeThickness(nudge_step));
    });
    row.append(&plus.button);

    card.body.append(&row);
    ctx.updaters.push(Box::new(move |snapshot| {
        slider.set_value(snapshot.thickness);
    }));
    card.root.upcast()
}

fn format_px(value: f64) -> String {
    format!("{value:.0}px")
}

fn eraser_mode_card(ctx: &mut SectionCtx) -> gtk4::Widget {
    let snapshot = ctx.snapshot;
    let card = section_card(ctx, ToolbarSideSection::EraserMode, "Eraser mode");
    if snapshot.side_section_collapsed(ToolbarSideSection::EraserMode) {
        return card.root.upcast();
    }

    let toggle = gtk4::CheckButton::with_label("Erase by stroke");
    toggle.set_tooltip_text(Some(&format_binding_label(
        "Erase by stroke",
        snapshot
            .binding_hints
            .binding_for_action(Action::ToggleEraserMode),
    )));
    toggle.set_active(snapshot.eraser_mode == EraserMode::Stroke);
    let sender = ctx.feedback.clone();
    let syncing = Rc::new(Cell::new(false));
    let toggle_sync = syncing.clone();
    toggle.connect_toggled(move |check| {
        if !toggle_sync.get() {
            send_event(
                &sender,
                ToolbarEvent::SetEraserMode(if check.is_active() {
                    EraserMode::Stroke
                } else {
                    EraserMode::Brush
                }),
            );
        }
    });
    card.body.append(&toggle);
    ctx.updaters.push(Box::new(move |snapshot| {
        let stroke_active = snapshot.eraser_mode == EraserMode::Stroke;
        if toggle.is_active() != stroke_active {
            syncing.set(true);
            toggle.set_active(stroke_active);
            syncing.set(false);
        }
    }));
    card.root.upcast()
}

fn polygon_sides_card(ctx: &mut SectionCtx) -> gtk4::Widget {
    let snapshot = ctx.snapshot;
    let card = section_card(ctx, ToolbarSideSection::PolygonSides, "Sides");
    if snapshot.side_section_collapsed(ToolbarSideSection::PolygonSides) {
        return card.root.upcast();
    }

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(6.0));
    let btn = ctx.sz(NUDGE_BUTTON_SIZE);
    let icon = ctx.sz(NUDGE_ICON_SIZE);

    let minus = icon_button(
        toolbar_icons::draw_icon_minus,
        (btn, btn),
        icon,
        "Decrease polygon sides",
    );
    let sender = ctx.feedback.clone();
    minus.button.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::NudgePolygonSides(-1));
    });
    row.append(&minus.button);

    let label = gtk4::Label::new(Some(&format!("{} sides", snapshot.polygon_sides)));
    label.set_hexpand(true);
    row.append(&label);

    let plus = icon_button(
        toolbar_icons::draw_icon_plus,
        (btn, btn),
        icon,
        "Increase polygon sides",
    );
    let sender = ctx.feedback.clone();
    plus.button.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::NudgePolygonSides(1));
    });
    row.append(&plus.button);

    card.body.append(&row);
    ctx.updaters.push(Box::new(move |snapshot| {
        label.set_text(&format!("{} sides", snapshot.polygon_sides));
    }));
    card.root.upcast()
}
