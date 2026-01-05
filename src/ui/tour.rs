//! Tour overlay rendering.

use crate::input::state::{InputState, TourStep};

/// Render the guided tour overlay.
pub fn render_tour(ctx: &cairo::Context, input_state: &InputState, width: u32, height: u32) {
    let Some(step) = input_state.current_tour_step() else {
        return;
    };

    let width = width as f64;
    let height = height as f64;

    // Semi-transparent backdrop
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.7);
    ctx.rectangle(0.0, 0.0, width, height);
    let _ = ctx.fill();

    // Dialog dimensions
    let dialog_width = 560.0;
    let dialog_padding = 48.0;
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
    let corner_radius = 12.0;
    rounded_rect(
        ctx,
        dialog_x,
        dialog_y,
        dialog_width,
        dialog_height,
        corner_radius,
    );
    ctx.set_source_rgba(0.15, 0.15, 0.18, 0.98);
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(0.4, 0.4, 0.5, 0.6);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    let content_x = dialog_x + dialog_padding;
    let mut y = dialog_y + dialog_padding;

    // Step counter
    ctx.set_source_rgba(0.6, 0.6, 0.7, 1.0);
    ctx.select_font_face(
        "sans-serif",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    ctx.set_font_size(14.0);
    let step_text = format!("Step {} of {}", input_state.tour_step + 1, TourStep::COUNT);
    ctx.move_to(content_x, y + 12.0);
    let _ = ctx.show_text(&step_text);
    y += 24.0;

    // Title
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    ctx.select_font_face(
        "sans-serif",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    ctx.set_font_size(24.0);
    ctx.move_to(content_x, y + 24.0);
    let _ = ctx.show_text(step.title());
    y += title_height + 16.0;

    // Description
    ctx.set_source_rgba(0.85, 0.85, 0.9, 1.0);
    ctx.select_font_face(
        "sans-serif",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    ctx.set_font_size(16.0);
    for line in step.description().lines() {
        ctx.move_to(content_x, y + 18.0);
        let _ = ctx.show_text(line);
        y += desc_line_height;
    }
    y += 24.0;

    // Progress bar
    let progress_width = dialog_width - dialog_padding * 2.0;
    let progress_y = y;
    ctx.set_source_rgba(0.3, 0.3, 0.35, 1.0);
    rounded_rect(ctx, content_x, progress_y, progress_width, 6.0, 3.0);
    let _ = ctx.fill();

    let filled_width =
        progress_width * ((input_state.tour_step + 1) as f64 / TourStep::COUNT as f64);
    ctx.set_source_rgba(0.4, 0.6, 1.0, 1.0);
    rounded_rect(ctx, content_x, progress_y, filled_width, 6.0, 3.0);
    let _ = ctx.fill();
    y += progress_height + 16.0;

    // Navigation hint
    ctx.set_source_rgba(0.5, 0.5, 0.6, 1.0);
    ctx.set_font_size(13.0);
    ctx.move_to(content_x, y + 13.0);
    let _ = ctx.show_text(step.nav_hint());
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
