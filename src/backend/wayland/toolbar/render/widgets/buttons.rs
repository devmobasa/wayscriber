use super::constants::{
    COLOR_ACCENT_BRIGHT, COLOR_ACCENT_GLOW, COLOR_BUTTON_ACTIVE, COLOR_BUTTON_DEFAULT,
    COLOR_BUTTON_DESTRUCTIVE, COLOR_BUTTON_DESTRUCTIVE_HOVER, COLOR_BUTTON_DISABLED,
    COLOR_BUTTON_HOVER, COLOR_CLOSE_DEFAULT, COLOR_CLOSE_HOVER, COLOR_FOCUS_RING, COLOR_PIN_ACTIVE,
    COLOR_PIN_DEFAULT, COLOR_PIN_HOVER, COLOR_SEGMENT_ACTIVE, COLOR_SEGMENT_BG,
    COLOR_SEGMENT_DIVIDER, COLOR_SEGMENT_HOVER, COLOR_SEGMENT_TEXT_ACTIVE,
    COLOR_SEGMENT_TEXT_INACTIVE, COLOR_TEXT_PRIMARY, LINE_WIDTH_THICK, RADIUS_LG, RADIUS_STD,
    SPACING_XS, set_color,
};
use super::draw_round_rect;
use crate::ui_text::{UiTextStyle, text_layout};
use std::f64::consts::PI;

pub(in crate::backend::wayland::toolbar::render) fn draw_drag_handle(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    hover: bool,
) {
    draw_round_rect(ctx, x, y, w, h, RADIUS_STD);
    // Improved visibility: higher fill alpha
    let fill_alpha = if hover { 0.75 } else { 0.45 };
    ctx.set_source_rgba(1.0, 1.0, 1.0, fill_alpha);
    let _ = ctx.fill();

    // Add subtle glow on hover
    if hover {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.15);
        draw_round_rect(ctx, x - 1.0, y - 1.0, w + 2.0, h + 2.0, RADIUS_STD + 1.0);
        let _ = ctx.stroke();
    }

    ctx.set_line_width(1.1);
    let bar_alpha = if hover { 1.0 } else { 0.85 };
    ctx.set_source_rgba(1.0, 1.0, 1.0, bar_alpha);
    let bar_w = w * 0.55;
    let bar_h = SPACING_XS;
    let bar_gap = SPACING_XS;
    let bar_x = x + (w - bar_w) / 2.0;
    let mut bar_y = drag_handle_bar_start_y(y, h, bar_h, bar_gap);
    for _ in 0..3 {
        draw_round_rect(ctx, bar_x, bar_y, bar_w, bar_h, 1.0);
        let _ = ctx.fill();
        bar_y += bar_h + bar_gap;
    }
}

fn drag_handle_bar_start_y(y: f64, h: f64, bar_h: f64, bar_gap: f64) -> f64 {
    let stack_h = 3.0 * bar_h + 2.0 * bar_gap;
    y + (h - stack_h) / 2.0
}

/// Minimize chrome button: a dash, not an X — the bar collapses to an
/// edge restore tab instead of disappearing.
pub(in crate::backend::wayland::toolbar::render) fn draw_minimize_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
) {
    let r = size / 2.0;
    let cx = x + r;
    let cy = y + r;

    set_color(
        ctx,
        if hover {
            COLOR_CLOSE_HOVER
        } else {
            COLOR_CLOSE_DEFAULT
        },
    );
    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.fill();

    set_color(ctx, COLOR_TEXT_PRIMARY);
    ctx.set_line_width(LINE_WIDTH_THICK);
    let inset = size * 0.28;
    ctx.move_to(x + inset, cy);
    ctx.line_to(x + size - inset, cy);
    let _ = ctx.stroke();
}

pub(in crate::backend::wayland::toolbar::render) fn draw_pin_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    pinned: bool,
    hover: bool,
) {
    let r = size / 2.0;
    let cx = x + r;
    let cy = y + r;

    // Draw outer glow when pinned for visual feedback
    if pinned {
        set_color(ctx, COLOR_ACCENT_GLOW);
        ctx.arc(cx, cy, r + 2.0, 0.0, PI * 2.0);
        let _ = ctx.fill();
    }

    // Use circle shape for visual consistency with close button
    let color = if pinned {
        COLOR_PIN_ACTIVE
    } else if hover {
        COLOR_PIN_HOVER
    } else {
        COLOR_PIN_DEFAULT
    };
    set_color(ctx, color);
    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.fill();

    let pin_size = size * 0.5;
    draw_pushpin(ctx, cx, cy, pin_size, pinned);
}

