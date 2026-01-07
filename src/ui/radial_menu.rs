use crate::config::ToolPresetConfig;
use crate::input::InputState;
use crate::input::state::{RadialMenuLayout, RadialMenuState};
use crate::input::tool::Tool;
use crate::toolbar_icons::{
    draw_icon_arrow, draw_icon_circle, draw_icon_eraser, draw_icon_highlight, draw_icon_line,
    draw_icon_marker, draw_icon_pen, draw_icon_rect, draw_icon_select,
};
use std::f64::consts::PI;

use super::primitives::fallback_text_extents;

/// Renders the radial preset menu as a pie chart overlay.
pub fn render_radial_menu(
    ctx: &cairo::Context,
    input_state: &InputState,
    _screen_width: u32,
    _screen_height: u32,
) {
    let (hover_index, slot_count) = match &input_state.radial_menu_state {
        RadialMenuState::Open {
            hover_index,
            slot_count,
            ..
        } => (*hover_index, *slot_count),
        RadialMenuState::Hidden => return,
    };

    let layout = match input_state.radial_menu_layout() {
        Some(layout) => *layout,
        None => return,
    };

    let _ = ctx.save();

    // Draw each segment
    let segment_angle = 2.0 * PI / slot_count as f64;
    let gap_angle = 0.03; // Small gap between segments

    for i in 0..slot_count {
        let start = layout.start_angle + (i as f64 * segment_angle) + gap_angle / 2.0;
        let end = start + segment_angle - gap_angle;
        let is_hovered = hover_index == Some(i);
        let preset = input_state.presets.get(i).and_then(|p| p.as_ref());

        draw_segment(ctx, &layout, start, end, is_hovered, preset, i + 1);
    }

    // Draw center cancel zone (hovered if no segment is selected)
    let center_hovered = hover_index.is_none();
    draw_center_zone(ctx, &layout, center_hovered);

    let _ = ctx.restore();
}

fn draw_segment(
    ctx: &cairo::Context,
    layout: &RadialMenuLayout,
    start_angle: f64,
    end_angle: f64,
    is_hovered: bool,
    preset: Option<&ToolPresetConfig>,
    slot_number: usize,
) {
    // Draw arc segment (pie slice)
    ctx.new_path();
    ctx.arc(
        layout.center_x,
        layout.center_y,
        layout.outer_radius,
        start_angle,
        end_angle,
    );
    ctx.arc_negative(
        layout.center_x,
        layout.center_y,
        layout.inner_radius,
        end_angle,
        start_angle,
    );
    ctx.close_path();

    // Fill color (highlight if hovered)
    if is_hovered {
        ctx.set_source_rgba(0.25, 0.32, 0.45, 0.95);
    } else {
        ctx.set_source_rgba(0.1, 0.13, 0.17, 0.92);
    }
    let _ = ctx.fill_preserve();

    // Border
    ctx.set_source_rgba(0.3, 0.35, 0.42, 0.8);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    // Calculate label position (midpoint of arc)
    let mid_angle = (start_angle + end_angle) / 2.0;
    let label_radius = (layout.inner_radius + layout.outer_radius) / 2.0;
    let label_x = layout.center_x + label_radius * mid_angle.cos();
    let label_y = layout.center_y + label_radius * mid_angle.sin();

    if let Some(preset) = preset {
        // Draw color swatch
        let color = preset.color.to_color();
        ctx.arc(label_x, label_y, 12.0, 0.0, 2.0 * PI);
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        let _ = ctx.fill();

        // Draw border around swatch
        ctx.arc(label_x, label_y, 12.0, 0.0, 2.0 * PI);
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.3);
        ctx.set_line_width(1.5);
        let _ = ctx.stroke();

        // Draw tool icon in the swatch
        // Choose contrasting icon color
        let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
        if luminance > 0.5 {
            ctx.set_source_rgba(0.0, 0.0, 0.0, 0.9);
        } else {
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        }

        let icon_size = 14.0;
        let icon_x = label_x - icon_size / 2.0;
        let icon_y = label_y - icon_size / 2.0;
        match preset.tool {
            Tool::Pen => draw_icon_pen(ctx, icon_x, icon_y, icon_size),
            Tool::Marker => draw_icon_marker(ctx, icon_x, icon_y, icon_size),
            Tool::Line => draw_icon_line(ctx, icon_x, icon_y, icon_size),
            Tool::Arrow => draw_icon_arrow(ctx, icon_x, icon_y, icon_size),
            Tool::Rect => draw_icon_rect(ctx, icon_x, icon_y, icon_size),
            Tool::Ellipse => draw_icon_circle(ctx, icon_x, icon_y, icon_size),
            Tool::Eraser => draw_icon_eraser(ctx, icon_x, icon_y, icon_size),
            Tool::Highlight => draw_icon_highlight(ctx, icon_x, icon_y, icon_size),
            Tool::Select => draw_icon_select(ctx, icon_x, icon_y, icon_size),
        }

        // Draw size indicator (small arc below swatch)
        let size_radius = 16.0;
        let size_indicator = (preset.size / 50.0).clamp(0.1, 1.0) * PI * 0.4;
        ctx.new_path();
        ctx.arc(
            label_x,
            label_y,
            size_radius,
            PI / 2.0 - size_indicator / 2.0,
            PI / 2.0 + size_indicator / 2.0,
        );
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.5);
        ctx.set_line_width(2.0);
        let _ = ctx.stroke();
    } else {
        // Empty slot - draw slot number
        ctx.set_source_rgba(0.5, 0.5, 0.5, 0.6);
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        let font_size = 16.0;
        ctx.set_font_size(font_size);
        let text = format!("{}", slot_number);
        let extents = ctx
            .text_extents(&text)
            .unwrap_or_else(|_| fallback_text_extents(font_size, &text));
        ctx.move_to(
            label_x - extents.width() / 2.0,
            label_y + extents.height() / 2.0,
        );
        let _ = ctx.show_text(&text);

        // Draw empty circle
        ctx.new_path();
        ctx.arc(label_x, label_y - 0.0, 10.0, 0.0, 2.0 * PI);
        ctx.set_source_rgba(0.4, 0.4, 0.4, 0.3);
        ctx.set_line_width(1.5);
        let _ = ctx.stroke();
    }
}

fn draw_center_zone(ctx: &cairo::Context, layout: &RadialMenuLayout, is_center_hovered: bool) {
    ctx.new_path();
    ctx.arc(
        layout.center_x,
        layout.center_y,
        layout.inner_radius - 2.0,
        0.0,
        2.0 * PI,
    );

    if is_center_hovered {
        // Reddish tint for cancel indication
        ctx.set_source_rgba(0.35, 0.15, 0.15, 0.9);
    } else {
        ctx.set_source_rgba(0.08, 0.1, 0.14, 0.9);
    }
    let _ = ctx.fill();

    // Draw X in center
    ctx.set_source_rgba(0.6, 0.6, 0.6, if is_center_hovered { 0.9 } else { 0.4 });
    ctx.set_line_width(2.0);
    let s = 8.0;
    ctx.move_to(layout.center_x - s, layout.center_y - s);
    ctx.line_to(layout.center_x + s, layout.center_y + s);
    ctx.move_to(layout.center_x + s, layout.center_y - s);
    ctx.line_to(layout.center_x - s, layout.center_y + s);
    let _ = ctx.stroke();
}
