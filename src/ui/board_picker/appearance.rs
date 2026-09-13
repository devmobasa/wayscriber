use crate::domain::BoardGridKind;
use crate::input::InputState;
use crate::input::state::AppearanceField;
use crate::ui::constants::{self, INPUT_CARET, OVERLAY_DIM_MEDIUM, TEXT_HINT, TEXT_PRIMARY};
use crate::ui::primitives::draw_rounded_rect;
use crate::ui::theme::{Rgba, popup};
use crate::ui_text::{UiTextEngine, UiTextStyle};

use super::palette::{PALETTE_SWATCH_GAP, PALETTE_SWATCH_SIZE};

const SHEET_RADIUS: f64 = 10.0;
/// Two-layer drop shadow that lifts the sheet off the dimmed picker.
const SHEET_SHADOW_SOFT: Rgba = (0.0, 0.0, 0.0, 0.25);
const SHEET_SHADOW: Rgba = (0.0, 0.0, 0.0, 0.35);
const HEADER_RULE: Rgba = (1.0, 1.0, 1.0, 0.08);
const FIELD_BG: Rgba = (0.10, 0.10, 0.12, 1.0);
const FIELD_BORDER: Rgba = (1.0, 1.0, 1.0, 0.18);
const CHIP_EDGE: Rgba = (1.0, 1.0, 1.0, 0.25);

pub(super) fn render(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    input: &InputState,
    screen_width: u32,
    screen_height: u32,
) {
    let Some(edit) = input.board_appearance_edit() else {
        return;
    };
    let (Some((x, y, width)), Some(header)) = (
        input.board_appearance_rect(),
        input.board_appearance_header(),
    ) else {
        return;
    };
    let style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 12.0,
    };
    let _ = ctx.save();
    draw_backdrop(ctx, input, screen_width, screen_height);
    draw_frame(ctx, x, y, width);

    let board_name = input
        .boards
        .board_states()
        .iter()
        .find(|board| board.spec.id == edit.board_id())
        .map_or("", |board| board.spec.name.as_str());
    let _ = ctx.save();
    ctx.rectangle(x, y - 66.0, (header.title_right - x).max(0.0), 32.0);
    ctx.clip();
    constants::set_color(ctx, TEXT_PRIMARY);
    engine.draw_baseline(
        ctx,
        UiTextStyle {
            weight: cairo::FontWeight::Bold,
            ..style
        },
        &format!("Paper — {board_name}"),
        x + 4.0,
        y - 45.0,
        None,
    );
    let _ = ctx.restore();

    let (field_x, field_y, field_width, field_height) = header.color_field;
    let color_focused = edit.focus == AppearanceField::Color;
    draw_rounded_rect(ctx, field_x, field_y, field_width, field_height, 4.0);
    constants::set_color(ctx, FIELD_BG);
    let _ = ctx.fill_preserve();
    constants::set_color(
        ctx,
        if color_focused {
            INPUT_CARET
        } else {
            FIELD_BORDER
        },
    );
    ctx.set_line_width(if color_focused { 1.5 } else { 1.0 });
    let _ = ctx.stroke();
    let (draft_color, _) = edit.preview();
    ctx.set_source_rgb(draft_color.r, draft_color.g, draft_color.b);
    draw_rounded_rect(ctx, field_x + 6.0, field_y + 6.0, 10.0, 10.0, 2.0);
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, CHIP_EDGE);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
    let _ = ctx.save();
    ctx.rectangle(field_x, field_y, field_width - 4.0, field_height);
    ctx.clip();
    constants::set_color(ctx, TEXT_PRIMARY);
    engine.draw_baseline(
        ctx,
        UiTextStyle {
            size: 11.0,
            ..style
        },
        &edit.color,
        field_x + 22.0,
        field_y + 15.0,
        None,
    );
    let _ = ctx.restore();

    let (close_x, close_y, close_width, close_height) = header.close;
    let (center_x, center_y) = (close_x + close_width / 2.0, close_y + close_height / 2.0);
    constants::set_color(ctx, TEXT_HINT);
    ctx.set_line_width(1.5);
    ctx.move_to(center_x - 4.5, center_y - 4.5);
    ctx.line_to(center_x + 4.5, center_y + 4.5);
    ctx.move_to(center_x + 4.5, center_y - 4.5);
    ctx.line_to(center_x - 4.5, center_y + 4.5);
    let _ = ctx.stroke();

    constants::set_color(ctx, HEADER_RULE);
    ctx.set_line_width(1.0);
    ctx.move_to(x - 12.0, y - 33.5);
    ctx.line_to(x + width + 12.0, y - 33.5);
    let _ = ctx.stroke();

    for (index, color) in super::helpers::BOARD_PALETTE.iter().enumerate() {
        ctx.set_source_rgb(color.r, color.g, color.b);
        ctx.rectangle(
            x + index as f64 * width / 11.0 + 1.0,
            y - 28.0,
            width / 11.0 - 3.0,
            21.0,
        );
        let _ = ctx.fill();
    }
    for (index, kind) in BoardGridKind::ALL.iter().enumerate() {
        let left = x + (index % 2) as f64 * width / 2.0;
        let top = y + (index / 2) as f64 * 28.0;
        constants::set_color(
            ctx,
            if edit.kind == *kind {
                INPUT_CARET
            } else {
                TEXT_HINT
            },
        );
        ctx.rectangle(left + 1.0, top + 1.0, width / 2.0 - 5.0, 24.0);
        let _ = ctx.stroke();
        engine.draw_baseline(ctx, style, kind.label(), left + 6.0, top + 17.0, None);
    }
    constants::set_color(
        ctx,
        if edit.focus == AppearanceField::Spacing {
            INPUT_CARET
        } else {
            TEXT_PRIMARY
        },
    );
    let spacing_label = match edit.kind {
        BoardGridKind::None => "Spacing",
        BoardGridKind::Cartesian => "Square side",
        BoardGridKind::Isometric | BoardGridKind::IsometricDots => "Triangle side",
    };
    engine.draw_baseline(
        ctx,
        style,
        &format!("{spacing_label}: {} px", edit.spacing),
        x + 4.0,
        y + 79.0,
        None,
    );
    for (offset, preset) in [(80.0, 20), (40.0, 40)] {
        // Selection follows the size value; field focus only colors the label.
        constants::set_color(
            ctx,
            if edit.spacing_matches(preset) {
                INPUT_CARET
            } else {
                TEXT_HINT
            },
        );
        ctx.rectangle(x + width - offset, y + 60.0, 36.0, 25.0);
        let _ = ctx.stroke();
        engine.draw_baseline(
            ctx,
            style,
            &preset.to_string(),
            x + width - offset + 9.0,
            y + 78.0,
            None,
        );
    }
    let _ = ctx.save();
    ctx.rectangle(x, y + 94.0, width, 56.0);
    ctx.clip();
    ctx.translate(x, y + 94.0);
    let (color, grid) = edit.preview();
    if let Ok(paper) = crate::draw::BoardPaper::for_context(color, grid, ctx) {
        let _ = paper.paint(ctx);
    }
    let _ = ctx.restore();
    for (offset, label) in [(0.0, "Apply"), (width / 2.0, "Cancel")] {
        constants::set_color(
            ctx,
            if label == "Apply" && edit.validation_error().is_some() {
                TEXT_HINT
            } else {
                TEXT_PRIMARY
            },
        );
        ctx.rectangle(x + offset + 1.0, y + 160.0, width / 2.0 - 5.0, 26.0);
        let _ = ctx.stroke();
        engine.draw_baseline(ctx, style, label, x + offset + 8.0, y + 178.0, None);
    }
    constants::set_color(ctx, TEXT_HINT);
    let message = edit
        .error
        .as_deref()
        .or_else(|| edit.validation_error())
        .unwrap_or("Session only • Tab: field • arrows: pattern • Enter: Apply");
    engine.draw_baseline(
        ctx,
        UiTextStyle {
            size: 10.0,
            ..style
        },
        message,
        x,
        y + 202.0,
        Some(width),
    );
    let _ = ctx.restore();
}

