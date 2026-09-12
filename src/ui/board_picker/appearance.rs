use crate::domain::BoardGridKind;
use crate::input::InputState;
use crate::input::state::AppearanceField;
use crate::ui::constants::{self, INPUT_CARET, TEXT_HINT, TEXT_PRIMARY};
use crate::ui_text::{UiTextEngine, UiTextStyle};

pub(super) fn render(engine: &UiTextEngine, ctx: &cairo::Context, input: &InputState) {
    let Some(edit) = input.board_appearance_edit() else {
        return;
    };
    let Some((x, y, width)) = input.board_appearance_rect() else {
        return;
    };
    let style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 12.0,
    };
    let _ = ctx.save();
    constants::set_color(
        ctx,
        constants::with_alpha(constants::PANEL_BG_CONTEXT_MENU, 1.0),
    );
    crate::ui::primitives::draw_rounded_rect(ctx, x - 12.0, y - 70.0, width + 24.0, 292.0, 8.0);
    let _ = ctx.fill();
    constants::set_color(
        ctx,
        if edit.focus == AppearanceField::Color {
            INPUT_CARET
        } else {
            TEXT_PRIMARY
        },
    );
    engine.draw_baseline(
        ctx,
        style,
        &format!("Session paper color: {}", edit.color),
        x + 4.0,
        y - 44.0,
        None,
    );
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
    for (offset, label) in [(80.0, "20"), (40.0, "40")] {
        ctx.rectangle(x + width - offset, y + 60.0, 36.0, 25.0);
        let _ = ctx.stroke();
        engine.draw_baseline(ctx, style, label, x + width - offset + 9.0, y + 78.0, None);
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
        .unwrap_or("Tab: field • arrows: pattern • Enter: Apply");
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
