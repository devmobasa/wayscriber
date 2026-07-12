use std::f64::consts::PI;

use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::config::Action;
use crate::draw::Color;
use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};
use crate::ui_text::UiTextStyle;

use super::super::super::widgets::constants::FONT_FAMILY_DEFAULT;
use super::super::super::widgets::{set_icon_color, *};

pub(super) type ColorSwatch = (Color, String, Option<Action>);
pub(super) type ColorToggleIconFn = fn(&cairo::Context, f64, f64, f64);

#[derive(Clone)]
pub(super) struct ColorSwatchToggle {
    pub(super) event: ToolbarEvent,
    pub(super) tooltip: &'static str,
    pub(super) icon_fn: ColorToggleIconFn,
}

#[derive(Copy, Clone)]
pub(super) struct ColorSwatchRowLayout {
    pub(super) start_x: f64,
    pub(super) row_y: f64,
    pub(super) swatch: f64,
    pub(super) swatch_gap: f64,
}

/// HSV triple the picker should display. The remembered picker triple wins
/// while it still resolves to the current color, so hue and saturation stay
/// put when the RGB value collapses to gray/black; any other color source
/// (swatch, hex, preset) falls back to a plain RGB→HSV conversion.
pub(super) fn effective_hsv(snapshot: &ToolbarSnapshot) -> (f64, f64, f64) {
    if let Some((h, s, v)) = snapshot.picker_hsv {
        let remembered = crate::draw::color::hsv_to_rgb(h, s, v);
        let current = snapshot.color;
        if (remembered.r - current.r).abs() < 1e-3
            && (remembered.g - current.g).abs() < 1e-3
            && (remembered.b - current.b).abs() < 1e-3
        {
            return (h, s, v);
        }
    }
    crate::draw::color::rgb_to_hsv(snapshot.color.r, snapshot.color.g, snapshot.color.b)
}

/// Draw the 2-D saturation/value area plus the hue bar; returns the total
/// block height so the rows below can stack after it.
pub(super) fn draw_color_picker_area(
    ctx: &cairo::Context,
    hits: &mut Vec<HitRegion>,
    snapshot: &ToolbarSnapshot,
    x: f64,
    picker_y: f64,
    picker_w: f64,
) -> f64 {
    let sv_h = ToolbarLayoutSpec::SIDE_COLOR_SV_HEIGHT;
    let hue_h = ToolbarLayoutSpec::SIDE_COLOR_HUE_HEIGHT;
    let hue_gap = ToolbarLayoutSpec::SIDE_COLOR_HUE_GAP;
    let (h, s, v) = effective_hsv(snapshot);

    draw_sat_val_area(ctx, x, picker_y, picker_w, sv_h, h);
    hits.push(HitRegion {
        focus_id: None,
        rect: (x, picker_y, picker_w, sv_h),
        event: ToolbarEvent::SetColorHsv { h, s, v },
        kind: HitKind::PickSatVal { hue: h },
        tooltip: None,
    });
    draw_color_indicator(
        ctx,
        x + s * picker_w,
        picker_y + (1.0 - v) * sv_h,
        snapshot.color,
    );

    let hue_y = picker_y + sv_h + hue_gap;
    draw_hue_bar(ctx, x, hue_y, picker_w, hue_h);
    hits.push(HitRegion {
        focus_id: None,
        rect: (x, hue_y, picker_w, hue_h),
        event: ToolbarEvent::SetColorHsv { h, s, v },
        kind: HitKind::PickHue { sat: s, val: v },
        tooltip: None,
    });
    let hue_color = crate::draw::color::hsv_to_rgb(h, 1.0, 1.0);
    draw_color_indicator(ctx, x + h * picker_w, hue_y + hue_h / 2.0, hue_color);

    sv_h + hue_gap + hue_h
}

