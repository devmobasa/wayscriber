use super::constants::{
    COLOR_TEXT_PRIMARY, COLOR_TOOLTIP_BACKGROUND, COLOR_TOOLTIP_BORDER, COLOR_TOOLTIP_SHADOW,
    FONT_FAMILY_DEFAULT, FONT_SIZE_TOOLTIP, LINE_WIDTH_THIN, RADIUS_STD, SPACING_LG, SPACING_MD,
    SPACING_STD, SPACING_XS, set_color,
};
use super::draw_round_rect;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::ui_text::{UiTextStyle, text_layout};

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
            let style = UiTextStyle {
                family: FONT_FAMILY_DEFAULT,
                slant: cairo::FontSlant::Normal,
                weight: cairo::FontWeight::Normal,
                size: FONT_SIZE_TOOLTIP,
            };
            let pad = SPACING_STD;
            let max_tooltip_w = (panel_width - SPACING_LG).max(40.0);
            let max_text_w = (max_tooltip_w - pad * 2.0).max(20.0);
            let layout = text_layout(ctx, style, text, Some(max_text_w));
            let ink_extents = layout.ink_extents();
            let text_w = ink_extents.width().max(1.0);
            let text_h = ink_extents.height().max(1.0);
            let tooltip_w = (text_w + pad * 2.0).min(max_tooltip_w);
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

            let text_x = tooltip_x + pad - ink_extents.x_bearing();
            let text_y = tooltip_y + pad - ink_extents.y_bearing();
            set_color(ctx, COLOR_TEXT_PRIMARY);
            layout.show_at_baseline(ctx, text_x, text_y);
            break;
        }
    }
}
