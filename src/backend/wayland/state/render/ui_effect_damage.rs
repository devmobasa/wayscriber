//! Targeted damage regions for transient UI effects.
//!
//! Toasts, feedback flashes, and modal UI updates used to force full-surface
//! damage because they emit no damage of their own. Instead, compute each
//! effect's on-screen footprint before rendering and damage only that, unioned
//! with the footprint from the previous frame so moves, content changes, and
//! disappearance are cleaned up correctly.

use super::super::*;
use super::tool_preview::mouse_tool_preview_damage_update;
use crate::util::Rect;

/// Safety margin around effect bounds for anti-aliasing bleed.
const UI_EFFECT_DAMAGE_MARGIN: i32 = 2;
/// Maximum radius of the text-edit entry glow pulse
/// (see `render_text_edit_entry_animation`: 30.0 + progress * 20.0).
const TEXT_EDIT_ENTRY_MAX_RADIUS: i32 = 50;

pub(super) fn effect_rect(bounds: (f64, f64, f64, f64), width: u32, height: u32) -> Option<Rect> {
    let (x, y, w, h) = bounds;
    let min_x = (x.floor() as i32 - UI_EFFECT_DAMAGE_MARGIN).max(0);
    let min_y = (y.floor() as i32 - UI_EFFECT_DAMAGE_MARGIN).max(0);
    let max_x =
        ((x + w).ceil() as i32 + UI_EFFECT_DAMAGE_MARGIN).min(width.min(i32::MAX as u32) as i32);
    let max_y =
        ((y + h).ceil() as i32 + UI_EFFECT_DAMAGE_MARGIN).min(height.min(i32::MAX as u32) as i32);
    Rect::from_min_max(min_x, min_y, max_x, max_y)
}

fn color_picker_effect_rect(input_state: &InputState, width: u32, height: u32) -> Option<Rect> {
    crate::ui::color_picker_popup_visual_geometry(input_state, width, height)
        .and_then(|bounds| effect_rect(bounds, width, height))
}

