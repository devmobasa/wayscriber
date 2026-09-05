use super::controls::{draw_action, draw_toggle};
use super::layout::{RegionActionBar, RegionActionRect, STATUS_ROW_HEIGHT};
use super::model::{RegionAction, RegionActionBarVisual, RegionCutStatus};
use super::{TOGGLE_FONT_SIZE, status_label_style};
use crate::ui::primitives::draw_rounded_rect;
use crate::ui::theme::{self, Rgba, overlay};
use crate::ui_text::UiTextEngine;

const BAR_RADIUS: f64 = overlay::RADIUS_PANEL;
/// Downward-only two-layer drop shadow, matching the command palette frame, so
/// the bar reads as floating above the frozen screenshot rather than painted
/// into it.
const SHADOW_OFFSET: f64 = 8.0;
const SHADOW_SOFT: Rgba = (0.0, 0.0, 0.0, 0.20);

pub(crate) fn render_region_action_bar(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    bar: &RegionActionBar,
    visual: RegionActionBarVisual,
) {
    let _ = ctx.save();
    draw_bar_frame(ctx, bar.bounds);

    for &item in &bar.items {
        draw_action(
            engine,
            ctx,
            item,
            visual.hovered == Some(item.action),
            visual.availability.allows(item.action),
            false,
        );
    }
    draw_row_divider(
        ctx,
        bar.items[0].bounds,
        bar.edit[0].bounds.y,
        bar.toggle.bounds.width,
    );
    for &item in &bar.edit {
        draw_action(
            engine,
            ctx,
            item,
            visual.hovered == Some(item.action),
            visual.availability.allows(item.action),
            visual.cut_armed && item.action == RegionAction::CutBand,
        );
    }
    draw_row_divider(
        ctx,
        bar.edit[0].bounds,
        bar.toggle.bounds.y,
        bar.toggle.bounds.width,
    );
    draw_toggle(
        engine,
        ctx,
        bar.toggle,
        visual.hovered,
        visual.include_drawings,
    );
    draw_status(engine, ctx, bar, visual.status);
    let _ = ctx.restore();
}

fn draw_bar_frame(ctx: &cairo::Context, bounds: RegionActionRect) {
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }
    for (offset, color) in [
        (SHADOW_OFFSET, SHADOW_SOFT),
        (SHADOW_OFFSET * 0.5, overlay::SHADOW),
    ] {
        theme::set_color(ctx, color);
        draw_rounded_rect(
            ctx,
            bounds.x,
            bounds.y + offset,
            bounds.width,
            bounds.height,
            BAR_RADIUS,
        );
        let _ = ctx.fill();
    }

    theme::set_color(ctx, crate::ui::theme::popup::bg_context_menu());
    draw_rounded_rect(
        ctx,
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        BAR_RADIUS,
    );
    let _ = ctx.fill();

    theme::set_color(ctx, crate::ui::theme::popup::border_context_menu());
    ctx.set_line_width(1.0);
    draw_rounded_rect(
        ctx,
        bounds.x + 0.5,
        bounds.y + 0.5,
        bounds.width - 1.0,
        bounds.height - 1.0,
        BAR_RADIUS - 0.5,
    );
    let _ = ctx.stroke();
}

/// Hairline between stacked rows, inset from the bar's padding so it reads as
/// a grouping rule rather than a border.
fn draw_row_divider(ctx: &cairo::Context, row: RegionActionRect, next_y: f64, width: f64) {
    if width <= 0.0 || row.height <= 0.0 {
        return;
    }
    let gap = (next_y - row.y - row.height).max(0.0);
    if gap <= 0.0 {
        return;
    }
    let y = (row.y + row.height + gap / 2.0).floor() + 0.5;
    theme::set_color(ctx, overlay::DIVIDER_LIGHT);
    ctx.set_line_width(1.0);
    ctx.move_to(row.x, y);
    ctx.line_to(row.x + width, y);
    let _ = ctx.stroke();
}

fn draw_status(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    bar: &RegionActionBar,
    status: Option<RegionCutStatus>,
) {
    let Some(status) = status else {
        return;
    };
    let Some(row) = bar.status_bounds() else {
        return;
    };
    let font_size = (TOGGLE_FONT_SIZE * (row.height / STATUS_ROW_HEIGHT).min(1.0)).max(0.0);
    if font_size < 1.0 {
        return;
    }
    let _ = ctx.save();
    ctx.rectangle(row.x, row.y, row.width, row.height);
    ctx.clip();
    let layout = engine.layout(ctx, status_label_style(font_size), status.message(), None);
    let extents = layout.ink_extents();
    theme::set_color(
        ctx,
        match status {
            RegionCutStatus::Updating => overlay::TEXT_HINT,
            RegionCutStatus::Failed => overlay::TEXT_PRIMARY,
        },
    );
    layout.show_at_baseline(
        ctx,
        row.x + (row.width - extents.width()) / 2.0 - extents.x_bearing(),
        row.y + (row.height - extents.height()) / 2.0 - extents.y_bearing(),
    );
    let _ = ctx.restore();
}
