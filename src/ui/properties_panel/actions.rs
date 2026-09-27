//! The panel's actions area: the four ordering buttons, and Duplicate and
//! Delete under them.

use cairo::FontWeight;

use crate::draw::Color;
use crate::input::state::properties_panel_metrics::{
    ACTION_BUTTON_HEIGHT, ACTION_ROW_GAP, ACTIONS_TOP_GAP, BODY_FONT, VALUE_FONT, text_style,
};
use crate::input::state::{
    PanelAction, PanelRect, PropertiesPanelHit, PropertiesPanelLayout, ShapePropertiesPanel,
};
use crate::ui::primitives::draw_rounded_rect;
use crate::ui::theme::overlay::{
    ACCENT_BRIGHT, ACCENT_PRIMARY, BG_HOVER, BG_HOVER_WASH, DIVIDER_LIGHT, RADIUS_STD,
    TEXT_DISABLED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use crate::ui::theme::{DESTRUCTIVE_RGB, Rgba, rgba, set_color, with_alpha};
use crate::ui_text::UiTextEngine;

/// Delete's label, in the destructive red the toolbar's Clear uses.
const DELETE_TEXT: Rgba = rgba(DESTRUCTIVE_RGB, 1.0);
const GLYPH_WIDTH: f64 = 1.6;

pub(super) fn draw_actions(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    panel: &ShapePropertiesPanel,
    layout: &PropertiesPanelLayout,
) {
    set_color(ctx, with_alpha(DIVIDER_LIGHT, 0.4));
    ctx.set_line_width(1.0);
    let divider = layout.actions_top + ACTIONS_TOP_GAP / 2.0 - 0.5;
    ctx.move_to(layout.content_x(), divider);
    ctx.line_to(layout.content_right(), divider);
    let _ = ctx.stroke();

    set_color(ctx, TEXT_PRIMARY);
    let label_style = text_style(BODY_FONT, FontWeight::Normal);
    let order_top = layout.actions_top + ACTIONS_TOP_GAP;
    let preset_top = order_top + (ACTION_BUTTON_HEIGHT + ACTION_ROW_GAP) * 2.0;
    for (label, top) in [("Order", order_top), ("Preset", preset_top)] {
        engine.draw_baseline(
            ctx,
            label_style,
            label,
            layout.content_x(),
            top + ACTION_BUTTON_HEIGHT / 2.0 + BODY_FONT * 0.35,
            None,
        );
    }

    for (action, rect) in layout.action_buttons() {
        let enabled = panel.action_enabled(action);
        let hovered = enabled && panel.hover == Some(PropertiesPanelHit::Action(action));
        let armed = action == PanelAction::SavePreset && panel.preset_save_mode;
        set_color(
            ctx,
            if armed {
                ACCENT_PRIMARY
            } else if hovered {
                BG_HOVER
            } else {
                BG_HOVER_WASH
            },
        );
        draw_rounded_rect(ctx, rect.x, rect.y, rect.width, rect.height, RADIUS_STD);
        let _ = ctx.fill();
        if panel.preset_save_mode && matches!(action, PanelAction::Preset(_)) {
            // Saving is armed: the slots are what to click next.
            set_color(ctx, with_alpha(ACCENT_BRIGHT, 0.7));
            ctx.set_line_width(1.0);
            draw_rounded_rect(
                ctx,
                rect.x + 0.5,
                rect.y + 0.5,
                rect.width - 1.0,
                rect.height - 1.0,
                RADIUS_STD,
            );
            let _ = ctx.stroke();
        }

        let ink = match (enabled, action) {
            (false, _) => TEXT_DISABLED,
            (true, PanelAction::Delete) => DELETE_TEXT,
            (true, _) if hovered || armed => TEXT_PRIMARY,
            (true, _) => TEXT_SECONDARY,
        };
        set_color(ctx, ink);
        match action {
            PanelAction::Duplicate => draw_label(engine, ctx, rect, "Duplicate"),
            PanelAction::Delete => draw_label(engine, ctx, rect, "Delete"),
            PanelAction::SavePreset => draw_label(engine, ctx, rect, "Save"),
            PanelAction::Preset(slot) => {
                let preset = panel.actions.presets.get(slot - 1).and_then(Option::as_ref);
                draw_preset_chip(engine, ctx, rect, slot, preset.map(|p| p.color), ink);
            }
            order => draw_order_glyph(ctx, rect, order),
        }
    }
}

/// A preset slot: its number beside a dot of the preset's color, or an
/// empty ring for a slot with nothing saved.
fn draw_preset_chip(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    rect: PanelRect,
    slot: usize,
    color: Option<Color>,
    ink: Rgba,
) {
    let style = text_style(VALUE_FONT, FontWeight::Normal);
    let number = slot.to_string();
    let number_width = engine
        .layout(ctx, style, &number, None)
        .ink_extents()
        .x_advance();
    let dot = 4.5;
    let gap = 4.0;
    let (cx, cy) = rect.center();
    let left = cx - (dot * 2.0 + gap + number_width) / 2.0;

    ctx.new_path();
    ctx.arc(left + dot, cy, dot, 0.0, std::f64::consts::TAU);
    match color {
        Some(color) => {
            set_color(ctx, (color.r, color.g, color.b, color.a.max(0.35)));
            let _ = ctx.fill();
        }
        None => {
            set_color(ctx, ink);
            ctx.set_line_width(1.0);
            let _ = ctx.stroke();
        }
    }
    set_color(ctx, ink);
    engine.draw_baseline(
        ctx,
        style,
        &number,
        left + dot * 2.0 + gap,
        cy + VALUE_FONT * 0.35,
        None,
    );
}

fn draw_label(engine: &UiTextEngine, ctx: &cairo::Context, rect: PanelRect, label: &str) {
    let style = text_style(VALUE_FONT, FontWeight::Normal);
    let width = engine
        .layout(ctx, style, label, None)
        .ink_extents()
        .x_advance();
    let (cx, cy) = rect.center();
    engine.draw_baseline(
        ctx,
        style,
        label,
        cx - width / 2.0,
        cy + VALUE_FONT * 0.35,
        None,
    );
}

/// An arrow pointing the way the shape moves; to back and to front add a bar
/// at the end of the stack it goes to.
fn draw_order_glyph(ctx: &cairo::Context, rect: PanelRect, action: PanelAction) {
    let (cx, cy) = rect.center();
    let up = matches!(action, PanelAction::Forward | PanelAction::ToFront);
    let to_end = matches!(action, PanelAction::ToBack | PanelAction::ToFront);
    let (tail, tip) = match (up, to_end) {
        (true, true) => (cy + 6.0, cy - 3.0),
        (true, false) => (cy + 6.0, cy - 6.0),
        (false, true) => (cy - 6.0, cy + 3.0),
        (false, false) => (cy - 6.0, cy + 6.0),
    };
    let back = if up { tip + 4.0 } else { tip - 4.0 };

    ctx.set_line_width(GLYPH_WIDTH);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.move_to(cx, tail);
    ctx.line_to(cx, tip);
    ctx.move_to(cx - 4.0, back);
    ctx.line_to(cx, tip);
    ctx.line_to(cx + 4.0, back);
    if to_end {
        let bar = if up { cy - 6.5 } else { cy + 6.5 };
        ctx.move_to(cx - 6.0, bar);
        ctx.line_to(cx + 6.0, bar);
    }
    let _ = ctx.stroke();
}
