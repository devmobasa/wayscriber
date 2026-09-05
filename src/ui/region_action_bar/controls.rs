use super::layout::RegionActionItem;
use super::model::RegionAction;
use super::{TOGGLE_FONT_SIZE, status_label_style};
use crate::ui::primitives::{draw_keycap_with_engine, draw_rounded_rect, keycap_size_with_engine};
use crate::ui::theme::{self, Rgba, overlay};
use crate::ui_text::{UiTextEngine, UiTextStyle};

const ITEM_RADIUS: f64 = overlay::RADIUS_MD;
const LABEL_FONT_SIZE: f64 = 11.0;
const KEYCAP_FONT_SIZE: f64 = 8.5;
/// Gap between an action's label and the keycap chip under it.
const LABEL_KEYCAP_GAP: f64 = 3.0;

const ITEM_BG: Rgba = (1.0, 1.0, 1.0, 0.06);
const ITEM_BORDER: Rgba = (1.0, 1.0, 1.0, 0.10);
const ITEM_BG_HOVER: Rgba = overlay::BG_HOVER;
const ITEM_BORDER_HOVER: Rgba = overlay::BORDER_FOCUS;
/// `Both` is what Enter does, so it carries the accent as the bar's default
/// action. Resting alpha stays below full so hover still reads as a change.
const PRIMARY_BG: Rgba = theme::rgba(theme::ACCENT_RGB, 0.80);
const PRIMARY_BG_HOVER: Rgba = theme::rgba(theme::ACCENT_RGB, 1.0);
const PRIMARY_BORDER: Rgba = theme::rgba(theme::ACCENT_BRIGHT_RGB, 0.45);

const KEYCAP_BG: Rgba = (1.0, 1.0, 1.0, 0.10);
const KEYCAP_BG_ON_ACCENT: Rgba = (1.0, 1.0, 1.0, 0.20);

const CHECKBOX_SIZE: f64 = 14.0;
const CHECKBOX_BORDER: Rgba = (1.0, 1.0, 1.0, 0.38);
const CHECKBOX_BG: Rgba = (1.0, 1.0, 1.0, 0.06);
const CHECKBOX_BG_CHECKED: Rgba = theme::rgba(theme::ACCENT_RGB, 0.95);
const TOGGLE_BG_HOVER: Rgba = (1.0, 1.0, 1.0, 0.07);

pub(super) fn draw_action(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    item: RegionActionItem,
    hovered: bool,
    enabled: bool,
    selected: bool,
) {
    if item.bounds.width <= 0.0 || item.bounds.height <= 0.0 {
        return;
    }
    let primary = item.action.is_primary();
    let (mut fill, mut border) = match (primary, hovered || selected) {
        (true, false) => (PRIMARY_BG, PRIMARY_BORDER),
        (true, true) => (PRIMARY_BG_HOVER, ITEM_BORDER_HOVER),
        (false, false) => (ITEM_BG, ITEM_BORDER),
        (false, true) => (ITEM_BG_HOVER, ITEM_BORDER_HOVER),
    };
    if selected && !primary {
        fill = theme::rgba(theme::ACCENT_RGB, 0.35);
        border = ITEM_BORDER_HOVER;
    }
    if !enabled {
        fill.3 *= 0.45;
        border.3 *= 0.45;
    }

    theme::set_color(ctx, fill);
    draw_rounded_rect(
        ctx,
        item.bounds.x,
        item.bounds.y,
        item.bounds.width,
        item.bounds.height,
        ITEM_RADIUS,
    );
    let _ = ctx.fill();

    theme::set_color(ctx, border);
    ctx.set_line_width(1.0);
    draw_rounded_rect(
        ctx,
        item.bounds.x + 0.5,
        item.bounds.y + 0.5,
        (item.bounds.width - 1.0).max(0.0),
        (item.bounds.height - 1.0).max(0.0),
        (ITEM_RADIUS - 0.5).max(0.0),
    );
    let _ = ctx.stroke();

    draw_action_content(engine, ctx, item, primary, enabled);
}

