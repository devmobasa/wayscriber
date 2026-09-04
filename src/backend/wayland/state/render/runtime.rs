use crate::util::Rect;

use super::super::canvas_layer::CanvasLayerCache;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UiEffect {
    UiToast,
    PresetToast,
    TextEditEntry,
    StatusHud,
    ZoomChip,
    InputHud,
    CommandPalette,
    ColorPicker,
    ToolPreview,
    ShapeMeasureBadge,
    OcrScan,
}

impl UiEffect {
    const COUNT: usize = 11;

    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct UiEffectFlags {
    active: u16,
    blocked_feedback: bool,
}

impl UiEffectFlags {
    pub(super) const fn with(mut self, effect: UiEffect, active: bool) -> Self {
        let bit = 1 << effect.index();
        if active {
            self.active |= bit;
        } else {
            self.active &= !bit;
        }
        self
    }

    pub(super) const fn with_blocked_feedback(mut self, active: bool) -> Self {
        self.blocked_feedback = active;
        self
    }

    pub(super) const fn active(self, effect: UiEffect) -> bool {
        self.active & (1 << effect.index()) != 0
    }

    pub(super) const fn blocked_feedback(self) -> bool {
        self.blocked_feedback
    }
}

#[derive(Debug, Default)]
pub(in crate::backend::wayland::state) struct UiDamageHistory {
    prev: [Option<Rect>; UiEffect::COUNT],
    measure_picker: Vec<Rect>,
    blocked_feedback_was_active: bool,
}

impl UiDamageHistory {
    pub(super) fn previous(&self, effect: UiEffect) -> Option<Rect> {
        self.prev[effect.index()]
    }

    /// Push old and new footprints, deduplicating an unchanged footprint, and
    /// remember `current` for the next rendered frame.
    pub(super) fn roll(
        &mut self,
        effect: UiEffect,
        current: Option<Rect>,
        regions: &mut Vec<Rect>,
    ) {
        let previous = std::mem::replace(&mut self.prev[effect.index()], current);
        match (previous, current) {
            (Some(previous), Some(current)) if previous == current => regions.push(current),
            (previous, current) => {
                regions.extend(previous);
                regions.extend(current);
            }
        }
    }

    pub(super) fn roll_status_hud(
        &mut self,
        current: Option<Rect>,
        surface: Option<Rect>,
        regions: &mut Vec<Rect>,
    ) {
        let previous = self.previous(UiEffect::StatusHud);
        if previous.is_some() != current.is_some()
            && let Some(surface) = surface
        {
            self.prev[UiEffect::StatusHud.index()] = current;
            regions.push(surface);
            return;
        }
        self.roll(UiEffect::StatusHud, current, regions);
    }

    pub(super) fn roll_measure_picker(&mut self, current: Vec<Rect>, regions: &mut Vec<Rect>) {
        regions.extend(self.measure_picker.iter().copied());
        regions.extend(current.iter().copied());
        self.measure_picker = current;
    }

    /// Returns whether blocked-feedback geometry must be damaged this frame.
    pub(super) fn roll_blocked_feedback(&mut self, active: bool) -> bool {
        let damage = active || self.blocked_feedback_was_active;
        self.blocked_feedback_was_active = active;
        damage
    }
}

pub(in crate::backend::wayland) struct RenderRuntime {
    canvas_layer_cache: CanvasLayerCache,
    draw_caches: crate::draw::RenderCaches,
    ui_damage: UiDamageHistory,
    profile_ui_baseline: Vec<u8>,
}

impl RenderRuntime {
    pub(in crate::backend::wayland) fn new() -> Self {
        Self {
            canvas_layer_cache: CanvasLayerCache::new(),
            draw_caches: crate::draw::RenderCaches::default(),
            ui_damage: UiDamageHistory::default(),
            profile_ui_baseline: Vec::new(),
        }
    }

    pub(in crate::backend::wayland::state) fn canvas_layer_cache_mut(
        &mut self,
    ) -> &mut CanvasLayerCache {
        &mut self.canvas_layer_cache
    }

    pub(in crate::backend::wayland::state) fn draw_caches_mut(
        &mut self,
    ) -> &mut crate::draw::RenderCaches {
        &mut self.draw_caches
    }

