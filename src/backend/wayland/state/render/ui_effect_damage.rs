//! Targeted damage regions for transient UI effects.
//!
//! Toasts, feedback flashes, and modal UI updates used to force full-surface
//! damage because they emit no damage of their own. Instead, compute each
//! effect's on-screen footprint before rendering and damage only that, unioned
//! with the footprint from the previous frame so moves, content changes, and
//! disappearance are cleaned up correctly.

use super::super::*;
use super::tool_preview::mouse_tool_preview_damage_rect;
use super::{UiEffect, UiEffectFlags};
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

fn color_picker_effect_rect(
    engine: &crate::ui_text::UiTextEngine,
    input_state: &InputState,
    width: u32,
    height: u32,
) -> Option<Rect> {
    crate::ui::color_picker_popup_visual_geometry_with_engine(engine, input_state, width, height)
        .and_then(|bounds| effect_rect(bounds, width, height))
}

fn chrome_cursor_can_rehit(has_cursor_focus: bool, cursor_blocked_by_toolbar: bool) -> bool {
    has_cursor_focus && !cursor_blocked_by_toolbar
}

/// Push damage covering an effect's previous and current footprint.
#[cfg(test)]
fn push_effect_damage(regions: &mut Vec<Rect>, prev: Option<Rect>, current: Option<Rect>) {
    let mut history = super::runtime::UiDamageHistory::default();
    history.roll(UiEffect::UiToast, prev, &mut Vec::new());
    history.roll(UiEffect::UiToast, current, regions);
}

/// Damage the status HUD and any fallback chrome whose visibility is the
/// inverse of the HUD's effective visibility.
///
/// A `Some`/`None` transition can coincide with a selection or text-mode
/// change, so the old and new fallback badge sets are not derivable from the
/// current input state alone. Conservatively repaint the surface on that rare
/// transition; steady visible layouts still use their targeted footprints.
#[cfg(test)]
fn push_status_hud_damage(
    regions: &mut Vec<Rect>,
    prev: Option<Rect>,
    current: Option<Rect>,
    width: u32,
    height: u32,
) {
    let mut history = super::runtime::UiDamageHistory::default();
    history.roll(UiEffect::StatusHud, prev, &mut Vec::new());
    history.roll_status_hud(
        current,
        Rect::new(
            0,
            0,
            width.min(i32::MAX as u32) as i32,
            height.min(i32::MAX as u32) as i32,
        ),
        regions,
    );
}