/// Label over keycap, the pair centred as one block so every control's text
/// sits on the same optical line regardless of ascenders or descenders.
fn draw_action_content(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    item: RegionActionItem,
    primary: bool,
    enabled: bool,
) {
    let _ = ctx.save();
    ctx.rectangle(
        item.bounds.x,
        item.bounds.y,
        item.bounds.width,
        item.bounds.height,
    );
    ctx.clip();

    let center_x = item.bounds.x + item.bounds.width / 2.0;
    let label = item.action.label();
    let layout = engine.layout(ctx, label_style(), label, None);
    let label_extents = layout.ink_extents();
    let shortcut = item.action.shortcut();
    let (keycap_width, keycap_height) = if shortcut.is_empty() {
        (0.0, 0.0)
    } else {
        keycap_size_with_engine(engine, ctx, shortcut, KEYCAP_FONT_SIZE)
    };

    let stack_height = if shortcut.is_empty() {
        label_extents.height()
    } else {
        label_extents.height() + LABEL_KEYCAP_GAP + keycap_height
    };
    let stack_top = item.bounds.y + (item.bounds.height - stack_height) / 2.0;
    let text_color = if !enabled {
        overlay::TEXT_TERTIARY
    } else if primary {
        overlay::TEXT_WHITE
    } else {
        overlay::TEXT_PRIMARY
    };

    theme::set_color(ctx, text_color);
    layout.show_at_baseline(
        ctx,
        center_x - label_extents.width() / 2.0 - label_extents.x_bearing(),
        stack_top - label_extents.y_bearing(),
    );

    if !shortcut.is_empty() {
        draw_keycap_with_engine(
            engine,
            ctx,
            center_x - keycap_width / 2.0,
            stack_top + label_extents.height() + LABEL_KEYCAP_GAP,
            shortcut,
            KEYCAP_FONT_SIZE,
            if primary {
                KEYCAP_BG_ON_ACCENT
            } else {
                KEYCAP_BG
            },
            if primary {
                overlay::TEXT_WHITE
            } else {
                overlay::TEXT_HINT
            },
        );
    }
    let _ = ctx.restore();
}

pub(super) fn draw_toggle(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    item: RegionActionItem,
    hovered: Option<RegionAction>,
    checked: bool,
) {
    if item.bounds.width <= 0.0 || item.bounds.height <= 0.0 {
        return;
    }
    let _ = ctx.save();
    ctx.rectangle(
        item.bounds.x,
        item.bounds.y,
        item.bounds.width,
        item.bounds.height,
    );
    ctx.clip();

    // The row itself stays quiet: the checkbox carries the on/off state, so an
    // enabled toggle no longer paints a full-width slab across the bar.
    if hovered == Some(item.action) {
        theme::set_color(ctx, TOGGLE_BG_HOVER);
        draw_rounded_rect(
            ctx,
            item.bounds.x,
            item.bounds.y,
            item.bounds.width,
            item.bounds.height,
            ITEM_RADIUS,
        );
        let _ = ctx.fill();
    }

    let box_size = CHECKBOX_SIZE.min(item.bounds.height - 2.0).max(0.0);
    let box_x = item.bounds.x + 6.0;
    let box_y = item.bounds.y + (item.bounds.height - box_size) / 2.0;
    draw_checkbox(ctx, box_x, box_y, box_size, checked);

    let label = item.action.label();
    let layout = engine.layout(ctx, toggle_label_style(), label, None);
    let extents = layout.ink_extents();
    theme::set_color(
        ctx,
        if checked {
            overlay::TEXT_PRIMARY
        } else {
            overlay::TEXT_TERTIARY
        },
    );
    layout.show_at_baseline(
        ctx,
        box_x + box_size + 8.0 - extents.x_bearing(),
        item.bounds.y + (item.bounds.height - extents.height()) / 2.0 - extents.y_bearing(),
    );

    let (keycap_width, keycap_height) =
        keycap_size_with_engine(engine, ctx, item.action.shortcut(), KEYCAP_FONT_SIZE);
    draw_keycap_with_engine(
        engine,
        ctx,
        item.bounds.x + item.bounds.width - 6.0 - keycap_width,
        item.bounds.y + (item.bounds.height - keycap_height) / 2.0,
        item.action.shortcut(),
        KEYCAP_FONT_SIZE,
        KEYCAP_BG,
        overlay::TEXT_HINT,
    );
    let _ = ctx.restore();
}

fn draw_checkbox(ctx: &cairo::Context, x: f64, y: f64, size: f64, checked: bool) {
    if size <= 0.0 {
        return;
    }
    theme::set_color(
        ctx,
        if checked {
            CHECKBOX_BG_CHECKED
        } else {
            CHECKBOX_BG
        },
    );
    draw_rounded_rect(ctx, x, y, size, size, overlay::RADIUS_SM);
    let _ = ctx.fill();

    theme::set_color(
        ctx,
        if checked {
            theme::rgba(theme::ACCENT_BRIGHT_RGB, 0.8)
        } else {
            CHECKBOX_BORDER
        },
    );
    ctx.set_line_width(1.0);
    draw_rounded_rect(
        ctx,
        x + 0.5,
        y + 0.5,
        (size - 1.0).max(0.0),
        (size - 1.0).max(0.0),
        (overlay::RADIUS_SM - 0.5).max(0.0),
    );
    let _ = ctx.stroke();

    if !checked {
        return;
    }
    theme::set_color(ctx, overlay::TEXT_WHITE);
    ctx.set_line_width((size * 0.14).max(1.4));
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.move_to(x + size * 0.26, y + size * 0.52);
    ctx.line_to(x + size * 0.44, y + size * 0.70);
    ctx.line_to(x + size * 0.76, y + size * 0.32);
    let _ = ctx.stroke();
}

fn label_style() -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: LABEL_FONT_SIZE,
    }
}

fn toggle_label_style() -> UiTextStyle<'static> {
    status_label_style(TOGGLE_FONT_SIZE)
}
