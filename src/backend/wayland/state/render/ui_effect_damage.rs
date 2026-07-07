//! Targeted damage regions for transient UI effects.
//!
//! Toasts and feedback flashes used to force full-surface damage for every
//! animation tick because they emit no damage of their own. Instead, compute
//! each effect's on-screen footprint before rendering and damage only that,
//! unioned with the footprint from the previous frame so moves, content
//! changes, and disappearance are cleaned up correctly.

use super::super::*;
use crate::util::Rect;

/// Safety margin around effect bounds for anti-aliasing bleed.
const UI_EFFECT_DAMAGE_MARGIN: i32 = 2;
/// Maximum radius of the text-edit entry glow pulse
/// (see `render_text_edit_entry_animation`: 30.0 + progress * 20.0).
const TEXT_EDIT_ENTRY_MAX_RADIUS: i32 = 50;

fn effect_rect(bounds: (f64, f64, f64, f64), width: u32, height: u32) -> Option<Rect> {
    let (x, y, w, h) = bounds;
    let min_x = (x.floor() as i32 - UI_EFFECT_DAMAGE_MARGIN).max(0);
    let min_y = (y.floor() as i32 - UI_EFFECT_DAMAGE_MARGIN).max(0);
    let max_x =
        ((x + w).ceil() as i32 + UI_EFFECT_DAMAGE_MARGIN).min(width.min(i32::MAX as u32) as i32);
    let max_y =
        ((y + h).ceil() as i32 + UI_EFFECT_DAMAGE_MARGIN).min(height.min(i32::MAX as u32) as i32);
    Rect::from_min_max(min_x, min_y, max_x, max_y)
}

/// Push damage covering an effect's previous and current footprint.
fn push_effect_damage(regions: &mut Vec<Rect>, prev: Option<Rect>, current: Option<Rect>) {
    match (prev, current) {
        (Some(prev), Some(current)) if prev == current => regions.push(current),
        (prev, current) => {
            if let Some(prev) = prev {
                regions.push(prev);
            }
            if let Some(current) = current {
                regions.push(current);
            }
        }
    }
}

impl WaylandState {
    /// Damage regions for transient UI effects (toasts, blocked-action flash,
    /// text-edit entry glow) for the current frame. Also updates the
    /// previous-frame tracking state, so this must be called exactly once per
    /// rendered frame, even on frames that force full damage for other reasons.
    pub(super) fn collect_ui_effect_damage(
        &mut self,
        ui_toast_active: bool,
        preset_feedback_active: bool,
        blocked_feedback_active: bool,
        text_edit_entry_active: bool,
        width: u32,
        height: u32,
    ) -> Vec<Rect> {
        let mut regions = Vec::new();

        let toast_rect = if ui_toast_active {
            crate::ui::ui_toast_geometry(&self.input_state, width, height)
                .and_then(|bounds| effect_rect(bounds, width, height))
        } else {
            None
        };
        push_effect_damage(&mut regions, self.data.prev_ui_toast_damage, toast_rect);
        self.data.prev_ui_toast_damage = toast_rect;

        let preset_rect = if preset_feedback_active {
            crate::ui::preset_toast_geometry(&self.input_state, width, height)
                .and_then(|bounds| effect_rect(bounds, width, height))
        } else {
            None
        };
        push_effect_damage(
            &mut regions,
            self.data.prev_preset_toast_damage,
            preset_rect,
        );
        self.data.prev_preset_toast_damage = preset_rect;

        if blocked_feedback_active || self.data.blocked_feedback_was_active {
            regions.extend(
                crate::ui::blocked_feedback_rects(width, height)
                    .into_iter()
                    .filter_map(|bounds| effect_rect(bounds, width, height)),
            );
        }
        self.data.blocked_feedback_was_active = blocked_feedback_active;

        let entry_rect = if text_edit_entry_active {
            self.text_edit_entry_screen_rect(width, height)
        } else {
            None
        };
        push_effect_damage(
            &mut regions,
            self.data.prev_text_edit_entry_damage,
            entry_rect,
        );
        self.data.prev_text_edit_entry_damage = entry_rect;

        regions
    }

    /// Screen-space rect covering the text-edit entry glow at its maximum extent.
    fn text_edit_entry_screen_rect(&self, width: u32, height: u32) -> Option<Rect> {
        let DrawingState::TextInput { x, y, .. } = &self.input_state.state else {
            return None;
        };
        // The glow renders in world coordinates inside the canvas transform;
        // translate to screen space (zoom forces full damage, so only the
        // pan offset matters here).
        let (origin_x, origin_y) = self.canvas_view_origin();
        let screen_x = *x as f64 - origin_x;
        let screen_y = *y as f64 - origin_y;
        let radius = TEXT_EDIT_ENTRY_MAX_RADIUS as f64;
        effect_rect(
            (
                screen_x - radius,
                screen_y - radius,
                radius * 2.0,
                radius * 2.0,
            ),
            width,
            height,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_rect_expands_by_margin_and_clamps_to_screen() {
        let rect = effect_rect((10.0, 20.0, 30.0, 40.0), 1920, 1080).expect("valid rect");
        assert_eq!(rect, Rect::new(8, 18, 34, 44).unwrap());

        let clamped = effect_rect((-5.0, -5.0, 20.0, 20.0), 100, 100).expect("clamped rect");
        assert_eq!(clamped.x, 0);
        assert_eq!(clamped.y, 0);
    }

    #[test]
    fn push_effect_damage_dedupes_identical_bounds() {
        let rect = Rect::new(5, 5, 10, 10).unwrap();
        let mut regions = Vec::new();
        push_effect_damage(&mut regions, Some(rect), Some(rect));
        assert_eq!(regions, vec![rect]);
    }

    #[test]
    fn push_effect_damage_covers_old_and_new_bounds() {
        let prev = Rect::new(0, 0, 10, 10).unwrap();
        let current = Rect::new(50, 50, 10, 10).unwrap();
        let mut regions = Vec::new();
        push_effect_damage(&mut regions, Some(prev), Some(current));
        assert_eq!(regions, vec![prev, current]);

        let mut cleanup = Vec::new();
        push_effect_damage(&mut cleanup, Some(prev), None);
        assert_eq!(cleanup, vec![prev]);
    }
}