pub(super) fn draw_preview_swatch_and_icon(
    ctx: &cairo::Context,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
    snapshot: &ToolbarSnapshot,
    x: f64,
    picker_y: f64,
    picker_h: f64,
) -> (f64, f64) {
    let preview_row_y = picker_y + picker_h + ToolbarLayoutSpec::SIDE_COLOR_PREVIEW_GAP_TOP;
    let preview_size = ToolbarLayoutSpec::SIDE_COLOR_PREVIEW_SIZE;
    let preview_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, preview_row_y, preview_size, preview_size))
        .unwrap_or(false);

    if preview_hover {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.3);
        draw_round_rect(
            ctx,
            x - 2.0,
            preview_row_y - 2.0,
            preview_size + 4.0,
            preview_size + 4.0,
            6.0,
        );
        let _ = ctx.fill();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.7);
        ctx.set_line_width(1.5);
    } else {
        ctx.set_source_rgba(0.5, 0.55, 0.6, 0.6);
        ctx.set_line_width(1.0);
    }
    draw_round_rect(
        ctx,
        x - 1.0,
        preview_row_y - 1.0,
        preview_size + 2.0,
        preview_size + 2.0,
        5.0,
    );
    let _ = ctx.stroke();

    draw_swatch(
        ctx,
        x,
        preview_row_y,
        preview_size,
        snapshot.color,
        preview_hover,
    );

    let icon_size = ToolbarLayoutSpec::SIDE_COLOR_EXPAND_ICON_SIZE;
    let icon_x = x + preview_size - icon_size - 2.0;
    let icon_y = preview_row_y + preview_size - icon_size - 2.0;
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.4);
    ctx.arc(
        icon_x + icon_size / 2.0,
        icon_y + icon_size / 2.0,
        icon_size / 2.0 + 1.0,
        0.0,
        PI * 2.0,
    );
    let _ = ctx.fill();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    ctx.set_line_width(1.2);
    ctx.set_line_cap(cairo::LineCap::Round);
    let arrow_margin = icon_size * 0.2;
    let arrow_x1 = icon_x + arrow_margin;
    let arrow_y1 = icon_y + icon_size - arrow_margin;
    let arrow_x2 = icon_x + icon_size - arrow_margin;
    let arrow_y2 = icon_y + arrow_margin;
    ctx.move_to(arrow_x1, arrow_y1);
    ctx.line_to(arrow_x2, arrow_y2);
    let _ = ctx.stroke();
    let head_len = icon_size * 0.3;
    ctx.move_to(arrow_x2 - head_len, arrow_y2);
    ctx.line_to(arrow_x2, arrow_y2);
    ctx.line_to(arrow_x2, arrow_y2 + head_len);
    let _ = ctx.stroke();

    hits.push(HitRegion {
        focus_id: None,
        rect: (x, preview_row_y, preview_size, preview_size),
        event: ToolbarEvent::OpenColorPickerPopup,
        kind: HitKind::Click,
        tooltip: Some("Click to pick color".to_string()),
    });

    (preview_row_y, preview_size)
}

pub(super) fn draw_hex_input(
    ctx: &cairo::Context,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
    x: f64,
    preview_row_y: f64,
    preview_size: f64,
    hex: &str,
) {
    let hex_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 11.0,
    };

    let hex_input_x = x + preview_size + 8.0;
    let hex_input_h = ToolbarLayoutSpec::SIDE_COLOR_HEX_INPUT_HEIGHT;
    let hex_input_y = preview_row_y + (preview_size - hex_input_h) / 2.0;
    let hex_input_w = ToolbarLayoutSpec::SIDE_COLOR_HEX_INPUT_WIDTH;
    let hex_icon_size = 10.0;
    let hex_icon_pad = 4.0;

    let hex_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, hex_input_x, hex_input_y, hex_input_w, hex_input_h))
        .unwrap_or(false);

    if hex_hover {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.06);
        draw_round_rect(
            ctx,
            hex_input_x - 1.0,
            hex_input_y - 1.0,
            hex_input_w + 2.0,
            hex_input_h + 2.0,
            5.0,
        );
        let _ = ctx.fill();
        ctx.set_source_rgba(0.35, 0.35, 0.4, 0.9);
    } else {
        ctx.set_source_rgba(0.2, 0.2, 0.2, 0.6);
    }
    draw_round_rect(ctx, hex_input_x, hex_input_y, hex_input_w, hex_input_h, 4.0);
    let _ = ctx.fill();

    if hex_hover {
        ctx.set_source_rgba(0.5, 0.5, 0.55, 0.6);
        ctx.set_line_width(1.0);
        draw_round_rect(ctx, hex_input_x, hex_input_y, hex_input_w, hex_input_h, 4.0);
        let _ = ctx.stroke();
    }

    let copy_icon_x = hex_input_x + hex_input_w - hex_icon_size - hex_icon_pad;
    let copy_icon_y = hex_input_y + (hex_input_h - hex_icon_size) / 2.0;
    if hex_hover {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.85);
    } else {
        ctx.set_source_rgba(0.6, 0.6, 0.6, 0.5);
    }
    toolbar_icons::draw_icon_copy(ctx, copy_icon_x, copy_icon_y, hex_icon_size);

    ctx.set_source_rgba(0.85, 0.85, 0.85, 1.0);
    let hex_layout = crate::ui_text::text_layout(ctx, hex_style, hex, None);
    let hex_extents = hex_layout.ink_extents();
    let text_area_w = hex_input_w - hex_icon_size - hex_icon_pad;
    hex_layout.show_at_baseline(
        ctx,
        hex_input_x + (text_area_w - hex_extents.width()) / 2.0,
        hex_input_y + hex_input_h / 2.0 + hex_extents.height() / 2.0,
    );

    // The copy icon is pushed first so it wins the overlap with the chip.
    hits.push(HitRegion {
        focus_id: None,
        rect: (copy_icon_x, copy_icon_y, hex_icon_size, hex_icon_size),
        event: ToolbarEvent::CopyHexColor,
        kind: HitKind::Click,
        tooltip: Some("Copy hex color".to_string()),
    });
    hits.push(HitRegion {
        focus_id: None,
        rect: (hex_input_x, hex_input_y, hex_input_w, hex_input_h),
        event: ToolbarEvent::EditHexColor,
        kind: HitKind::Click,
        tooltip: Some("Type a hex color (Enter applies)".to_string()),
    });

    let paste_btn_x = hex_input_x + hex_input_w + 4.0;
    let paste_btn_size = 20.0;
    let paste_btn_hover = hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                paste_btn_x,
                hex_input_y,
                paste_btn_size,
                paste_btn_size,
            )
        })
        .unwrap_or(false);
    draw_button(
        ctx,
        paste_btn_x,
        hex_input_y,
        paste_btn_size,
        paste_btn_size,
        false,
        paste_btn_hover,
    );
    set_icon_color(ctx, paste_btn_hover);
    toolbar_icons::draw_icon_paste(
        ctx,
        paste_btn_x + (paste_btn_size - 12.0) / 2.0,
        hex_input_y + (paste_btn_size - 12.0) / 2.0,
        12.0,
    );
    hits.push(HitRegion {
        focus_id: None,
        rect: (paste_btn_x, hex_input_y, paste_btn_size, paste_btn_size),
        event: ToolbarEvent::PasteHexColor,
        kind: HitKind::Click,
        tooltip: Some("Paste hex color from clipboard".to_string()),
    });

    let eyedropper_x = paste_btn_x + paste_btn_size + 4.0;
    let eyedropper_hover = hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                eyedropper_x,
                hex_input_y,
                paste_btn_size,
                paste_btn_size,
            )
        })
        .unwrap_or(false);
    draw_button(
        ctx,
        eyedropper_x,
        hex_input_y,
        paste_btn_size,
        paste_btn_size,
        false,
        eyedropper_hover,
    );
    set_icon_color(ctx, eyedropper_hover);
    toolbar_icons::draw_icon_eyedropper(
        ctx,
        eyedropper_x + 4.0,
        hex_input_y + 4.0,
        paste_btn_size - 8.0,
    );
    hits.push(HitRegion {
        focus_id: None,
        rect: (eyedropper_x, hex_input_y, paste_btn_size, paste_btn_size),
        event: ToolbarEvent::PickScreenColor,
        kind: HitKind::Click,
        tooltip: Some("Pick color from screen".to_string()),
    });
}