    pub(in crate::backend::wayland::state) fn canvas_draw_parts_mut(
        &mut self,
    ) -> (&mut CanvasLayerCache, &mut crate::draw::RenderCaches) {
        (&mut self.canvas_layer_cache, &mut self.draw_caches)
    }

    pub(in crate::backend::wayland::state) fn ui_damage_mut(&mut self) -> &mut UiDamageHistory {
        &mut self.ui_damage
    }

    pub(in crate::backend::wayland::state) fn profile_ui_baseline(&self) -> &[u8] {
        &self.profile_ui_baseline
    }

    pub(in crate::backend::wayland::state) fn profile_ui_baseline_mut(&mut self) -> &mut Vec<u8> {
        &mut self.profile_ui_baseline
    }
}

#[cfg(test)]
mod tests {
    use super::super::tool_preview::mouse_tool_preview_damage_rect;
    use super::*;
    use crate::backend::wayland::state::buffer_damage::{BufferDamageTracker, FullDamageReason};

    fn rect(x: i32) -> Rect {
        Rect::new(x, 0, 10, 10).expect("test rectangle")
    }

    #[test]
    fn effect_slots_are_independent() {
        const EFFECTS: [UiEffect; UiEffect::COUNT] = [
            UiEffect::UiToast,
            UiEffect::PresetToast,
            UiEffect::TextEditEntry,
            UiEffect::StatusHud,
            UiEffect::ZoomChip,
            UiEffect::InputHud,
            UiEffect::CommandPalette,
            UiEffect::ColorPicker,
            UiEffect::ToolPreview,
            UiEffect::ShapeMeasureBadge,
            UiEffect::OcrScan,
        ];
        let mut history = UiDamageHistory::default();

        for (index, effect) in EFFECTS.into_iter().enumerate() {
            let mut regions = Vec::new();
            history.roll(effect, Some(rect(index as i32 * 20)), &mut regions);
            assert_eq!(regions, vec![rect(index as i32 * 20)]);
        }
        for (index, effect) in EFFECTS.into_iter().enumerate() {
            assert_eq!(history.previous(effect), Some(rect(index as i32 * 20)));
        }
    }

    #[test]
    fn tool_preview_appearance_motion_and_disappearance_roll_rendered_bounds() {
        let mut history = UiDamageHistory::default();
        let mut regions = Vec::new();
        let first = mouse_tool_preview_damage_rect(8.0, (100.0, 100.0), 800, 600)
            .expect("first preview footprint");
        let moved = mouse_tool_preview_damage_rect(8.0, (400.0, 400.0), 800, 600)
            .expect("moved preview footprint");
        assert_ne!(first, moved);

        history.roll(UiEffect::ToolPreview, Some(first), &mut regions);
        assert_eq!(regions, vec![first]);
        assert_eq!(history.previous(UiEffect::ToolPreview), Some(first));
        regions.clear();

        history.roll(UiEffect::ToolPreview, Some(moved), &mut regions);
        assert_eq!(regions, vec![first, moved]);
        assert_eq!(history.previous(UiEffect::ToolPreview), Some(moved));
        regions.clear();

        history.roll(UiEffect::ToolPreview, None, &mut regions);
        assert_eq!(regions, vec![moved]);
        assert_eq!(history.previous(UiEffect::ToolPreview), None);
        regions.clear();

        history.roll(UiEffect::ToolPreview, None, &mut regions);
        assert!(
            regions.is_empty(),
            "the hidden preview is cleared only once"
        );
    }

    #[test]
    fn status_hud_visibility_transitions_do_not_leave_stale_history() {
        let mut history = UiDamageHistory::default();
        let surface = Rect::new(0, 0, 800, 600).expect("surface");
        let first = rect(20);
        let moved = rect(100);

        for (current, expected) in [
            (Some(first), vec![surface]),
            (Some(first), vec![first]),
            (Some(moved), vec![first, moved]),
            (None, vec![surface]),
            (None, vec![]),
            (Some(moved), vec![surface]),
            (Some(moved), vec![moved]),
        ] {
            let mut regions = Vec::new();
            history.roll_status_hud(current, Some(surface), &mut regions);
            assert_eq!(regions, expected, "HUD transition to {current:?}");
            assert_eq!(history.previous(UiEffect::StatusHud), current);
        }
    }

