use crate::draw::ORANGE;
use crate::input::state::PresetFeedbackKind;
use crate::ui::toolbar::ToolbarSnapshot;

use super::super::super::super::widgets::draw_round_rect;
use super::PresetSlotLayout;

pub(super) fn draw_preset_feedback(
    ctx: &cairo::Context,
    snapshot: &ToolbarSnapshot,
    layout_spec: &PresetSlotLayout,
    slot_index: usize,
    slot_x: f64,
    preset_exists: bool,
) {
    if let Some(feedback) = snapshot
        .preset_feedback
        .get(slot_index)
        .and_then(|feedback| feedback.as_ref())
    {
        let fade = (1.0 - feedback.progress as f64).clamp(0.0, 1.0);
        if fade > 0.0 {
            let (r, g, b) = match feedback.kind {
                PresetFeedbackKind::Apply => (0.35, 0.55, 0.95),
                PresetFeedbackKind::Save => (0.25, 0.75, 0.4),
                PresetFeedbackKind::Clear => (0.9, 0.3, 0.3),
            };
            ctx.set_source_rgba(r, g, b, 0.35 * fade);
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
    }
    if preset_exists && snapshot.active_preset_slot == Some(slot_index + 1) {
        ctx.set_source_rgba(ORANGE.r, ORANGE.g, ORANGE.b, 0.95);
        ctx.set_line_width(2.0);
        draw_round_rect(
            ctx,
            slot_x + 1.0,
            layout_spec.slot_row_y + 1.0,
            layout_spec.slot_size - 2.0,
            layout_spec.slot_size - 2.0,
            7.0,
        );
        let _ = ctx.stroke();
    }
}
