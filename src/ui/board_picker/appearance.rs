use crate::domain::BoardGridKind;
use crate::input::InputState;
use crate::input::state::AppearanceField;
use crate::ui::constants::{
    self, ACCENT_PRIMARY, BG_INPUT_SELECTION, INPUT_CARET, OVERLAY_DIM_MEDIUM, TEXT_HINT,
    TEXT_PRIMARY, TEXT_WHITE,
};
use crate::ui::primitives::{draw_rounded_rect, text_extents_for_with_engine};
use crate::ui::theme::{Rgba, popup};
use crate::ui_text::{UiTextEngine, UiTextStyle};

use super::palette::{PALETTE_SWATCH_GAP, PALETTE_SWATCH_SIZE};

const SLIDER_RAIL: Rgba = (1.0, 1.0, 1.0, 0.18);
const SLIDER_THUMB_EDGE: Rgba = (0.0, 0.0, 0.0, 0.35);
const FIELD_INVALID: Rgba = (0.90, 0.35, 0.30, 0.9);
/// Secondary button: a quiet fill and border beside the accent-filled Apply.
const BUTTON_SECONDARY_BG: Rgba = (0.25, 0.25, 0.30, 0.95);
const BUTTON_SECONDARY_BORDER: Rgba = (0.40, 0.40, 0.45, 0.8);

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
    let (Some(frame), Some(header)) = (
        input.board_appearance_frame(),
        input.board_appearance_header(),
    ) else {
        return;
    };
    let width = frame.width;
    let style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 12.0,
    };
    let _ = ctx.save();
    draw_backdrop(ctx, input, screen_width, screen_height);
    // The sheet is laid out in sheet units from its content origin and drawn
    // magnified, so text, controls, and strokes scale together.
    ctx.translate(frame.x, frame.y);
    ctx.scale(frame.scale, frame.scale);
    let outline = frame.outline();
    draw_frame(ctx, outline);

    let board_name = input
        .boards
        .board_states()
        .iter()
        .find(|board| board.spec.id == edit.board_id())
        .map_or("", |board| board.spec.name.as_str());
    let _ = ctx.save();
    ctx.rectangle(0.0, -66.0, header.title_right.max(0.0), 32.0);
    ctx.clip();
    constants::set_color(ctx, TEXT_PRIMARY);
    engine.draw_baseline(
        ctx,
        UiTextStyle {
            weight: cairo::FontWeight::Bold,
            ..style
        },
        &format!("Paper: {board_name}"),
        4.0,
        -45.0,
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
    ctx.move_to(outline.0, -33.5);
    ctx.line_to(outline.0 + outline.2, -33.5);
    let _ = ctx.stroke();

    for (index, color) in super::helpers::BOARD_PALETTE.iter().enumerate() {
        ctx.set_source_rgb(color.r, color.g, color.b);
        ctx.rectangle(
            index as f64 * width / 11.0 + 1.0,
            -28.0,
            width / 11.0 - 3.0,
            21.0,
        );
        let _ = ctx.fill();
    }
    for (index, kind) in BoardGridKind::ALL.iter().enumerate() {
        let left = (index % 2) as f64 * width / 2.0;
        let top = (index / 2) as f64 * 28.0;
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
    if let Some(row) = input.board_appearance_size_row() {
        draw_size_row(
            engine,
            ctx,
            SizeRowPaint {
                label_x: 4.0,
                label_right: row.label_right,
                label: match edit.kind {
                    BoardGridKind::None => "Spacing",
                    BoardGridKind::Cartesian => "Square side",
                    BoardGridKind::Isometric | BoardGridKind::IsometricDots => "Triangle side",
                },
                slider: (row.track.2 > 0.0).then_some((row.rail, row.thumb_x, row.thumb_radius)),
                field: row.field,
                text: &edit.spacing,
                focused: edit.focus == AppearanceField::Spacing,
                armed: edit.spacing_armed,
                dragging: edit.size_dragging,
                valid: edit.size_is_valid(),
            },
            style,
        );
    }
    let _ = ctx.save();
    ctx.rectangle(0.0, 94.0, width, 56.0);
    ctx.clip();
    ctx.translate(0.0, 94.0);
    // The pattern stays at board size so the preview shows the real spacing.
    ctx.scale(1.0 / frame.scale, 1.0 / frame.scale);
    let (color, grid) = edit.preview();
    if let Ok(paper) = crate::draw::BoardPaper::for_context(color, grid, ctx) {
        let _ = paper.paint(ctx);
    }
    let _ = ctx.restore();
    if let Some(buttons) = input.board_appearance_buttons() {
        draw_button(
            engine,
            ctx,
            buttons.cancel,
            "Cancel",
            ButtonStyle::Secondary,
            style,
        );
        let apply_style = if edit.validation_error().is_none() {
            ButtonStyle::Primary
        } else {
            ButtonStyle::Disabled
        };
        draw_button(engine, ctx, buttons.apply, "Apply", apply_style, style);
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
        0.0,
        202.0,
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

fn draw_frame(ctx: &cairo::Context, (left, top, frame_width, frame_height): (f64, f64, f64, f64)) {
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

struct SizeRowPaint<'a> {
    label_x: f64,
    label_right: f64,
    label: &'a str,
    /// Rail `(start_x, end_x, center_y)`, thumb x, and thumb radius.
    slider: Option<((f64, f64, f64), f64, f64)>,
    field: (f64, f64, f64, f64),
    text: &'a str,
    focused: bool,
    /// Typing replaces the size, shown as a selection.
    armed: bool,
    dragging: bool,
    valid: bool,
}

/// Label, logarithmic size slider, and editable pixel field.
fn draw_size_row(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    row: SizeRowPaint<'_>,
    style: UiTextStyle<'_>,
) {
    let (field_x, field_y, field_width, field_height) = row.field;
    let baseline = field_y + field_height / 2.0 + 4.5;

    let _ = ctx.save();
    ctx.rectangle(
        row.label_x,
        field_y - 4.0,
        (row.label_right - row.label_x).max(0.0),
        field_height + 8.0,
    );
    ctx.clip();
    constants::set_color(
        ctx,
        if row.focused {
            INPUT_CARET
        } else {
            TEXT_PRIMARY
        },
    );
    engine.draw_baseline(ctx, style, row.label, row.label_x, baseline, None);
    let _ = ctx.restore();

    if let Some(((start, end, center_y), thumb_x, radius)) = row.slider {
        draw_rounded_rect(ctx, start, center_y - 2.0, end - start, 4.0, 2.0);
        constants::set_color(ctx, SLIDER_RAIL);
        let _ = ctx.fill();
        if thumb_x > start {
            draw_rounded_rect(ctx, start, center_y - 2.0, thumb_x - start, 4.0, 2.0);
            constants::set_color(ctx, ACCENT_PRIMARY);
            let _ = ctx.fill();
        }

        ctx.new_sub_path();
        ctx.arc(thumb_x, center_y, radius, 0.0, std::f64::consts::TAU);
        constants::set_color(ctx, TEXT_PRIMARY);
        let _ = ctx.fill_preserve();
        constants::set_color(
            ctx,
            if row.focused || row.dragging {
                INPUT_CARET
            } else {
                SLIDER_THUMB_EDGE
            },
        );
        ctx.set_line_width(if row.dragging { 2.5 } else { 1.5 });
        let _ = ctx.stroke();
    }

    draw_rounded_rect(ctx, field_x, field_y, field_width, field_height, 4.0);
    constants::set_color(ctx, FIELD_BG);
    let _ = ctx.fill_preserve();
    let (border, border_width) = if !row.valid {
        (FIELD_INVALID, 1.5)
    } else if row.focused {
        (INPUT_CARET, 1.5)
    } else {
        (FIELD_BORDER, 1.0)
    };
    constants::set_color(ctx, border);
    ctx.set_line_width(border_width);
    let _ = ctx.stroke();

    let _ = ctx.save();
    ctx.rectangle(field_x + 2.0, field_y, field_width - 4.0, field_height);
    ctx.clip();
    let text_x = field_x + 8.0;
    let advance = text_extents_for_with_engine(
        engine,
        ctx,
        style.family,
        style.slant,
        style.weight,
        style.size,
        row.text,
    )
    .x_advance();
    if row.focused && row.armed && !row.text.is_empty() {
        constants::set_color(ctx, BG_INPUT_SELECTION);
        ctx.rectangle(
            text_x - 2.0,
            field_y + 5.0,
            advance + 4.0,
            field_height - 10.0,
        );
        let _ = ctx.fill();
    }
    constants::set_color(ctx, TEXT_PRIMARY);
    engine.draw_baseline(ctx, style, row.text, text_x, baseline, None);
    constants::set_color(ctx, TEXT_HINT);
    engine.draw_baseline(
        ctx,
        UiTextStyle {
            size: 11.0,
            ..style
        },
        "px",
        text_x + advance + 4.0,
        baseline,
        None,
    );
    let _ = ctx.restore();
}

#[derive(Clone, Copy)]
enum ButtonStyle {
    Primary,
    Disabled,
    Secondary,
}

fn draw_button(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    (x, y, width, height): (f64, f64, f64, f64),
    label: &str,
    button: ButtonStyle,
    style: UiTextStyle<'_>,
) {
    draw_rounded_rect(ctx, x, y, width, height, 6.0);
    let text = match button {
        ButtonStyle::Primary => {
            constants::set_color(ctx, ACCENT_PRIMARY);
            let _ = ctx.fill();
            TEXT_WHITE
        }
        ButtonStyle::Disabled => {
            constants::set_color(ctx, constants::with_alpha(ACCENT_PRIMARY, 0.3));
            let _ = ctx.fill();
            constants::with_alpha(TEXT_WHITE, 0.5)
        }
        ButtonStyle::Secondary => {
            constants::set_color(ctx, BUTTON_SECONDARY_BG);
            let _ = ctx.fill_preserve();
            constants::set_color(ctx, BUTTON_SECONDARY_BORDER);
            ctx.set_line_width(1.0);
            let _ = ctx.stroke();
            TEXT_PRIMARY
        }
    };

    let bold = UiTextStyle {
        weight: cairo::FontWeight::Bold,
        ..style
    };
    let extents = text_extents_for_with_engine(
        engine,
        ctx,
        bold.family,
        bold.slant,
        bold.weight,
        bold.size,
        label,
    );
    constants::set_color(ctx, text);
    engine.draw_baseline(
        ctx,
        bold,
        label,
        x + (width - extents.width()) / 2.0,
        y + height / 2.0 + 4.5,
        None,
    );
}
