use super::constants::{
    COLOR_ACCENT_BRIGHT, COLOR_ACCENT_GLOW, COLOR_BUTTON_ACTIVE, COLOR_BUTTON_DEFAULT,
    COLOR_BUTTON_DESTRUCTIVE_HOVER, COLOR_BUTTON_DISABLED, COLOR_BUTTON_HOVER, COLOR_CLOSE_DEFAULT,
    COLOR_CLOSE_HOVER, COLOR_DRAG_HANDLE, COLOR_DRAG_HANDLE_HOVER, COLOR_FOCUS_RING,
    COLOR_ICON_HOVER, COLOR_ICON_HOVER_BG, COLOR_PIN_ACTIVE, COLOR_PIN_DEFAULT, COLOR_PIN_HOVER,
    COLOR_SEGMENT_ACTIVE, COLOR_SEGMENT_BG, COLOR_SEGMENT_DIVIDER, COLOR_SEGMENT_HOVER,
    COLOR_SEGMENT_TEXT_ACTIVE, COLOR_SEGMENT_TEXT_INACTIVE, COLOR_TEXT_PRIMARY,
    COLOR_TEXT_TERTIARY, RADIUS_LG, RADIUS_STD, set_color,
};
use super::draw_round_rect;
use crate::ui::theme::{DESTRUCTIVE_RGB, Rgba, rgba};
use crate::ui_text::{UiTextStyle, text_layout};
use std::f64::consts::PI;

/// Faint white glow behind a hovered flat button (quieter than
/// COLOR_ICON_HOVER_BG; value-coincides with COLOR_DIVIDER but is a hover
/// affordance, not a separator).
const COLOR_BUTTON_HOVER_GLOW: Rgba = (1.0, 1.0, 1.0, 0.08);
/// Warning-tinted glow behind a hovered destructive button (destructive
/// root at a faint tint alpha).
const COLOR_DESTRUCTIVE_GLOW: Rgba = rgba(DESTRUCTIVE_RGB, 0.15);

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
    set_color(
        ctx,
        if hover {
            COLOR_DRAG_HANDLE_HOVER
        } else {
            COLOR_DRAG_HANDLE
        },
    );
    let _ = ctx.fill();

    // Add subtle glow on hover
    if hover {
        set_color(ctx, COLOR_ICON_HOVER_BG);
        draw_round_rect(ctx, x - 1.0, y - 1.0, w + 2.0, h + 2.0, RADIUS_STD + 1.0);
        let _ = ctx.stroke();
    }

    set_color(
        ctx,
        if hover {
            COLOR_ICON_HOVER
        } else {
            COLOR_TEXT_TERTIARY
        },
    );
    let icon_size = w.min(h);
    crate::toolbar_icons::draw_icon_drag(
        ctx,
        x + (w - icon_size) / 2.0,
        y + (h - icon_size) / 2.0,
        icon_size,
    );
}

/// Minimize the horizontal top bar into its edge restore tab.
pub(in crate::backend::wayland::toolbar::render) fn draw_minimize_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
) {
    draw_collapse_button(
        ctx,
        x,
        y,
        size,
        hover,
        crate::toolbar_icons::draw_icon_minimize,
    );
}

/// Minimize the vertical side palette into its edge restore tab.
pub(in crate::backend::wayland::toolbar::render) fn draw_side_minimize_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
) {
    draw_collapse_button(
        ctx,
        x,
        y,
        size,
        hover,
        crate::toolbar_icons::draw_icon_side_minimize,
    );
}

fn draw_collapse_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
    glyph: fn(&cairo::Context, f64, f64, f64),
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
    let icon_size = size * 0.6;
    glyph(
        ctx,
        x + (size - icon_size) / 2.0,
        y + (size - icon_size) / 2.0,
        icon_size,
    );
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

    set_color(ctx, COLOR_TEXT_PRIMARY);
    let icon_size = size * 0.62;
    let icon_x = x + (size - icon_size) / 2.0;
    let icon_y = y + (size - icon_size) / 2.0;
    if pinned {
        crate::toolbar_icons::draw_icon_pin(ctx, icon_x, icon_y, icon_size);
    } else {
        crate::toolbar_icons::draw_icon_unpin(ctx, icon_x, icon_y, icon_size);
    }
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
        set_color(ctx, COLOR_BUTTON_HOVER_GLOW);
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
/// a normal flat button at rest, with the destructive red fill appearing
/// only on hover so the bar never carries a persistent red tile.
pub(in crate::backend::wayland::toolbar::render) fn draw_destructive_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    hover: bool,
) {
    if !hover {
        draw_button(ctx, x, y, w, h, false, false);
        return;
    }

    // Warning-tinted glow behind the hovered tile
    set_color(ctx, COLOR_DESTRUCTIVE_GLOW);
    draw_round_rect(ctx, x - 1.0, y - 1.0, w + 2.0, h + 2.0, RADIUS_LG + 1.0);
    let _ = ctx.fill();

    set_color(ctx, COLOR_BUTTON_DESTRUCTIVE_HOVER);
    draw_round_rect(ctx, x, y, w, h, RADIUS_LG);
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
