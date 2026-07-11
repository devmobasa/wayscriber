use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::config::action_label;
use crate::draw::Color;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui::toolbar::bindings::{action_for_clear_preset, action_for_save_preset};

use super::super::super::widgets::{draw_button, draw_round_rect, point_in_rect};
use super::SidePaletteLayout;
use super::widgets::draw_keycap;

mod content;
mod feedback;

pub(super) struct PresetSlotLayout {
    pub(super) slot_size: f64,
    pub(super) slot_gap: f64,
    pub(super) slot_row_y: f64,
    pub(super) icon_size: f64,
    pub(super) swatch_size: f64,
    pub(super) number_box: f64,
    pub(super) keycap_pad: f64,
    pub(super) keycap_radius: f64,
    pub(super) card_x: f64,
    pub(super) card_w: f64,
    pub(super) section_y: f64,
}

/// Size of the hover-revealed clear (✕) badge in a filled slot's corner.
const CLEAR_BADGE_SIZE: f64 = 14.0;

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

    // The clear badge sits over the apply target and hit-testing is
    // first-match, so its region must precede the slot's apply region.
    let clear_badge = (preset_exists && slot_hover_any).then(|| {
        let badge_x = slot_x + layout_spec.slot_size - CLEAR_BADGE_SIZE - 2.0;
        let badge_y = layout_spec.slot_row_y + 2.0;
        hits.push(HitRegion {
            focus_id: None,
            rect: (badge_x, badge_y, CLEAR_BADGE_SIZE, CLEAR_BADGE_SIZE),
            event: ToolbarEvent::ClearPreset(slot),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_for_clear_preset(slot)
                    .map(action_label)
                    .unwrap_or("Clear Preset"),
                snapshot.binding_hints.clear_preset(slot),
            )),
        });
        (badge_x, badge_y)
    });

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
        // Empty slot: click saves the current tool setup here.
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

        hits.push(HitRegion {
            focus_id: None,
            rect: (
                slot_x,
                layout_spec.slot_row_y,
                layout_spec.slot_size,
                layout_spec.slot_size,
            ),
            event: ToolbarEvent::SavePreset(slot),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_for_save_preset(slot)
                    .map(action_label)
                    .unwrap_or("Save Preset"),
                snapshot.binding_hints.save_preset(slot),
            )),
        });

        let plus = layout_spec.icon_size.min(14.0);
        ctx.set_source_rgba(1.0, 1.0, 1.0, if empty_slot_hover { 0.9 } else { 0.45 });
        toolbar_icons::draw_icon_plus(
            ctx,
            slot_x + (layout_spec.slot_size - plus) / 2.0,
            layout_spec.slot_row_y + (layout_spec.slot_size - plus) / 2.0,
            plus,
        );
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

    if let Some((badge_x, badge_y)) = clear_badge {
        draw_clear_badge(ctx, badge_x, badge_y);
    }

    hover_preset_color
}

fn draw_clear_badge(ctx: &cairo::Context, x: f64, y: f64) {
    let size = CLEAR_BADGE_SIZE;
    ctx.set_source_rgba(0.75, 0.2, 0.2, 0.9);
    ctx.arc(
        x + size / 2.0,
        y + size / 2.0,
        size / 2.0,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    let inset = size * 0.3;
    ctx.set_line_width(1.6);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.move_to(x + inset, y + inset);
    ctx.line_to(x + size - inset, y + size - inset);
    let _ = ctx.stroke();
    ctx.move_to(x + size - inset, y + inset);
    ctx.line_to(x + inset, y + size - inset);
    let _ = ctx.stroke();
}