pub(super) fn draw_color_swatch_row(
    ctx: &cairo::Context,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
    snapshot: &ToolbarSnapshot,
    layout: ColorSwatchRowLayout,
    colors: &[ColorSwatch],
    toggle: Option<ColorSwatchToggle>,
) {
    let mut x = layout.start_x;
    for (color, name, action) in colors {
        draw_swatch(
            ctx,
            x,
            layout.row_y,
            layout.swatch,
            *color,
            *color == snapshot.color,
        );
        let binding = action.and_then(|action| snapshot.binding_hints.binding_for_action(action));
        let tooltip = format_binding_label(name, binding);
        hits.push(HitRegion {
            focus_id: None,
            rect: (x, layout.row_y, layout.swatch, layout.swatch),
            event: ToolbarEvent::SetColor(*color),
            kind: HitKind::Click,
            tooltip: Some(tooltip),
        });
        x += layout.swatch + layout.swatch_gap;
    }

    let Some(toggle) = toggle else {
        return;
    };

    let is_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, layout.row_y, layout.swatch, layout.swatch))
        .unwrap_or(false);
    draw_button(
        ctx,
        x,
        layout.row_y,
        layout.swatch,
        layout.swatch,
        false,
        is_hover,
    );
    set_icon_color(ctx, is_hover);
    (toggle.icon_fn)(
        ctx,
        x + (layout.swatch - 14.0) / 2.0,
        layout.row_y + (layout.swatch - 14.0) / 2.0,
        14.0,
    );
    hits.push(HitRegion {
        focus_id: None,
        rect: (x, layout.row_y, layout.swatch, layout.swatch),
        event: toggle.event,
        kind: HitKind::Click,
        tooltip: Some(toggle.tooltip.to_string()),
    });
}

#[cfg(test)]
mod tests {
    use super::effective_hsv;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::{ToolbarBindingHints, ToolbarSnapshot};

    #[test]
    fn effective_hsv_prefers_matching_memory_and_rejects_stale() {
        let state = make_test_input_state();
        let mut snapshot =
            ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());

        // White picked at hue 0.4: RGB loses the hue, memory keeps it.
        snapshot.color = crate::draw::color::hsv_to_rgb(0.4, 0.0, 1.0);
        snapshot.picker_hsv = Some((0.4, 0.0, 1.0));
        assert_eq!(effective_hsv(&snapshot), (0.4, 0.0, 1.0));

        // The color changed via another source (swatch/hex); stale memory
        // must lose to the actual color.
        snapshot.color = crate::draw::color::hsv_to_rgb(0.0, 1.0, 1.0);
        let (h, s, v) = effective_hsv(&snapshot);
        assert!(h.abs() < 1e-9);
        assert!((s - 1.0).abs() < 1e-9);
        assert!((v - 1.0).abs() < 1e-9);
    }
}