    #[test]
    fn tool_preview_cleanup_reaches_each_reused_buffer() {
        let mut history = UiDamageHistory::default();
        let mut damage = BufferDamageTracker::new(2);
        let first = mouse_tool_preview_damage_rect(8.0, (100.0, 100.0), 800, 600)
            .expect("first preview footprint");
        let moved = mouse_tool_preview_damage_rect(8.0, (400.0, 400.0), 800, 600)
            .expect("moved preview footprint");

        // Warm both slots, then let one lag behind while the other displays
        // the preview's appearance and movement.
        for slot in [1, 2] {
            damage.take_buffer_damage_report(slot, 800, 600, 1, 4096);
        }
        for (current, expected) in [
            (Some(first), vec![first]),
            (Some(moved), vec![first, moved]),
        ] {
            let mut regions = Vec::new();
            history.roll(UiEffect::ToolPreview, current, &mut regions);
            damage.add_regions(regions);
            let report = damage.take_buffer_damage_report(1, 800, 600, 1, 4096);
            assert_eq!(report.full_reason, None);
            assert_eq!(report.regions, expected);
        }

        let mut regions = Vec::new();
        history.roll(UiEffect::ToolPreview, None, &mut regions);
        damage.add_regions(regions);
        let lagging = damage.take_buffer_damage_report(2, 800, 600, 1, 4096);
        assert_eq!(lagging.full_reason, None);
        assert_eq!(lagging.regions, vec![first, moved]);

        let recent = damage.take_buffer_damage_report(1, 800, 600, 1, 4096);
        assert_eq!(recent.full_reason, None);
        assert_eq!(recent.regions, vec![moved]);
        for slot in [1, 2] {
            assert!(
                damage
                    .take_buffer_damage_report(slot, 800, 600, 1, 4096)
                    .regions
                    .is_empty(),
                "cleanup must be drained independently for slot {slot}"
            );
        }
    }

    #[test]
    fn full_damage_does_not_replace_effect_history_for_later_cleanup() {
        let mut history = UiDamageHistory::default();
        let mut damage = BufferDamageTracker::new(2);
        let surface = Rect::new(0, 0, 800, 600).expect("surface");
        let first = rect(20);
        let moved = rect(100);
        for slot in [1, 2] {
            damage.take_buffer_damage_report(slot, 800, 600, 1, 4096);
        }
        history.roll(UiEffect::ToolPreview, Some(first), &mut Vec::new());

        // History still advances when a separate cause forces full damage.
        damage.mark_all_full(FullDamageReason::Zoom);
        let mut regions = Vec::new();
        history.roll(UiEffect::ToolPreview, Some(moved), &mut regions);
        damage.add_regions(regions);
        for slot in [1, 2] {
            let report = damage.take_buffer_damage_report(slot, 800, 600, 1, 4096);
            assert_eq!(report.regions, vec![surface]);
            assert_eq!(report.full_reason, Some(FullDamageReason::Zoom));
        }

        let mut regions = Vec::new();
        history.roll(UiEffect::ToolPreview, None, &mut regions);
        damage.add_regions(regions);
        for slot in [1, 2] {
            let report = damage.take_buffer_damage_report(slot, 800, 600, 1, 4096);
            assert_eq!(report.full_reason, None);
            assert_eq!(report.regions, vec![moved]);
        }
    }

    #[test]
    fn blocked_feedback_cleanup_is_requested_once() {
        let mut history = UiDamageHistory::default();

        assert!(history.roll_blocked_feedback(true));
        assert!(history.roll_blocked_feedback(false));
        assert!(!history.roll_blocked_feedback(false));
    }

    #[test]
    fn measure_picker_rolls_old_and_new_strips() {
        let mut history = UiDamageHistory::default();
        let mut regions = Vec::new();
        history.roll_measure_picker(vec![rect(0)], &mut regions);
        regions.clear();

        history.roll_measure_picker(vec![rect(20)], &mut regions);

        assert_eq!(regions, vec![rect(0), rect(20)]);
    }
}
