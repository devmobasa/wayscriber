use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::draw::Color;
use std::f64::consts::{FRAC_PI_2, PI};

pub(super) fn draw_panel_background(ctx: &cairo::Context, width: f64, height: f64) {
    ctx.set_source_rgba(0.05, 0.05, 0.08, 0.92);
    draw_round_rect(ctx, 0.0, 0.0, width, height, 14.0);
    let _ = ctx.fill();
}

pub(super) fn draw_drag_handle(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, hover: bool) {
    draw_round_rect(ctx, x, y, w, h, 4.0);
    let alpha = if hover { 0.9 } else { 0.6 };
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha * 0.5);
    let _ = ctx.fill();

    ctx.set_line_width(1.1);
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha);
    let bar_w = w * 0.55;
    let bar_h = 2.0;
    let bar_x = x + (w - bar_w) / 2.0;
    let mut bar_y = y + (h - 3.0 * bar_h) / 2.0;
    for _ in 0..3 {
        draw_round_rect(ctx, bar_x, bar_y, bar_w, bar_h, 1.0);
        let _ = ctx.fill();
        bar_y += bar_h + 2.0;
    }
}

pub(super) fn draw_group_card(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    ctx.set_source_rgba(0.12, 0.12, 0.18, 0.35);
    draw_round_rect(ctx, x, y, w, h, 8.0);
    let _ = ctx.fill();
}

pub(super) fn point_in_rect(px: f64, py: f64, x: f64, y: f64, w: f64, h: f64) -> bool {
    px >= x && px <= x + w && py >= y && py <= y + h
}

pub(super) fn set_icon_color(ctx: &cairo::Context, hover: bool) {
    if hover {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    } else {
        ctx.set_source_rgba(0.95, 0.95, 0.95, 0.9);
    }
}

pub(super) fn draw_tooltip(
    ctx: &cairo::Context,
    hits: &[HitRegion],
    hover: Option<(f64, f64)>,
    panel_width: f64,
    above: bool,
) {
    let Some((hx, hy)) = hover else { return };

    for hit in hits {
        if hit.contains(hx, hy)
            && let Some(text) = &hit.tooltip
        {
            ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            ctx.set_font_size(12.0);

            let pad = 6.0;
            let max_tooltip_w = (panel_width - 8.0).max(40.0);
            let max_text_w = (max_tooltip_w - pad * 2.0).max(20.0);
            let lines = wrap_tooltip_lines(ctx, text, max_text_w);
            let mut max_line_w: f64 = 0.0;
            for line in &lines {
                if let Ok(ext) = ctx.text_extents(line) {
                    max_line_w = max_line_w.max(ext.width() + ext.x_bearing().abs());
                }
            }
            let tooltip_w = (max_line_w + pad * 2.0).min(max_tooltip_w);
            let font_extents = ctx.font_extents().ok();
            let line_height = font_extents
                .as_ref()
                .map(|ext| ext.height())
                .unwrap_or(12.0)
                .max(12.0);
            let line_gap = 2.0;
            let text_h = if lines.is_empty() {
                0.0
            } else {
                line_height * lines.len() as f64 + line_gap * (lines.len().saturating_sub(1)) as f64
            };
            let tooltip_h = text_h + pad * 2.0;

            let btn_center_x = hit.rect.0 + hit.rect.2 / 2.0;
            let mut tooltip_x = btn_center_x - tooltip_w / 2.0;
            let gap = 6.0;
            let tooltip_y = if above {
                hit.rect.1 - tooltip_h - gap
            } else {
                hit.rect.1 + hit.rect.3 + gap
            };

            if tooltip_x < 4.0 {
                tooltip_x = 4.0;
            }
            if tooltip_x + tooltip_w > panel_width - 4.0 {
                tooltip_x = panel_width - tooltip_w - 4.0;
            }

            let shadow_offset = 2.0;
            ctx.set_source_rgba(0.0, 0.0, 0.0, 0.3);
            draw_round_rect(
                ctx,
                tooltip_x + shadow_offset,
                tooltip_y + shadow_offset,
                tooltip_w,
                tooltip_h,
                4.0,
            );
            let _ = ctx.fill();

            ctx.set_source_rgba(0.1, 0.1, 0.15, 0.95);
            draw_round_rect(ctx, tooltip_x, tooltip_y, tooltip_w, tooltip_h, 4.0);
            let _ = ctx.fill();

            ctx.set_source_rgba(0.4, 0.4, 0.5, 0.8);
            ctx.set_line_width(1.0);
            draw_round_rect(ctx, tooltip_x, tooltip_y, tooltip_w, tooltip_h, 4.0);
            let _ = ctx.stroke();

            let ascent = font_extents
                .as_ref()
                .map(|ext| ext.ascent())
                .unwrap_or(line_height * 0.8);
            for (idx, line) in lines.iter().enumerate() {
                let line_y = tooltip_y + pad + ascent + idx as f64 * (line_height + line_gap);
                if let Ok(ext) = ctx.text_extents(line) {
                    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
                    ctx.move_to(tooltip_x + pad - ext.x_bearing(), line_y);
                    let _ = ctx.show_text(line);
                }
            }
            break;
        }
    }
}