impl WaylandState {
    /// Damage regions for transient UI effects (toasts, blocked-action flash,
    /// text-edit entry glow) for the current frame. Also updates the
    /// previous-frame tracking state, so this must be called exactly once per
    /// rendered frame, even on frames that force full damage for other reasons.
    pub(super) fn collect_ui_effect_damage(
        &mut self,
        flags: UiEffectFlags,
        width: u32,
        height: u32,
    ) -> Vec<Rect> {
        let mut regions = Vec::new();

        let toast_rect = if flags.active(UiEffect::UiToast) {
            crate::ui::ui_toast_geometry_with_engine(
                self.render.ui_text(),
                &self.input_state,
                width,
                height,
            )
            .and_then(|bounds| effect_rect(bounds, width, height))
        } else {
            None
        };
        self.render
            .ui_damage_mut()
            .roll(UiEffect::UiToast, toast_rect, &mut regions);

        let preset_rect = if flags.active(UiEffect::PresetToast) {
            crate::ui::preset_toast_geometry_with_engine(
                self.render.ui_text(),
                &self.input_state,
                width,
                height,
            )
            .and_then(|bounds| effect_rect(bounds, width, height))
        } else {
            None
        };
        self.render
            .ui_damage_mut()
            .roll(UiEffect::PresetToast, preset_rect, &mut regions);

        if self
            .render
            .ui_damage_mut()
            .roll_blocked_feedback(flags.blocked_feedback())
        {
            regions.extend(
                crate::ui::blocked_feedback_rects(width, height)
                    .into_iter()
                    .filter_map(|bounds| effect_rect(bounds, width, height)),
            );
        }

        let entry_rect = if flags.active(UiEffect::TextEditEntry) {
            self.text_edit_entry_screen_rect(width, height)
        } else {
            None
        };
        self.render
            .ui_damage_mut()
            .roll(UiEffect::TextEditEntry, entry_rect, &mut regions);

        // The status HUD layout is refreshed here, once per frame and before
        // rendering, so damage geometry, rendering, and pointer hit-testing
        // all read the same cache for the frame.
        let chrome_cursor_focused =
            chrome_cursor_can_rehit(self.has_cursor_focus(), self.cursor_blocked_by_toolbar());
        let status_hud_rect = if flags.active(UiEffect::StatusHud) {
            self.input_state
                .update_status_hud_layout_for_pointer_with_engine(
                    self.render.ui_text(),
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
        let surface = Rect::new(
            0,
            0,
            width.min(i32::MAX as u32) as i32,
            height.min(i32::MAX as u32) as i32,
        );
        self.render
            .ui_damage_mut()
            .roll_status_hud(status_hud_rect, surface, &mut regions);

        // The zoom chip follows the same once-per-frame layout refresh as the
        // status HUD, so damage geometry, rendering, and pointer hit-testing
        // all read the same cache for the frame; the appear → move → disappear
        // union keeps stale pixels cleaned up when the percentage changes.
        let zoom_chip_rect = if flags.active(UiEffect::ZoomChip) {
            self.input_state
                .update_zoom_chip_layout_for_pointer_with_engine(
                    self.render.ui_text(),
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
        self.render
            .ui_damage_mut()
            .roll(UiEffect::ZoomChip, zoom_chip_rect, &mut regions);

        // The input HUD's chip row grows, shrinks, and fades every few frames;
        // the same appear → resize → disappear union keeps the stale chips
        // cleaned up without escalating a keystroke to the full surface.
        let input_hud_rect = if flags.active(UiEffect::InputHud) {
            crate::ui::input_hud_geometry(&self.input_state, width, height)
                .and_then(|bounds| effect_rect(bounds, width, height))
        } else {
            None
        };
        self.render
            .ui_damage_mut()
            .roll(UiEffect::InputHud, input_hud_rect, &mut regions);

        // Opening and closing the palette force full damage because the
        // backdrop dimmer changes. While it remains open, only the panel and
        // optional action tooltip change, so typing and selection no longer
        // fall through to the full-surface empty-damage fallback.
        let command_palette_rect = if flags.active(UiEffect::CommandPalette) {
            crate::ui::command_palette_visual_geometry_with_engine(
                self.render.ui_text(),
                &self.input_state,
                width,
                height,
            )
            .and_then(|bounds| effect_rect(bounds, width, height))
        } else {
            None
        };
        self.render.ui_damage_mut().roll(
            UiEffect::CommandPalette,
            command_palette_rect,
            &mut regions,
        );

        // Like the command palette, the color picker owns a stable full-screen
        // dimmer while open. Opening/closing already forces full damage; while
        // engaged, redraw only its panel and optional action tooltip so hex
        // typing cannot fall through to the full-screen empty-damage fallback.
        let color_picker_rect = flags
            .active(UiEffect::ColorPicker)
            .then(|| {
                color_picker_effect_rect(self.render.ui_text(), &self.input_state, width, height)
            })
            .flatten();
        self.render
            .ui_damage_mut()
            .roll(UiEffect::ColorPicker, color_picker_rect, &mut regions);

        let preview_position = self.stylus_hover_cursor_position().unwrap_or_else(|| {
            let (x, y) = self.pointer.position();
            (x as f64, y as f64)
        });
        let preview_rect = flags
            .active(UiEffect::ToolPreview)
            .then(|| {
                mouse_tool_preview_damage_rect(
                    self.input_state.thickness_for_active_tool(),
                    preview_position,
                    width,
                    height,
                )
            })
            .flatten();
        self.render
            .ui_damage_mut()
            .roll(UiEffect::ToolPreview, preview_rect, &mut regions);

        let measure_badge_rect = flags
            .active(UiEffect::ShapeMeasureBadge)
            .then(|| self.shape_measure_badge_visual(width, height))
            .flatten()
            .and_then(|badge| effect_rect(badge.bounds, width, height));
        self.render.ui_damage_mut().roll(
            UiEffect::ShapeMeasureBadge,
            measure_badge_rect,
            &mut regions,
        );

        // The scan overlay spans its region and, once settled, the outcome card
        // beside it. Both move only when the phase changes, so the previous
        // union is re-emitted to clear the sweep it leaves behind.
        // Visibility, not animation: a static overlay under reduced motion
        // still has to be damaged and cleared even though it asks for no
        // frames of its own, so this keys off the overlay itself.
        let ocr_scan_rect = self.input_state.ocr_scan().and_then(|scan| {
            let outcome = scan
                .result(std::time::Instant::now())
                .map(|(outcome, _)| outcome);
            effect_rect(
                crate::ui::ocr_scan_geometry(
                    self.render.ui_text(),
                    scan.region(),
                    outcome,
                    (width, height),
                ),
                width,
                height,
            )
        });
        self.render
            .ui_damage_mut()
            .roll(UiEffect::OcrScan, ocr_scan_rect, &mut regions);

        let measure_picker_damage = if self.input_state.region_state().purpose()
            == Some(crate::input::state::RegionPurposeTag::Measure)
        {
            let (pointer_x, pointer_y) = self.pointer.position();
            crate::ui::measure_picker_damage(
                self.input_state.region_state().selection(),
                (f64::from(pointer_x), f64::from(pointer_y)),
                (width, height),
            )
        } else {
            Vec::new()
        };
        self.render
            .ui_damage_mut()
            .roll_measure_picker(measure_picker_damage, &mut regions);

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
    fn shape_badge_damage_unions_appear_move_and_disappear_footprints() {
        let first = crate::ui::measure_shape_badge(
            &crate::ui_text::UiTextEngine::default(),
            true,
            (20, 30),
            (100.0, 100.0),
            800,
            600,
        )
        .and_then(|badge| effect_rect(badge.bounds, 800, 600));
        let second = crate::ui::measure_shape_badge(
            &crate::ui_text::UiTextEngine::default(),
            true,
            (200, 300),
            (300.0, 250.0),
            800,
            600,
        )
        .and_then(|badge| effect_rect(badge.bounds, 800, 600));
        let mut damage = Vec::new();

        push_effect_damage(&mut damage, None, first);
        assert_eq!(damage, vec![first.expect("appeared")]);

        damage.clear();
        push_effect_damage(&mut damage, first, second);
        assert_eq!(
            damage,
            vec![first.expect("previous"), second.expect("moved")]
        );

        damage.clear();
        push_effect_damage(&mut damage, second, None);
        assert_eq!(damage, vec![second.expect("disappeared")]);
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

    /// A visible status HUD still follows the same move/resize union contract
    /// as the toasts, so stale pixels are cleaned without a full repaint.
    #[test]
    fn visible_status_hud_relayout_unions_prev_and_current_footprints() {
        let corner = effect_rect((15.0, 900.0, 400.0, 40.0), 1920, 1080);
        let moved = effect_rect((1505.0, 900.0, 400.0, 40.0), 1920, 1080);
        assert!(corner.is_some() && moved.is_some());

        let mut relocate = Vec::new();
        push_status_hud_damage(&mut relocate, corner, moved, 1920, 1080);
        assert_eq!(relocate, vec![corner.unwrap(), moved.unwrap()]);
    }

    #[test]
    fn status_hud_effective_visibility_flip_damages_fallback_chrome() {
        let hud = effect_rect((15.0, 900.0, 400.0, 40.0), 1920, 1080);
        let full = Rect::new(0, 0, 1920, 1080).expect("positive full-surface damage fixture");

        let mut reveal_fallback = Vec::new();
        push_status_hud_damage(&mut reveal_fallback, hud, None, 1920, 1080);
        assert_eq!(reveal_fallback, vec![full]);

        let mut cover_fallback = Vec::new();
        push_status_hud_damage(&mut cover_fallback, None, hud, 1920, 1080);
        assert_eq!(cover_fallback, vec![full]);
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

    /// The input HUD's chip row grows as chips arrive and shrinks as they
    /// expire, then vanishes entirely; each transition must union the old and
    /// new footprints so no stale chip is left on screen.
    #[test]
    fn input_hud_damage_lifecycle_unions_prev_and_current_footprints() {
        // Bottom-center footprints: adding a chip widens the row and moves its
        // left edge, since the row stays centered.
        let one_chip = effect_rect((900.0, 1032.0, 120.0, 36.0), 1920, 1080);
        let two_chips = effect_rect((840.0, 1032.0, 240.0, 36.0), 1920, 1080);
        assert!(one_chip.is_some() && two_chips.is_some());

        let mut appear = Vec::new();
        push_effect_damage(&mut appear, None, one_chip);
        assert_eq!(appear, vec![one_chip.unwrap()]);

        let mut grow = Vec::new();
        push_effect_damage(&mut grow, one_chip, two_chips);
        assert_eq!(grow, vec![one_chip.unwrap(), two_chips.unwrap()]);

        let mut expire = Vec::new();
        push_effect_damage(&mut expire, two_chips, None);
        assert_eq!(expire, vec![two_chips.unwrap()]);
    }

    /// A live HUD's damage stays a small strip rather than escalating a
    /// keystroke to the whole surface.
    #[test]
    fn input_hud_damage_is_a_compact_row_not_the_full_surface() {
        let mut input = crate::input::state::test_support::make_test_input_state();
        input.init_input_hud_from_config(crate::input::state::InputHudSettings::from(
            &crate::config::InputHudConfig {
                enabled: true,
                ..crate::config::InputHudConfig::default()
            },
        ));
        input.note_input_hud_key(crate::input::Key::Escape, crate::input::Modifiers::new());

        let bounds = crate::ui::input_hud_geometry(&input, 1920, 1080).expect("row geometry");
        let damage = effect_rect(bounds, 1920, 1080).expect("row damage");
        assert!(damage.width * damage.height < 1920 * 1080 / 10);
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

        let damage =
            color_picker_effect_rect(&crate::ui_text::UiTextEngine::default(), &input, 1920, 1080)
                .expect("popup damage");

        // Before targeted popup damage, an ordinary key used the renderer's
        // 1920x1080 empty-damage fallback (2,073,600 pixels). Now it is the
        // centered panel plus the standard two-pixel safety margin. Derived
        // from the layout constants rather than hard-coded, so changing the
        // panel's size does not fail this test for the wrong reason.
        const MARGIN: i32 = 2;
        let panel_w = crate::input::state::COLOR_PICKER_POPUP_WIDTH as i32;
        let panel_h = crate::input::state::COLOR_PICKER_POPUP_HEIGHT as i32;
        let expected = Rect::new(
            (1920 - panel_w) / 2 - MARGIN,
            (1080 - panel_h) / 2 - MARGIN,
            panel_w + MARGIN * 2,
            panel_h + MARGIN * 2,
        )
        .expect("panel rect");
        assert_eq!(damage, expected);
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

        let damage =
            color_picker_effect_rect(&crate::ui_text::UiTextEngine::default(), &input, 1920, 1080)
                .expect("tooltip damage");

        assert!(damage.width > 304);
        assert!(damage.width < 1920);
        assert!(damage.height < 1080);
    }
}
