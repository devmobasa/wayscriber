mod format;
mod header;
mod slot;
mod widgets;

use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::draw::Color;

use super::super::widgets::draw_group_card;
use header::draw_presets_header;
use slot::{PresetSlotLayout, draw_preset_slot};

pub(super) fn draw_presets_section(layout: &mut SidePaletteLayout, y: &mut f64) -> Option<Color> {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let card_x = layout.card_x;
    let card_w = layout.card_w;
    let section_gap = layout.section_gap;

    let slot_count = snapshot.preset_slot_count.min(snapshot.presets.len());
    if !snapshot.show_presets || slot_count == 0 {
        return None;
    }

    let mut hover_preset_color: Option<Color> = None;

    let presets_card_h = ToolbarLayoutSpec::SIDE_PRESET_CARD_HEIGHT;
    draw_group_card(ctx, card_x, *y, card_w, presets_card_h);
    draw_presets_header(layout, *y, card_x, card_w, slot_count);

    let slot_size = ToolbarLayoutSpec::SIDE_PRESET_SLOT_SIZE;
    let slot_gap = ToolbarLayoutSpec::SIDE_PRESET_SLOT_GAP;
    let slot_row_y = *y + ToolbarLayoutSpec::SIDE_PRESET_ROW_OFFSET_Y;
    let action_row_y = slot_row_y + slot_size + ToolbarLayoutSpec::SIDE_PRESET_ACTION_GAP;
    let action_h = ToolbarLayoutSpec::SIDE_PRESET_ACTION_HEIGHT;
    let action_gap = ToolbarLayoutSpec::SIDE_PRESET_ACTION_BUTTON_GAP;
    let action_w = (slot_size - action_gap) / 2.0;
    let action_icon = (action_h * 0.6).round();
    let icon_size = (slot_size * 0.45).round();
    let swatch_size = (slot_size * 0.35).round();
    let number_box = (slot_size * 0.4).round();
    let keycap_pad = (slot_size * 0.1).round().max(3.0);
    let keycap_radius = (number_box * 0.25).max(3.0);

    let preset_layout = PresetSlotLayout {
        slot_size,
        slot_gap,
        slot_row_y,
        action_row_y,
        action_h,
        action_gap,
        action_w,
        action_icon,
        icon_size,
        swatch_size,
        number_box,
        keycap_pad,
        keycap_radius,
        card_x,
        card_w,
        section_y: *y,
    };

    for slot_index in 0..slot_count {
        if let Some(color) = draw_preset_slot(layout, &preset_layout, slot_index) {
            hover_preset_color = Some(color);
        }
    }

    *y += presets_card_h + section_gap;

    hover_preset_color
}