pub(super) fn wrap_tooltip_lines(ctx: &cairo::Context, text: &str, max_width: f64) -> Vec<String> {
    if max_width <= 0.0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if let Ok(ext) = ctx.text_extents(word)
            && ext.width() > max_width
        {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let mut part = String::new();
            for ch in word.chars() {
                let candidate = format!("{part}{ch}");
                let width = ctx
                    .text_extents(&candidate)
                    .map(|ext| ext.width())
                    .unwrap_or(0.0);
                if width <= max_width || part.is_empty() {
                    part = candidate;
                } else {
                    lines.push(std::mem::take(&mut part));
                    part = ch.to_string();
                }
            }
            if !part.is_empty() {
                lines.push(part);
            }
            continue;
        }

        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        let width = ctx
            .text_extents(&candidate)
            .map(|ext| ext.width())
            .unwrap_or(0.0);
        if width <= max_width || current.is_empty() {
            current = candidate;
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(text.to_string());
    }
    lines
}

pub(super) fn draw_close_button(ctx: &cairo::Context, x: f64, y: f64, size: f64, hover: bool) {
    let r = size / 2.0;
    let cx = x + r;
    let cy = y + r;

    if hover {
        ctx.set_source_rgba(0.8, 0.3, 0.3, 0.9);
    } else {
        ctx.set_source_rgba(0.5, 0.5, 0.55, 0.7);
    }
    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    ctx.set_line_width(2.0);
    let inset = size * 0.3;
    ctx.move_to(x + inset, y + inset);
    ctx.line_to(x + size - inset, y + size - inset);
    let _ = ctx.stroke();
    ctx.move_to(x + size - inset, y + inset);
    ctx.line_to(x + inset, y + size - inset);
    let _ = ctx.stroke();
}

pub(super) fn draw_pin_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    pinned: bool,
    hover: bool,
) {
    let (r, g, b, a) = if pinned {
        (0.25, 0.6, 0.35, 0.95)
    } else if hover {
        (0.35, 0.35, 0.45, 0.85)
    } else {
        (0.3, 0.3, 0.35, 0.7)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, size, size, 4.0);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    let cx = x + size / 2.0;
    let cy = y + size / 2.0;
    let pin_r = size * 0.2;

    ctx.arc(cx, cy - pin_r * 0.5, pin_r, 0.0, PI * 2.0);
    let _ = ctx.fill();

    ctx.set_line_width(2.0);
    ctx.move_to(cx, cy + pin_r * 0.5);
    ctx.line_to(cx, cy + pin_r * 2.0);
    let _ = ctx.stroke();
}

pub(super) fn draw_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    active: bool,
    hover: bool,
) {
    let (r, g, b, a) = if active {
        (0.25, 0.5, 0.95, 0.95)
    } else if hover {
        (0.35, 0.35, 0.45, 0.85)
    } else {
        (0.2, 0.22, 0.26, 0.75)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 6.0);
    let _ = ctx.fill();
}

pub(super) fn draw_label_center(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, text: &str) {
    if let Ok(ext) = ctx.text_extents(text) {
        let tx = x + (w - ext.width()) / 2.0 - ext.x_bearing();
        let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.move_to(tx, ty);
        let _ = ctx.show_text(text);
    }
}

pub(super) fn draw_label_center_color(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    text: &str,
    color: (f64, f64, f64, f64),
) {
    if let Ok(ext) = ctx.text_extents(text) {
        let tx = x + (w - ext.width()) / 2.0 - ext.x_bearing();
        let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
        ctx.set_source_rgba(color.0, color.1, color.2, color.3);
        ctx.move_to(tx, ty);
        let _ = ctx.show_text(text);
    }
}

