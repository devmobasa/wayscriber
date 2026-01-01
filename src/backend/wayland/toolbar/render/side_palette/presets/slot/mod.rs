use crate::draw::Color;

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
    let slot_hover = hover
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
        .unwrap_or(false)
        && preset_exists;
    draw_button(
        ctx,
        slot_x,
        layout_spec.slot_row_y,
        layout_spec.slot_size,
        layout_spec.slot_size,
        false,
        slot_hover,
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
        ctx.set_source_rgba(0.05, 0.05, 0.07, 0.35);
        draw_round_rect(
            ctx,
            slot_x + 1.0,
            layout_spec.slot_row_y + 1.0,
            layout_spec.slot_size - 2.0,
            layout_spec.slot_size - 2.0,
            6.0,
        );
        let _ = ctx.fill();
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
