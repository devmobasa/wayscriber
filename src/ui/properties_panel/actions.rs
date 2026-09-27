//! The panel's actions area: the four ordering buttons, and Duplicate and
//! Delete under them.

use cairo::FontWeight;

use crate::input::state::properties_panel_metrics::{
    ACTION_BUTTON_HEIGHT, ACTIONS_TOP_GAP, BODY_FONT, VALUE_FONT, text_style,
};
use crate::input::state::{
    PanelAction, PanelRect, PropertiesPanelHit, PropertiesPanelLayout, ShapePropertiesPanel,
};
use crate::ui::primitives::draw_rounded_rect;
use crate::ui::theme::overlay::{
    BG_HOVER, BG_HOVER_WASH, DIVIDER_LIGHT, RADIUS_STD, TEXT_DISABLED, TEXT_PRIMARY, TEXT_SECONDARY,
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
    engine.draw_baseline(
        ctx,
        text_style(BODY_FONT, FontWeight::Normal),
        "Order",
        layout.content_x(),
        layout.actions_top + ACTIONS_TOP_GAP + ACTION_BUTTON_HEIGHT / 2.0 + BODY_FONT * 0.35,
        None,
    );

    for (action, rect) in layout.action_buttons() {
        let enabled = panel.actions.enabled(action);
        let hovered = enabled && panel.hover == Some(PropertiesPanelHit::Action(action));
        set_color(ctx, if hovered { BG_HOVER } else { BG_HOVER_WASH });
        draw_rounded_rect(ctx, rect.x, rect.y, rect.width, rect.height, RADIUS_STD);
        let _ = ctx.fill();

        let ink = match (enabled, action) {
            (false, _) => TEXT_DISABLED,
            (true, PanelAction::Delete) => DELETE_TEXT,
            (true, _) if hovered => TEXT_PRIMARY,
            (true, _) => TEXT_SECONDARY,
        };
        set_color(ctx, ink);
        match action {
            PanelAction::Duplicate => draw_label(engine, ctx, rect, "Duplicate"),
            PanelAction::Delete => draw_label(engine, ctx, rect, "Delete"),
            order => draw_order_glyph(ctx, rect, order),
        }
    }
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