pub(super) fn draw_label_left(ctx: &cairo::Context, x: f64, y: f64, _w: f64, h: f64, text: &str) {
    if let Ok(ext) = ctx.text_extents(text) {
        let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.move_to(x, ty);
        let _ = ctx.show_text(text);
    }
}

pub(super) fn draw_section_label(ctx: &cairo::Context, x: f64, y: f64, text: &str) {
    ctx.set_source_rgba(0.8, 0.8, 0.85, 0.9);
    ctx.move_to(x, y);
    let _ = ctx.show_text(text);
}

pub(super) fn draw_swatch(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    color: Color,
    active: bool,
) {
    ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
    draw_round_rect(ctx, x, y, size, size, 4.0);
    let _ = ctx.fill();

    let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
    if luminance < 0.3 {
        ctx.set_source_rgba(0.5, 0.5, 0.5, 0.8);
        ctx.set_line_width(1.5);
        draw_round_rect(ctx, x, y, size, size, 4.0);
        let _ = ctx.stroke();
    }

    if active {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        ctx.set_line_width(2.0);
        draw_round_rect(ctx, x - 2.0, y - 2.0, size + 4.0, size + 4.0, 5.0);
        let _ = ctx.stroke();
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_checkbox(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    checked: bool,
    hover: bool,
    label: &str,
) {
    let (r, g, b, a) = if hover {
        (0.32, 0.34, 0.4, 0.9)
    } else {
        (0.22, 0.24, 0.28, 0.75)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 4.0);
    let _ = ctx.fill();

    let box_size = h * 0.55;
    let box_x = x + 8.0;
    let box_y = y + (h - box_size) / 2.0;
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    ctx.rectangle(box_x, box_y, box_size, box_size);
    ctx.set_line_width(1.5);
    let _ = ctx.stroke();
    if checked {
        ctx.move_to(box_x + 3.0, box_y + box_size / 2.0);
        ctx.line_to(box_x + box_size / 2.0, box_y + box_size - 3.0);
        ctx.line_to(box_x + box_size - 3.0, box_y + 3.0);
        let _ = ctx.stroke();
    }

    let label_x = box_x + box_size + 8.0;
    draw_label_left(ctx, label_x, y, w - (label_x - x), h, label);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_mini_checkbox(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    checked: bool,
    hover: bool,
    label: &str,
) {
    let (r, g, b, a) = if checked {
        (0.25, 0.5, 0.35, 0.9)
    } else if hover {
        (0.32, 0.34, 0.4, 0.85)
    } else {
        (0.2, 0.22, 0.26, 0.7)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 3.0);
    let _ = ctx.fill();

    let box_size = h * 0.6;
    let box_x = x + 4.0;
    let box_y = y + (h - box_size) / 2.0;
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.85);
    ctx.rectangle(box_x, box_y, box_size, box_size);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    if checked {
        ctx.move_to(box_x + 2.0, box_y + box_size / 2.0);
        ctx.line_to(box_x + box_size / 2.0, box_y + box_size - 2.0);
        ctx.line_to(box_x + box_size - 2.0, box_y + 2.0);
        let _ = ctx.stroke();
    }

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(10.0);
    if let Ok(ext) = ctx.text_extents(label) {
        let label_x = x + box_size + 8.0 + (w - box_size - 12.0 - ext.width()) / 2.0;
        let label_y = y + (h + ext.height()) / 2.0;
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        ctx.move_to(label_x, label_y);
        let _ = ctx.show_text(label);
    }
}

pub(super) fn draw_color_picker(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    let hue_grad = cairo::LinearGradient::new(x, y, x + w, y);
    hue_grad.add_color_stop_rgba(0.0, 1.0, 0.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.17, 1.0, 1.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.33, 0.0, 1.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.5, 0.0, 1.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(0.66, 0.0, 0.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(0.83, 1.0, 0.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(1.0, 1.0, 0.0, 0.0, 1.0);

    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&hue_grad);
    let _ = ctx.fill();

    let val_grad = cairo::LinearGradient::new(x, y, x, y + h);
    val_grad.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.0);
    val_grad.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.65);
    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&val_grad);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.4);
    ctx.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}

pub(super) fn draw_round_rect(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    let r = radius.min(w / 2.0).min(h / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + w - r, y + r, r, -FRAC_PI_2, 0.0);
    ctx.arc(x + w - r, y + h - r, r, 0.0, FRAC_PI_2);
    ctx.arc(x + r, y + h - r, r, FRAC_PI_2, PI);
    ctx.arc(x + r, y + r, r, PI, PI * 1.5);
    ctx.close_path();
}