fn chrome_cursor_can_rehit(has_cursor_focus: bool, cursor_blocked_by_toolbar: bool) -> bool {
    has_cursor_focus && !cursor_blocked_by_toolbar
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
    #[allow(clippy::too_many_arguments)]
    pub(super) fn collect_ui_effect_damage(
        &mut self,
        ui_toast_active: bool,
        preset_feedback_active: bool,
        blocked_feedback_active: bool,
        text_edit_entry_active: bool,
        status_hud_active: bool,
        zoom_chip_active: bool,
        command_palette_active: bool,
        color_picker_active: bool,
        tool_preview_active: bool,
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

        // The status HUD layout is refreshed here, once per frame and before
        // rendering, so damage geometry, rendering, and pointer hit-testing
        // all read the same cache for the frame.
        let chrome_cursor_focused =
            chrome_cursor_can_rehit(self.has_cursor_focus(), self.cursor_blocked_by_toolbar());
        let status_hud_rect = if status_hud_active {
            self.input_state.update_status_hud_layout_for_pointer(
                self.config.ui.status_bar_position,
                &self.config.ui.status_bar_style,
                width,
                height,
                chrome_cursor_focused,
            );
            crate::ui::status_hud_geometry(&self.input_state, width, height)
                .and_then(|bounds| effect_rect(bounds, width, height))
        } else {
            self.input_state.clear_status_hud_layout();
            None
        };
        push_effect_damage(
            &mut regions,
            self.data.prev_status_hud_damage,
            status_hud_rect,
        );
        self.data.prev_status_hud_damage = status_hud_rect;

        // The zoom chip follows the same once-per-frame layout refresh as the
        // status HUD, so damage geometry, rendering, and pointer hit-testing
        // all read the same cache for the frame; the appear → move → disappear
        // union keeps stale pixels cleaned up when the percentage changes.
        let zoom_chip_rect = if zoom_chip_active {
            self.input_state.update_zoom_chip_layout_for_pointer(
                &self.config.ui.status_bar_style,
                width,
                height,
                chrome_cursor_focused,
            );
            crate::ui::zoom_chip_geometry(&self.input_state, width, height)
                .and_then(|bounds| effect_rect(bounds, width, height))
        } else {
            self.input_state.clear_zoom_chip_layout();
            None
        };
        push_effect_damage(
            &mut regions,
            self.data.prev_zoom_chip_damage,
            zoom_chip_rect,
        );
        self.data.prev_zoom_chip_damage = zoom_chip_rect;

        // Opening and closing the palette force full damage because the
        // backdrop dimmer changes. While it remains open, only the panel and
        // optional action tooltip change, so typing and selection no longer
        // fall through to the full-surface empty-damage fallback.
        let command_palette_rect = if command_palette_active {
            crate::ui::command_palette_visual_geometry(&self.input_state, width, height)
                .and_then(|bounds| effect_rect(bounds, width, height))
        } else {
            None
        };
        push_effect_damage(
            &mut regions,
            self.data.prev_command_palette_damage,
            command_palette_rect,
        );
        self.data.prev_command_palette_damage = command_palette_rect;

        // Like the command palette, the color picker owns a stable full-screen
        // dimmer while open. Opening/closing already forces full damage; while
        // engaged, redraw only its panel and optional action tooltip so hex
        // typing cannot fall through to the full-screen empty-damage fallback.
        let color_picker_rect = color_picker_active
            .then(|| color_picker_effect_rect(&self.input_state, width, height))
            .flatten();
        push_effect_damage(
            &mut regions,
            self.data.prev_color_picker_damage,
            color_picker_rect,
        );
        self.data.prev_color_picker_damage = color_picker_rect;

        let preview_position = self.stylus_hover_cursor_position().unwrap_or_else(|| {
            let (x, y) = self.current_mouse();
            (x as f64, y as f64)
        });
        let preview_update = mouse_tool_preview_damage_update(
            self.data.prev_tool_preview_damage,
            tool_preview_active,
            self.input_state.thickness_for_active_tool(),
            preview_position,
            width,
            height,
        );
        regions.extend(preview_update.rects);
        self.data.prev_tool_preview_damage = preview_update.current;

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
    fn stylus_only_cursor_focus_rehits_overlay_chrome() {
        assert!(chrome_cursor_can_rehit(true, false));
        assert!(!chrome_cursor_can_rehit(false, false));
        assert!(!chrome_cursor_can_rehit(true, true));
    }

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

    /// The status HUD follows the same appear → move → disappear union
    /// contract as the toasts: every transition damages both the old and new
    /// footprint so stale pixels are always cleaned up.
    #[test]
    fn status_hud_damage_lifecycle_unions_prev_and_current_footprints() {
        let corner = effect_rect((15.0, 900.0, 400.0, 40.0), 1920, 1080);
        let moved = effect_rect((1505.0, 900.0, 400.0, 40.0), 1920, 1080);
        assert!(corner.is_some() && moved.is_some());

        // Appear: only the new footprint.
        let mut appear = Vec::new();
        push_effect_damage(&mut appear, None, corner);
        assert_eq!(appear, vec![corner.unwrap()]);

        // Move (e.g. status_bar_position change): old + new footprints.
        let mut relocate = Vec::new();
        push_effect_damage(&mut relocate, corner, moved);
        assert_eq!(relocate, vec![corner.unwrap(), moved.unwrap()]);

        // Disappear (bar hidden / UI suppressed): old footprint only.
        let mut vanish = Vec::new();
        push_effect_damage(&mut vanish, moved, None);
        assert_eq!(vanish, vec![moved.unwrap()]);
    }

    /// The bottom-right zoom chip uses the same appear → move → disappear union
    /// as the status HUD/toasts: a percentage change (which resizes the pill
    /// and shifts its left edge, since it stays right-anchored) damages both
    /// the old and new footprint so stale digits are always cleaned up.
    #[test]
    fn zoom_chip_damage_lifecycle_unions_prev_and_current_footprints() {
        // Right-anchored footprints: a wider "250%" pill starts further left
        // than a "100%" pill while both share the same right/bottom edge.
        let narrow = effect_rect((1780.0, 1032.0, 132.0, 40.0), 1920, 1080);
        let wide = effect_rect((1770.0, 1032.0, 142.0, 40.0), 1920, 1080);
        assert!(narrow.is_some() && wide.is_some());

        // Appear (cursor enters overlay / zoom actions enabled): new only.
        let mut appear = Vec::new();
        push_effect_damage(&mut appear, None, narrow);
        assert_eq!(appear, vec![narrow.unwrap()]);

        // Percentage change: old + new footprints.
        let mut relayout = Vec::new();
        push_effect_damage(&mut relayout, narrow, wide);
        assert_eq!(relayout, vec![narrow.unwrap(), wide.unwrap()]);

        // Disappear (cursor leaves / zoom actions disabled): old only.
        let mut vanish = Vec::new();
        push_effect_damage(&mut vanish, wide, None);
        assert_eq!(vanish, vec![wide.unwrap()]);
    }

    /// Palette query changes can resize the panel, and hover tooltips can
    /// extend its footprint. Both old and new bounds must be redrawn without
    /// escalating a keystroke to the full screen.
    #[test]
    fn command_palette_damage_lifecycle_stays_targeted_and_cleans_old_bounds() {
        let compact = effect_rect((550.0, 216.0, 820.0, 300.0), 1920, 1080);
        let expanded = effect_rect((550.0, 216.0, 960.0, 420.0), 1920, 1080);
        let compact = compact.expect("compact palette bounds");
        let expanded = expanded.expect("palette plus tooltip bounds");

        assert!(compact.width < 1920);
        assert!(compact.height < 1080);
        assert!(expanded.width < 1920);
        assert!(expanded.height < 1080);

        let mut query_change = Vec::new();
        push_effect_damage(&mut query_change, Some(compact), Some(expanded));
        assert_eq!(query_change, vec![compact, expanded]);

        let mut close = Vec::new();
        push_effect_damage(&mut close, Some(expanded), None);
        assert_eq!(close, vec![expanded]);
    }

    #[test]
    fn color_picker_typing_damage_is_compact_instead_of_full_screen() {
        let mut input = crate::input::state::test_support::make_test_input_state();
        input.open_color_picker_popup();

        let damage = color_picker_effect_rect(&input, 1920, 1080).expect("popup damage");

        // Before targeted popup damage, an ordinary key used the renderer's
        // 1920x1080 empty-damage fallback (2,073,600 pixels). The 300x340
        // panel plus the standard two-pixel safety margin is 104,576 pixels.
        assert_eq!(damage, Rect::new(808, 368, 304, 344).unwrap());
        assert!(damage.width * damage.height < 1920 * 1080 / 10);
    }

    #[test]
    fn color_picker_damage_includes_the_hovered_action_tooltip() {
        let mut input = crate::input::state::test_support::make_test_input_state();
        input.open_color_picker_popup();
        input.update_color_picker_popup_layout(1920, 1080);
        let layout = input.color_picker_popup_layout().expect("popup layout");
        input.color_picker_popup_set_hover(Some((
            layout.eyedropper_btn_x + layout.action_btn_size / 2.0,
            layout.eyedropper_btn_y + layout.action_btn_size / 2.0,
        )));

        let damage = color_picker_effect_rect(&input, 1920, 1080).expect("tooltip damage");

        assert!(damage.width > 304);
        assert!(damage.width < 1920);
        assert!(damage.height < 1080);
    }
}
