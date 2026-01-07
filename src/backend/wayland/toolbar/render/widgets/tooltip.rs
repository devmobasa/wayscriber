use super::constants::{
    COLOR_TEXT_PRIMARY, COLOR_TOOLTIP_BACKGROUND, COLOR_TOOLTIP_BORDER, COLOR_TOOLTIP_SHADOW,
    FONT_FAMILY_DEFAULT, FONT_SIZE_TOOLTIP, LINE_WIDTH_THIN, RADIUS_STD, SPACING_LG, SPACING_MD,
    SPACING_STD, SPACING_XS, set_color,
};
use super::draw_round_rect;
use crate::backend::wayland::toolbar::hit::HitRegion;

pub(in crate::backend::wayland::toolbar::render) fn draw_tooltip(
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
            ctx.select_font_face(
                FONT_FAMILY_DEFAULT,
                cairo::FontSlant::Normal,
                cairo::FontWeight::Normal,
            );
            ctx.set_font_size(FONT_SIZE_TOOLTIP);

            let pad = SPACING_STD;
            let max_tooltip_w = (panel_width - SPACING_LG).max(40.0);
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
                .unwrap_or(FONT_SIZE_TOOLTIP)
                .max(FONT_SIZE_TOOLTIP);
            let line_gap = SPACING_XS;
            let text_h = if lines.is_empty() {
                0.0
            } else {
                line_height * lines.len() as f64 + line_gap * (lines.len().saturating_sub(1)) as f64
            };
            let tooltip_h = text_h + pad * 2.0;

            let btn_center_x = hit.rect.0 + hit.rect.2 / 2.0;
            let mut tooltip_x = btn_center_x - tooltip_w / 2.0;
            let gap = SPACING_STD;
            let tooltip_y = if above {
                hit.rect.1 - tooltip_h - gap
            } else {
                hit.rect.1 + hit.rect.3 + gap
            };

            if tooltip_x < SPACING_MD {
                tooltip_x = SPACING_MD;
            }
            if tooltip_x + tooltip_w > panel_width - SPACING_MD {
                tooltip_x = panel_width - tooltip_w - SPACING_MD;
            }

            let shadow_offset = SPACING_XS;
            set_color(ctx, COLOR_TOOLTIP_SHADOW);
            draw_round_rect(
                ctx,
                tooltip_x + shadow_offset,
                tooltip_y + shadow_offset,
                tooltip_w,
                tooltip_h,
                RADIUS_STD,
            );
            let _ = ctx.fill();

            set_color(ctx, COLOR_TOOLTIP_BACKGROUND);
            draw_round_rect(ctx, tooltip_x, tooltip_y, tooltip_w, tooltip_h, RADIUS_STD);
            let _ = ctx.fill();

            set_color(ctx, COLOR_TOOLTIP_BORDER);
            ctx.set_line_width(LINE_WIDTH_THIN);
            draw_round_rect(ctx, tooltip_x, tooltip_y, tooltip_w, tooltip_h, RADIUS_STD);
            let _ = ctx.stroke();

            let ascent = font_extents
                .as_ref()
                .map(|ext| ext.ascent())
                .unwrap_or(line_height * 0.8);
            for (idx, line) in lines.iter().enumerate() {
                let line_y = tooltip_y + pad + ascent + idx as f64 * (line_height + line_gap);
                if let Ok(ext) = ctx.text_extents(line) {
                    set_color(ctx, COLOR_TEXT_PRIMARY);
                    ctx.move_to(tooltip_x + pad - ext.x_bearing(), line_y);
                    let _ = ctx.show_text(line);
                }
            }
            break;
        }
    }
}

pub(in crate::backend::wayland::toolbar::render) fn wrap_tooltip_lines(
    ctx: &cairo::Context,
    text: &str,
    max_width: f64,
) -> Vec<String> {
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
