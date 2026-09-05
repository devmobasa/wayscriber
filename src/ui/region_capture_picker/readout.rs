use super::layout::{PANEL_PADDING_X, pointer_panel_layout, selection_badge_layout};
use super::{PANEL_FILL, PANEL_RADIUS};
use crate::ui::primitives::{draw_rounded_rect, text_extents_for_with_engine};
use crate::ui::region_action_bar::RegionActionRect;
use crate::ui_text::UiTextEngine;

const PANEL_BORDER: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.16);
pub(super) const READOUT_FONT_SIZE: f64 = 12.0;

/// The measurement chip. `selection`, when present, anchors it to that
/// rectangle; otherwise it trails the pointer.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_readout_panel(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    text: &str,
    font_size: f64,
    pointer: (f64, f64),
    selection: Option<(f64, f64, f64, f64)>,
    action_bar: Option<RegionActionRect>,
    screen: (u32, u32),
    weight: cairo::FontWeight,
) {
    let extents = text_extents_for_with_engine(
        engine,
        ctx,
        "monospace",
        cairo::FontSlant::Normal,
        weight,
        font_size,
        text,
    );
    let layout = match selection {
        Some(rect) => selection_badge_layout(rect, extents.width(), action_bar, screen),
        None => pointer_panel_layout(pointer, extents.width(), screen),
    };
    if layout.width <= 0.0 || layout.height <= 0.0 {
        return;
    }
    ctx.set_source_rgba(PANEL_FILL.0, PANEL_FILL.1, PANEL_FILL.2, PANEL_FILL.3);
    let radius = PANEL_RADIUS
        .min(layout.width / 2.0)
        .min(layout.height / 2.0);
    draw_rounded_rect(ctx, layout.x, layout.y, layout.width, layout.height, radius);
    let _ = ctx.fill();
    ctx.set_source_rgba(
        PANEL_BORDER.0,
        PANEL_BORDER.1,
        PANEL_BORDER.2,
        PANEL_BORDER.3,
    );
    ctx.set_line_width(1.0);
    draw_rounded_rect(
        ctx,
        layout.x + 0.5,
        layout.y + 0.5,
        layout.width - 1.0,
        layout.height - 1.0,
        (radius - 0.5).max(0.0),
    );
    let _ = ctx.stroke();

    ctx.set_source_rgb(1.0, 1.0, 1.0);
    ctx.select_font_face(
        "monospace",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    ctx.set_font_size(font_size);
    let baseline = layout.y + (layout.height - extents.height()) / 2.0 - extents.y_bearing();
    let _ = ctx.save();
    ctx.rectangle(layout.x, layout.y, layout.width, layout.height);
    ctx.clip();
    ctx.move_to(layout.x + PANEL_PADDING_X - extents.x_bearing(), baseline);
    let _ = ctx.show_text(text);
    let _ = ctx.restore();
}