/// Draw a pushpin glyph centered at (cx, cy). The control means
/// "keep this toolbar open at startup", so the glyph is a thumbtack —
/// filled when pinned, outline when not.
fn draw_pushpin(ctx: &cairo::Context, cx: f64, cy: f64, size: f64, filled: bool) {
    let s = size;

    ctx.new_path();
    // Head: flat cap at the top
    ctx.move_to(cx - s * 0.45, cy - s * 0.85);
    ctx.line_to(cx + s * 0.45, cy - s * 0.85);
    ctx.line_to(cx + s * 0.3, cy - s * 0.55);
    // Neck down to the flange
    ctx.line_to(cx + s * 0.3, cy - s * 0.15);
    // Flange: wider base plate
    ctx.line_to(cx + s * 0.6, cy + s * 0.15);
    ctx.line_to(cx - s * 0.6, cy + s * 0.15);
    ctx.line_to(cx - s * 0.3, cy - s * 0.15);
    ctx.line_to(cx - s * 0.3, cy - s * 0.55);
    ctx.close_path();

    set_color(ctx, COLOR_TEXT_PRIMARY);
    if filled {
        let _ = ctx.fill();
    } else {
        ctx.set_line_width(1.3);
        let _ = ctx.stroke();
    }

    // Needle below the flange
    ctx.set_line_width(if filled { 1.6 } else { 1.3 });
    ctx.move_to(cx, cy + s * 0.15);
    ctx.line_to(cx, cy + s * 0.85);
    let _ = ctx.stroke();
}

pub(in crate::backend::wayland::toolbar::render) fn draw_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    active: bool,
    hover: bool,
) {
    // Add subtle glow on hover for better visibility
    if hover && !active {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.08);
        draw_round_rect(ctx, x - 1.0, y - 1.0, w + 2.0, h + 2.0, RADIUS_LG + 1.0);
        let _ = ctx.fill();
    }

    // Active state: add outer glow ring
    if active {
        set_color(ctx, COLOR_ACCENT_GLOW);
        draw_round_rect(ctx, x - 2.0, y - 2.0, w + 4.0, h + 4.0, RADIUS_LG + 2.0);
        let _ = ctx.fill();
    }

    let color = if active {
        COLOR_BUTTON_ACTIVE
    } else if hover {
        COLOR_BUTTON_HOVER
    } else {
        COLOR_BUTTON_DEFAULT
    };
    set_color(ctx, color);
    draw_round_rect(ctx, x, y, w, h, RADIUS_LG);
    let _ = ctx.fill();

    // Active state: add bottom indicator line
    if active {
        set_color(ctx, COLOR_ACCENT_BRIGHT);
        let indicator_w = w * 0.5;
        let indicator_h = 2.5;
        let indicator_x = x + (w - indicator_w) / 2.0;
        let indicator_y = y + h - indicator_h - 2.0;
        draw_round_rect(ctx, indicator_x, indicator_y, indicator_w, indicator_h, 1.5);
        let _ = ctx.fill();
    }
}

/// Draw a disabled button body: dimmed background, no hover or active
/// affordance, so the tile itself reads inert before the icon/label does.
pub(in crate::backend::wayland::toolbar::render) fn draw_disabled_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    set_color(ctx, COLOR_BUTTON_DISABLED);
    draw_round_rect(ctx, x, y, w, h, RADIUS_LG);
    let _ = ctx.fill();
}

/// Draw a keyboard focus ring around an element.
/// Call this after drawing the element to show focus indication.
#[allow(dead_code)]
pub(in crate::backend::wayland::toolbar::render) fn draw_focus_ring(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    radius: f64,
) {
    set_color(ctx, COLOR_FOCUS_RING);
    ctx.set_line_width(2.0);
    draw_round_rect(ctx, x - 2.0, y - 2.0, w + 4.0, h + 4.0, radius + 2.0);
    let _ = ctx.stroke();
}