/// Dims everything behind the sheet so it reads as a dialog above the picker.
/// The picker's own swatches stay bright because they still edit the draft.
fn draw_backdrop(ctx: &cairo::Context, input: &InputState, screen_width: u32, screen_height: u32) {
    ctx.new_path();
    ctx.rectangle(0.0, 0.0, f64::from(screen_width), f64::from(screen_height));
    if let Some(layout) = input.board_picker_layout()
        && layout.palette_rows > 0
        && layout.palette_cols > 0
    {
        let unit = PALETTE_SWATCH_SIZE + PALETTE_SWATCH_GAP;
        let margin = 5.0;
        rounded_sub_path(
            ctx,
            layout.origin_x + layout.padding_x - margin,
            layout.palette_top - margin,
            layout.palette_cols as f64 * unit - PALETTE_SWATCH_GAP + margin * 2.0,
            layout.palette_rows as f64 * unit - PALETTE_SWATCH_GAP + margin * 2.0,
            6.0,
        );
    }
    ctx.set_fill_rule(cairo::FillRule::EvenOdd);
    ctx.set_source_rgba(0.0, 0.0, 0.0, OVERLAY_DIM_MEDIUM);
    let _ = ctx.fill();
    ctx.set_fill_rule(cairo::FillRule::Winding);
}

/// Adds a rounded rectangle to the current path without clearing it.
fn rounded_sub_path(ctx: &cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    use std::f64::consts::{FRAC_PI_2, PI};

    let radius = radius.min(width / 2.0).min(height / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + width - radius, y + radius, radius, -FRAC_PI_2, 0.0);
    ctx.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        FRAC_PI_2,
    );
    ctx.arc(x + radius, y + height - radius, radius, FRAC_PI_2, PI);
    ctx.arc(x + radius, y + radius, radius, PI, PI + FRAC_PI_2);
    ctx.close_path();
}

fn draw_frame(ctx: &cairo::Context, x: f64, y: f64, width: f64) {
    let (left, top, frame_width, frame_height) = (x - 12.0, y - 70.0, width + 24.0, 292.0);
    for (offset, shadow) in [(10.0, SHEET_SHADOW_SOFT), (4.0, SHEET_SHADOW)] {
        constants::set_color(ctx, shadow);
        draw_rounded_rect(
            ctx,
            left,
            top + offset,
            frame_width,
            frame_height,
            SHEET_RADIUS,
        );
        let _ = ctx.fill();
    }

    draw_rounded_rect(ctx, left, top, frame_width, frame_height, SHEET_RADIUS);
    constants::set_color(ctx, constants::with_alpha(popup::bg_modal(), 1.0));
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, popup::border_modal());
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}
