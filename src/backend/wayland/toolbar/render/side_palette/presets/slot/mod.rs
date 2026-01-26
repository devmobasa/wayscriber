use crate::draw::Color;
use crate::ui_text::{UiTextStyle, text_layout};

use super::super::super::widgets::constants::FONT_FAMILY_DEFAULT;
use super::super::super::widgets::{draw_button, draw_round_rect, point_in_rect};
use super::SidePaletteLayout;
use super::widgets::draw_keycap;

mod actions;
mod content;
mod feedback;

pub(super) struct PresetSlotLayout {
    pub(super) slot_size: f64,
    pub(super) slot_gap: f64,
    pub(super) slot_row_y: f64,
    pub(super) action_row_y: f64,
    pub(super) action_h: f64,
    pub(super) action_gap: f64,
    pub(super) action_w: f64,
    pub(super) action_icon: f64,
    pub(super) icon_size: f64,
    pub(super) swatch_size: f64,
    pub(super) number_box: f64,
    pub(super) keycap_pad: f64,
    pub(super) keycap_radius: f64,
    pub(super) card_x: f64,
    pub(super) card_w: f64,
    pub(super) section_y: f64,
}

pub(super) fn draw_preset_slot(
    layout: &mut SidePaletteLayout,
    layout_spec: &PresetSlotLayout,
    slot_index: usize,
) -> Option<Color> {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let x = layout.x;

    let slot = slot_index + 1;
    let slot_x = x + slot_index as f64 * (layout_spec.slot_size + layout_spec.slot_gap);
    let preset = snapshot
        .presets
        .get(slot_index)
        .and_then(|preset| preset.as_ref());
    let preset_exists = preset.is_some();

    // Check hover for both filled and empty slots
    let slot_hover_any = hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                slot_x,
                layout_spec.slot_row_y,
                layout_spec.slot_size,
                layout_spec.slot_size,
            )
        })
        .unwrap_or(false);

    // Hover for filled slots (for apply action)
    let slot_hover = slot_hover_any && preset_exists;
    // Hover for empty slots (for save hint)
    let empty_slot_hover = slot_hover_any && !preset_exists;

    draw_button(
        ctx,
        slot_x,
        layout_spec.slot_row_y,
        layout_spec.slot_size,
        layout_spec.slot_size,
        false,
        slot_hover || empty_slot_hover,
    );

    if let Some(preset) = preset {
        ctx.set_source_rgba(preset.color.r, preset.color.g, preset.color.b, 0.12);
        draw_round_rect(
            ctx,
            slot_x + 1.0,
            layout_spec.slot_row_y + 1.0,
            layout_spec.slot_size - 2.0,
            layout_spec.slot_size - 2.0,
            6.0,
        );
        let _ = ctx.fill();
        ctx.set_source_rgba(preset.color.r, preset.color.g, preset.color.b, 0.35);
        ctx.set_line_width(1.0);
        draw_round_rect(
            ctx,
            slot_x + 1.0,
            layout_spec.slot_row_y + 1.0,
            layout_spec.slot_size - 2.0,
            layout_spec.slot_size - 2.0,
            6.0,
        );
        let _ = ctx.stroke();
    } else {
        // Empty slot background - brighter on hover
        let bg_alpha = if empty_slot_hover { 0.45 } else { 0.35 };
        ctx.set_source_rgba(0.05, 0.05, 0.07, bg_alpha);
        draw_round_rect(
            ctx,
            slot_x + 1.0,
            layout_spec.slot_row_y + 1.0,
            layout_spec.slot_size - 2.0,
            layout_spec.slot_size - 2.0,
            6.0,
        );
        let _ = ctx.fill();

        // Show "Save" hint on hover for empty slots
        if empty_slot_hover {
            // Draw a subtle "+" icon or "Save" hint
            let hint_style = UiTextStyle {
                family: FONT_FAMILY_DEFAULT,
                slant: cairo::FontSlant::Normal,
                weight: cairo::FontWeight::Normal,
                size: 9.0,
            };
            let hint_text = "Save";
            let hint_layout = text_layout(ctx, hint_style, hint_text, None);
            let hint_extents = hint_layout.ink_extents();
            let hint_x = slot_x + (layout_spec.slot_size - hint_extents.width()) / 2.0
                - hint_extents.x_bearing();
            let hint_y = layout_spec.slot_row_y + layout_spec.slot_size - 8.0;
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.7);
            hint_layout.show_at_baseline(ctx, hint_x, hint_y);
        }
    }

    let hover_preset_color = content::draw_preset_content(
        ctx,
        snapshot,
        hits,
        layout_spec,
        slot_x,
        slot,
        slot_hover,
        preset,
    );

    let key_x = slot_x + layout_spec.keycap_pad;
    let key_y = layout_spec.slot_row_y + layout_spec.keycap_pad;
    draw_keycap(
        ctx,
        key_x,
        key_y,
        layout_spec.number_box,
        layout_spec.keycap_radius,
        &slot.to_string(),
        preset_exists,
    );

    feedback::draw_preset_feedback(
        ctx,
        snapshot,
        layout_spec,
        slot_index,
        slot_x,
        preset_exists,
    );

    // Draw subtle separator line between slot and action buttons
    let sep_y = layout_spec.action_row_y - 3.0;
    ctx.set_source_rgba(0.5, 0.5, 0.55, 0.25);
    ctx.set_line_width(0.5);
    ctx.move_to(slot_x + 2.0, sep_y);
    ctx.line_to(slot_x + layout_spec.slot_size - 2.0, sep_y);
    let _ = ctx.stroke();

    actions::draw_preset_actions(
        ctx,
        snapshot,
        hits,
        layout_spec,
        slot_x,
        slot,
        preset_exists,
        hover,
    );

    hover_preset_color
}
