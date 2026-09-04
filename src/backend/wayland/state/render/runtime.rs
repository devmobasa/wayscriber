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
    ui_damage: UiDamageHistory,
    profile_ui_baseline: Vec<u8>,
}

impl RenderRuntime {
    pub(in crate::backend::wayland) fn new() -> Self {
        Self {
            canvas_layer_cache: CanvasLayerCache::new(),
            ui_damage: UiDamageHistory::default(),
            profile_ui_baseline: Vec::new(),
        }
    }

    pub(in crate::backend::wayland::state) fn canvas_layer_cache(&self) -> &CanvasLayerCache {
        &self.canvas_layer_cache
    }

    pub(in crate::backend::wayland::state) fn canvas_layer_cache_mut(
        &mut self,
    ) -> &mut CanvasLayerCache {
        &mut self.canvas_layer_cache
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
