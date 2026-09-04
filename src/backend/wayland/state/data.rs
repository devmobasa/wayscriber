use std::time::Instant;

use crate::backend::wayland::acquisition::ScreenAcquisitionRegistry;
use crate::backend::wayland::zoom::ZoomWaiterRegistry;

use super::region_capture::{ActiveScreenRegion, WindowSnapSession};
use super::screen_image::ScreenSourceToken;

use super::capture::OverlayCaptureBarrier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDragKind {
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlaySuppression {
    #[default]
    None,
    Capture,
    DesktopBackdrop,
    ExternalDialog,
    Frozen,
    Zoom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlaySuppressionKeyboardPolicy {
    #[default]
    Release,
    Retain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XdgFrozenFullscreenState {
    #[default]
    Inactive,
    PendingConfigure,
    Active,
}

impl OverlaySuppression {
    pub(in crate::backend::wayland) fn requires_capture_barrier(self) -> bool {
        matches!(
            self,
            Self::Capture | Self::DesktopBackdrop | Self::Frozen | Self::Zoom
        )
    }

    pub(in crate::backend::wayland) fn effective_for_board(
        self,
        board_is_transparent: bool,
    ) -> Self {
        if self == Self::Zoom && !board_is_transparent {
            Self::None
        } else {
            self
        }
    }

    pub(in crate::backend::wayland) fn renders_canvas(self) -> bool {
        !matches!(
            self,
            Self::DesktopBackdrop | Self::ExternalDialog | Self::Frozen | Self::Zoom
        )
    }

    /// Whether pointer-driven previews and editing affordances belong in the
    /// canvas pass. A capture frame retains committed annotations but omits
    /// transient state that is not part of the saved drawing.
    pub(in crate::backend::wayland) fn renders_canvas_transients(self) -> bool {
        self == Self::None
    }

    pub(in crate::backend::wayland) fn renders_ui(self) -> bool {
        self == Self::None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MoveDrag {
    pub kind: MoveDragKind,
    pub last_coord: (f64, f64),
    /// Whether last_coord is in screen coordinates (true) or toolbar-local (false)
    pub coord_is_screen: bool,
}
/// Focus/pointer/toolbar interaction data owned by WaylandState and shared with handlers.
#[derive(Debug, Default)]
pub struct StateData {
    pub(super) toolbar_dragging: bool,
    pub(super) toolbar_drag_preview: bool,
    /// Highest GTK drag sequence numbers drained per bar; echoed in
    /// updates so the GTK side can discard stale offset mirrors.
    pub(super) gtk_top_offset_seq: u64,
    /// GTK surface currently parked at its drag origin while the main overlay
    /// renders the moving toolbar preview.
    pub(super) gtk_drag_preview: Option<crate::toolbar_gtk::GtkToolbarKind>,
    /// Offset corrections accumulated while a persistence barrier freezes a
    /// start-relative GTK drag. They discard fenced motion if the exact same
    /// preview later resumes.
    pub(super) gtk_top_drag_rebase: Option<(f64, f64)>,
    /// A GTK drag that emitted feedback while a modal was engaged stays
    /// blocked until its matching drag-end feedback arrives.
    pub(super) gtk_top_drag_blocked: bool,
    pub(super) toolbar_move_drag: Option<MoveDrag>,
    pub(super) active_drag_kind: Option<MoveDragKind>,
    pub(super) drag_top_base_x: Option<f64>,
    pub(super) drag_top_base_y: Option<f64>,
    pub(super) toolbar_drag_handoff_at: Option<Instant>,
    pub(super) toolbar_drag_flush_requested: bool,
    pub(super) toolbar_drag_pending_apply: bool,
    pub(super) last_toolbar_drag_apply: Option<Instant>,
    pub(super) pending_freeze_on_start: bool,
    pub(super) screen_acquisition: ScreenAcquisitionRegistry,
    pub(super) zoom_waiter: ZoomWaiterRegistry,
    pub(super) active_eyedropper_source: Option<ScreenSourceToken>,
    pub(super) active_screen_region: Option<ActiveScreenRegion>,
    pub(super) window_snap: Option<WindowSnapSession>,
    pub(super) region_review_edits: Option<super::region_capture::RegionReviewEdits>,
    pub(super) next_screen_region_generation: u64,
    pub(super) frozen_enabled: bool,
    pub(super) preferred_output_identity: Option<String>,
    pub(super) xdg_fullscreen: bool,
    pub(super) xdg_frozen_fullscreen_state: XdgFrozenFullscreenState,
    pub(super) xdg_frozen_fullscreen_requested_at: Option<Instant>,
    pub(super) main_surface_uses_overlay_layer: bool,
    pub(super) overlay_suppression: OverlaySuppression,
    pub(super) overlay_suppression_keyboard_policy: OverlaySuppressionKeyboardPolicy,
    pub(super) overlay_capture_barrier: OverlayCaptureBarrier,
    pub(super) overlay_clickthrough: bool,
    /// Reused pre-UI pixel snapshot for render-profile UI-only remapping.
    pub(super) render_profile_ui_baseline: Vec<u8>,
    /// Previous-frame damage bounds for transient UI effects, so partial
    /// redraws cover both the old and new footprint of each effect.
    pub(super) prev_ui_toast_damage: Option<crate::util::Rect>,
    pub(super) prev_preset_toast_damage: Option<crate::util::Rect>,
    pub(super) blocked_feedback_was_active: bool,
    pub(super) prev_text_edit_entry_damage: Option<crate::util::Rect>,
    pub(super) prev_status_hud_damage: Option<crate::util::Rect>,
    pub(super) prev_zoom_chip_damage: Option<crate::util::Rect>,
    pub(super) prev_input_hud_damage: Option<crate::util::Rect>,
    pub(super) prev_command_palette_damage: Option<crate::util::Rect>,
    pub(super) prev_color_picker_damage: Option<crate::util::Rect>,
    pub(super) prev_tool_preview_damage: Option<crate::util::Rect>,
    pub(super) prev_shape_measure_badge_damage: Option<crate::util::Rect>,
    /// Union the OCR scan overlay covered last frame, so its sweep is cleared.
    pub(super) prev_ocr_scan_damage: Option<crate::util::Rect>,
    /// Previous-frame strips for Measure Mode's crosshair, frame, and readout.
    pub(super) prev_measure_picker_damage: Vec<crate::util::Rect>,
    /// Per-session shortcut-coach accumulator (slow-path streak, cooldown, and
    /// per-session cap). Session-only; the across-session cap and learned
    /// suppression live in the persisted onboarding state.
    pub(super) shortcut_coach: super::onboarding::ShortcutCoachSession,
}

impl StateData {
    pub fn new() -> Self {
        Self {
            toolbar_dragging: false,
            toolbar_drag_preview: false,
            gtk_top_offset_seq: 0,
            gtk_drag_preview: None,
            gtk_top_drag_rebase: None,
            gtk_top_drag_blocked: false,
            toolbar_move_drag: None,
            active_drag_kind: None,
            drag_top_base_x: None,
            drag_top_base_y: None,
            toolbar_drag_handoff_at: None,
            toolbar_drag_flush_requested: false,
            toolbar_drag_pending_apply: false,
            last_toolbar_drag_apply: None,
            pending_freeze_on_start: false,
            screen_acquisition: ScreenAcquisitionRegistry::default(),
            zoom_waiter: ZoomWaiterRegistry::default(),
            active_eyedropper_source: None,
            active_screen_region: None,
            window_snap: None,
            region_review_edits: None,
            next_screen_region_generation: 1,
            frozen_enabled: false,
            preferred_output_identity: None,
            xdg_fullscreen: false,
            xdg_frozen_fullscreen_state: XdgFrozenFullscreenState::Inactive,
            xdg_frozen_fullscreen_requested_at: None,
            main_surface_uses_overlay_layer: false,
            overlay_suppression: OverlaySuppression::None,
            overlay_suppression_keyboard_policy: OverlaySuppressionKeyboardPolicy::Release,
            overlay_capture_barrier: OverlayCaptureBarrier::default(),
            overlay_clickthrough: false,
            render_profile_ui_baseline: Vec::new(),
            prev_ui_toast_damage: None,
            prev_preset_toast_damage: None,
            blocked_feedback_was_active: false,
            prev_text_edit_entry_damage: None,
            prev_status_hud_damage: None,
            prev_zoom_chip_damage: None,
            prev_input_hud_damage: None,
            prev_command_palette_damage: None,
            prev_color_picker_damage: None,
            prev_tool_preview_damage: None,
            prev_shape_measure_badge_damage: None,
            prev_ocr_scan_damage: None,
            prev_measure_picker_damage: Vec::new(),
            shortcut_coach: super::onboarding::ShortcutCoachSession::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OverlaySuppression;
    #[test]
    fn desktop_backdrop_suppression_hides_canvas_and_ui() {
        let suppression = OverlaySuppression::DesktopBackdrop.effective_for_board(true);

        assert!(!suppression.renders_canvas());
        assert!(!suppression.renders_ui());
    }

    #[test]
    fn normal_capture_suppression_keeps_canvas_without_ui() {
        let suppression = OverlaySuppression::Capture.effective_for_board(true);

        assert!(suppression.renders_canvas());
        assert!(!suppression.renders_ui());
        assert!(!suppression.renders_canvas_transients());
        assert!(OverlaySuppression::None.renders_canvas_transients());
    }

    #[test]
    fn external_dialog_suppression_hides_canvas_and_ui() {
        let suppression = OverlaySuppression::ExternalDialog.effective_for_board(true);

        assert!(!suppression.renders_canvas());
        assert!(!suppression.renders_ui());
    }

    #[test]
    fn zoom_suppression_only_applies_on_transparent_boards() {
        assert_eq!(
            OverlaySuppression::Zoom.effective_for_board(false),
            OverlaySuppression::None
        );
        assert_eq!(
            OverlaySuppression::Zoom.effective_for_board(true),
            OverlaySuppression::Zoom
        );
    }
}
