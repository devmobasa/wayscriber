//! Tour overlay rendering.

use crate::input::state::{InputState, TourStep};
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::constants::{
    self, BORDER_MODAL, OVERLAY_DIM_HEAVY, PANEL_BG_MODAL, PROGRESS_FILL, PROGRESS_TRACK,
    RADIUS_PANEL, SPACING_PANEL, TEXT_DESCRIPTION, TEXT_HINT, TEXT_WHITE,
};

/// Render the guided tour overlay.
pub fn render_tour(ctx: &cairo::Context, input_state: &InputState, width: u32, height: u32) {
    let Some(step) = input_state.current_tour_step() else {
        return;
    };

    let width = width as f64;
    let height = height as f64;

    // Semi-transparent backdrop
    ctx.set_source_rgba(0.0, 0.0, 0.0, OVERLAY_DIM_HEAVY);
    ctx.rectangle(0.0, 0.0, width, height);
    let _ = ctx.fill();

    // Dialog dimensions
    let dialog_width = 560.0;
    let dialog_padding = SPACING_PANEL;
    let dialog_x = (width - dialog_width) / 2.0;
    let dialog_y = height * 0.3;

    // Calculate dialog height based on content
    let title_height = 36.0;
    let desc_line_height = 24.0;
    let desc_lines = step.description().lines().count() as f64;
    let nav_height = 24.0;
    let progress_height = 20.0;
    let dialog_height = dialog_padding * 2.0
        + title_height
        + desc_lines * desc_line_height
        + nav_height
        + progress_height
        + 40.0;

    // Dialog background
    rounded_rect(
        ctx,
        dialog_x,
        dialog_y,
        dialog_width,
        dialog_height,
        RADIUS_PANEL,
    );
    constants::set_color(ctx, PANEL_BG_MODAL);
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, BORDER_MODAL);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    let content_x = dialog_x + dialog_padding;
    let mut y = dialog_y + dialog_padding;
    let step_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 14.0,
    };
    let title_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: 24.0,
    };
    let desc_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 16.0,
    };
    let nav_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 13.0,
    };

    // Step counter
    constants::set_color(ctx, TEXT_HINT);
    let step_text = format!("Step {} of {}", input_state.tour_step + 1, TourStep::COUNT);
    draw_text_baseline(ctx, step_style, &step_text, content_x, y + 12.0, None);
    y += 24.0;

    // Title
    constants::set_color(ctx, TEXT_WHITE);
    draw_text_baseline(ctx, title_style, step.title(), content_x, y + 24.0, None);
    y += title_height + 16.0;

    // Description
    constants::set_color(ctx, TEXT_DESCRIPTION);
    for line in step.description().lines() {
        draw_text_baseline(ctx, desc_style, line, content_x, y + 18.0, None);
        y += desc_line_height;
    }
    y += 24.0;

    // Progress bar
    let progress_width = dialog_width - dialog_padding * 2.0;
    let progress_y = y;
    constants::set_color(ctx, PROGRESS_TRACK);
    rounded_rect(ctx, content_x, progress_y, progress_width, 6.0, 3.0);
    let _ = ctx.fill();

    let filled_width =
        progress_width * ((input_state.tour_step + 1) as f64 / TourStep::COUNT as f64);
    constants::set_color(ctx, PROGRESS_FILL);
    rounded_rect(ctx, content_x, progress_y, filled_width, 6.0, 3.0);
    let _ = ctx.fill();
    y += progress_height + 16.0;

    // Navigation hint
    ctx.set_source_rgba(TEXT_HINT.0, TEXT_HINT.1, TEXT_HINT.2, 0.8);
    draw_text_baseline(ctx, nav_style, step.nav_hint(), content_x, y + 13.0, None);
}

fn rounded_rect(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    ctx.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    ctx.arc(
        x + r,
        y + h - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    ctx.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    ctx.close_path();
}