/// Draw a button for destructive actions (e.g., Clear, board/page Delete):
/// red-tinted body plus a red accent line so it never reads like navigation.
pub(in crate::backend::wayland::toolbar::render) fn draw_destructive_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    hover: bool,
) {
    // Add warning-tinted glow on hover
    if hover {
        ctx.set_source_rgba(0.9, 0.4, 0.3, 0.15);
        draw_round_rect(ctx, x - 1.0, y - 1.0, w + 2.0, h + 2.0, RADIUS_LG + 1.0);
        let _ = ctx.fill();
    }

    let color = if hover {
        COLOR_BUTTON_DESTRUCTIVE_HOVER
    } else {
        COLOR_BUTTON_DESTRUCTIVE
    };
    set_color(ctx, color);
    draw_round_rect(ctx, x, y, w, h, RADIUS_LG);
    let _ = ctx.fill();

    // Red accent line at top edge
    ctx.set_source_rgba(0.85, 0.35, 0.3, if hover { 0.9 } else { 0.7 });
    let accent_w = w * 0.6;
    let accent_h = 2.0;
    let accent_x = x + (w - accent_w) / 2.0;
    let accent_y = y + 2.0;
    draw_round_rect(ctx, accent_x, accent_y, accent_w, accent_h, 1.0);
    let _ = ctx.fill();
}

/// Draw a segmented control with two options.
/// Returns nothing but renders the control with proper active/hover states.
#[allow(clippy::too_many_arguments)]
pub(in crate::backend::wayland::toolbar::render) fn draw_segmented_control(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    labels: (&str, &str),
    active_segment: usize, // 0=left active, 1=right active
    hover: Option<usize>,  // Which segment is hovered
    label_style: UiTextStyle<'_>,
) {
    let segment_w = w / 2.0;
    let radius = 6.0;
    let inner_radius = 4.0;
    let padding = 2.0;

    // Draw outer container
    draw_round_rect(ctx, x, y, w, h, radius);
    set_color(ctx, COLOR_SEGMENT_BG);
    let _ = ctx.fill();

    // Draw active segment background
    let active_x = if active_segment == 0 {
        x + padding
    } else {
        x + segment_w + padding / 2.0
    };
    draw_round_rect(
        ctx,
        active_x,
        y + padding,
        segment_w - padding * 1.5,
        h - padding * 2.0,
        inner_radius,
    );
    set_color(ctx, COLOR_SEGMENT_ACTIVE);
    let _ = ctx.fill();

    // Draw hover effect on inactive segment
    if let Some(hover_idx) = hover
        && hover_idx != active_segment
    {
        let hover_x = if hover_idx == 0 {
            x + padding
        } else {
            x + segment_w + padding / 2.0
        };
        draw_round_rect(
            ctx,
            hover_x,
            y + padding,
            segment_w - padding * 1.5,
            h - padding * 2.0,
            inner_radius,
        );
        set_color(ctx, COLOR_SEGMENT_HOVER);
        let _ = ctx.fill();
    }

    // Draw center divider (only when not hovering on the divider area)
    ctx.set_line_width(1.0);
    set_color(ctx, COLOR_SEGMENT_DIVIDER);
    ctx.move_to(x + segment_w, y + 4.0);
    ctx.line_to(x + segment_w, y + h - 4.0);
    let _ = ctx.stroke();

    // Draw labels
    for (i, label) in [labels.0, labels.1].iter().enumerate() {
        let text_color = if i == active_segment {
            COLOR_SEGMENT_TEXT_ACTIVE
        } else {
            COLOR_SEGMENT_TEXT_INACTIVE
        };
        set_color(ctx, text_color);
        let label_x = if i == 0 { x } else { x + segment_w };

        // Center the label in the segment
        let layout = text_layout(ctx, label_style, label, None);
        let ext = layout.ink_extents();
        let tx = label_x + (segment_w - ext.width()) / 2.0 - ext.x_bearing();
        let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
        layout.show_at_baseline(ctx, tx, ty);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_handle_bars_center_full_stack_in_button() {
        let y = 12.0;
        let h = 18.0;
        let bar_h = 2.0;
        let bar_gap = 2.0;

        let start_y = drag_handle_bar_start_y(y, h, bar_h, bar_gap);
        let stack_h = 3.0 * bar_h + 2.0 * bar_gap;

        assert_eq!(start_y, 16.0);
        assert_eq!(start_y + stack_h / 2.0, y + h / 2.0);
    }
}
