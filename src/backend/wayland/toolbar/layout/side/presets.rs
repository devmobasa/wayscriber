use super::{HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec};

pub(super) fn push_preset_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    let presets_card_h = ToolbarLayoutSpec::SIDE_PRESET_CARD_HEIGHT;
    let slot_count = ctx
        .snapshot
        .preset_slot_count
        .min(ctx.snapshot.presets.len());
    if !ctx.snapshot.show_presets || slot_count == 0 {
        return y;
    }

    let slot_size = ToolbarLayoutSpec::SIDE_PRESET_SLOT_SIZE;
    let slot_gap = ToolbarLayoutSpec::SIDE_PRESET_SLOT_GAP;
    let slot_row_y = y + ToolbarLayoutSpec::SIDE_PRESET_ROW_OFFSET_Y;
    let action_row_y = slot_row_y + slot_size + ToolbarLayoutSpec::SIDE_PRESET_ACTION_GAP;
    let action_gap = ToolbarLayoutSpec::SIDE_PRESET_ACTION_BUTTON_GAP;
    let action_w = (slot_size - action_gap) / 2.0;
    for slot_index in 0..slot_count {
        let slot = slot_index + 1;
        let slot_x = ctx.x + slot_index as f64 * (slot_size + slot_gap);
        let preset_exists = ctx
            .snapshot
            .presets
            .get(slot_index)
            .and_then(|preset| preset.as_ref())
            .is_some();
        if preset_exists {
            hits.push(HitRegion {
                rect: (slot_x, slot_row_y, slot_size, slot_size),
                event: ToolbarEvent::ApplyPreset(slot),
                kind: HitKind::Click,
                tooltip: Some(format!("Apply preset {}", slot)),
            });
        }
        hits.push(HitRegion {
            rect: (
                slot_x,
                action_row_y,
                action_w,
                ToolbarLayoutSpec::SIDE_PRESET_ACTION_HEIGHT,
            ),
            event: ToolbarEvent::SavePreset(slot),
            kind: HitKind::Click,
            tooltip: Some(format!("Save preset {}", slot)),
        });
        if preset_exists {
            hits.push(HitRegion {
                rect: (
                    slot_x + action_w + action_gap,
                    action_row_y,
                    action_w,
                    ToolbarLayoutSpec::SIDE_PRESET_ACTION_HEIGHT,
                ),
                event: ToolbarEvent::ClearPreset(slot),
                kind: HitKind::Click,
                tooltip: Some(format!("Clear preset {}", slot)),
            });
        }
    }

    y + presets_card_h + ctx.section_gap
}
