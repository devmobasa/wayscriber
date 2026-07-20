use std::time::Instant;

use crate::backend::wayland::toolbar::{ToolbarFocusTarget, hit::HitRegion};

use super::capture::OverlayCaptureBarrier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDragKind {
    Top,
    Side,
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
use wayland_client::protocol::wl_seat;

/// Focus/pointer/toolbar interaction data owned by WaylandState and shared with handlers.
#[derive(Debug, Default)]
pub struct StateData {
    pub(super) has_keyboard_focus: bool,
    pub(super) has_pointer_focus: bool,
    pub(super) current_mouse_x: i32,
    pub(super) current_mouse_y: i32,
    pub(super) board_panning: bool,
    pub(super) board_pan_last_pos: (f64, f64),
    pub(super) board_pan_key_held: bool,
    pub(super) current_seat: Option<wl_seat::WlSeat>,
    pub(super) last_activation_serial: Option<u32>,
    pub(super) pointer_over_toolbar: bool,
    pub(super) toolbar_dragging: bool,
    pub(super) toolbar_drag_preview: bool,
    pub(super) current_keyboard_interactivity:
        Option<smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity>,
    pub(super) toolbar_needs_recreate: bool,
    pub(super) toolbar_layer_shell_missing_logged: bool,
    pub(super) inline_toolbars: bool,
    pub(super) inline_top_hits: Vec<HitRegion>,
    pub(super) inline_side_hits: Vec<HitRegion>,
    pub(super) inline_top_rect: Option<(f64, f64, f64, f64)>,
    pub(super) inline_side_rect: Option<(f64, f64, f64, f64)>,
    pub(super) inline_top_hover: Option<(f64, f64)>,
    pub(super) inline_side_hover: Option<(f64, f64)>,
    pub(super) inline_top_hover_start: Option<Instant>,
    pub(super) inline_side_hover_start: Option<Instant>,
    pub(super) inline_top_focus_index: Option<usize>,
    pub(super) inline_side_focus_index: Option<usize>,
    pub(super) inline_top_focus_id: Option<String>,
    pub(super) inline_side_focus_id: Option<String>,
    pub(super) toolbar_focus_target: Option<ToolbarFocusTarget>,
    pub(super) toolbar_top_offset: f64,
    pub(super) toolbar_top_offset_y: f64,
    pub(super) toolbar_side_offset: f64,
    pub(super) toolbar_side_offset_x: f64,
    pub(super) toolbar_configure_miss_count: u32,
    /// Highest GTK drag sequence numbers drained per bar; echoed in
    /// updates so the GTK side can discard stale offset mirrors.
    pub(super) gtk_top_offset_seq: u64,
    pub(super) gtk_side_offset_seq: u64,
    /// GTK surface currently parked at its drag origin while the main overlay
    /// renders the moving toolbar preview.
    pub(super) gtk_drag_preview: Option<crate::toolbar_gtk::GtkToolbarKind>,
    /// A GTK drag that emitted feedback while a modal was engaged stays
    /// blocked until its matching drag-end feedback arrives.
    pub(super) gtk_top_drag_blocked: bool,
    pub(super) gtk_side_drag_blocked: bool,
    /// Pointer is over the GTK top toolbar window (reported via feedback;
    /// GTK runs on its own connection). Restores the top-strip idle fade.
    pub(super) gtk_top_hover: bool,
    pub(super) last_applied_top_margin: Option<i32>,
    pub(super) last_applied_side_margin: Option<i32>,
    pub(super) last_applied_top_margin_top: Option<i32>,
    pub(super) last_applied_side_margin_left: Option<i32>,
    pub(super) toolbar_move_drag: Option<MoveDrag>,
    pub(super) active_drag_kind: Option<MoveDragKind>,
    pub(super) drag_top_base_x: Option<f64>,
    pub(super) drag_top_base_y: Option<f64>,
    pub(super) toolbar_drag_handoff_at: Option<Instant>,
    pub(super) toolbar_drag_flush_requested: bool,
    pub(super) toolbar_drag_pending_apply: bool,
    pub(super) last_toolbar_drag_apply: Option<Instant>,
    pub(super) pending_activation_token: Option<String>,
    pub(super) startup_activation_token: Option<String>,
    pub(super) pending_freeze_on_start: bool,
    pub(super) frozen_enabled: bool,
    pub(super) has_seen_surface_enter: bool,
    pub(super) preferred_output_identity: Option<String>,
    pub(super) xdg_fullscreen: bool,
    pub(super) xdg_frozen_fullscreen_state: XdgFrozenFullscreenState,
    pub(super) xdg_frozen_fullscreen_requested_at: Option<Instant>,
    pub(super) main_surface_uses_overlay_layer: bool,
    pub(super) overlay_suppression: OverlaySuppression,
    pub(super) overlay_suppression_keyboard_policy: OverlaySuppressionKeyboardPolicy,
    pub(super) overlay_capture_barrier: OverlayCaptureBarrier,
    pub(super) overlay_clickthrough: bool,
    /// True when surface is configured and has keyboard focus; keys are blocked until ready.
    pub(super) overlay_ready: bool,
    /// Suppress the next pointer release after a modal click (e.g., command palette).
    pub(super) suppress_next_release: bool,
    /// True when a left press began inside the main-surface toast.
    pub(super) pending_toast_press: bool,
    /// True when a left press began inside the interactive status HUD.
    pub(super) pending_status_hud_press: bool,
    /// The chip press a left press began (`None` when no chip press is pending;
    /// `Passive` for the passive `NN%` readout / inter-piece gap; `Button(kind)`
    /// for an actionable button). Any pending press keeps its release consumed;
    /// a `Button` release fires only when it lands on the SAME button.
    pub(super) pending_zoom_chip_press: crate::ui::ZoomChipPress,
    /// Suppress overlay exit on focus loss for a short window (e.g., clipboard helpers).
    pub(super) suppress_focus_exit_until: Option<Instant>,
    /// Short guard window after xdg focus loss where compositor close requests are ignored
    /// in stay mode to avoid spurious GNOME close events.
    pub(super) xdg_close_guard_until: Option<Instant>,
    /// Explicit compositor close request received for xdg fallback window.
    pub(super) xdg_explicit_close_requested: bool,
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
    pub(super) prev_tool_preview_damage: Option<crate::util::Rect>,
    /// Idle-fade engine for the top-strip islands; its value is published
    /// on every toolbar snapshot as `top_fade`.
    pub(super) top_strip_fade: crate::ui::toolbar::snapshot::fade::TopStripFade,
    /// Per-session shortcut-coach accumulator (slow-path streak, cooldown, and
    /// per-session cap). Session-only; the across-session cap and learned
    /// suppression live in the persisted onboarding state.
    pub(super) shortcut_coach: super::onboarding::ShortcutCoachSession,
}

impl StateData {
    pub fn new() -> Self {
        Self {
            has_keyboard_focus: false,
            has_pointer_focus: false,
            current_mouse_x: 0,
            current_mouse_y: 0,
            board_panning: false,
            board_pan_last_pos: (0.0, 0.0),
            board_pan_key_held: false,
            current_seat: None,
            last_activation_serial: None,
            pointer_over_toolbar: false,
            toolbar_dragging: false,
            toolbar_drag_preview: false,
            current_keyboard_interactivity: None,
            toolbar_needs_recreate: true,
            toolbar_layer_shell_missing_logged: false,
            inline_toolbars: false,
            inline_top_hits: Vec::new(),
            inline_side_hits: Vec::new(),
            inline_top_rect: None,
            inline_side_rect: None,
            inline_top_hover: None,
            inline_side_hover: None,
            inline_top_hover_start: None,
            inline_side_hover_start: None,
            inline_top_focus_index: None,
            inline_side_focus_index: None,
            inline_top_focus_id: None,
            inline_side_focus_id: None,
            toolbar_focus_target: None,
            toolbar_top_offset: 0.0,
            toolbar_top_offset_y: 0.0,
            toolbar_side_offset: 0.0,
            toolbar_side_offset_x: 0.0,
            toolbar_configure_miss_count: 0,
            gtk_top_offset_seq: 0,
            gtk_side_offset_seq: 0,
            gtk_drag_preview: None,
            gtk_top_drag_blocked: false,
            gtk_top_hover: false,
            gtk_side_drag_blocked: false,
            last_applied_top_margin: None,
            last_applied_side_margin: None,
            last_applied_top_margin_top: None,
            last_applied_side_margin_left: None,
            toolbar_move_drag: None,
            active_drag_kind: None,
            drag_top_base_x: None,
            drag_top_base_y: None,
            toolbar_drag_handoff_at: None,
            toolbar_drag_flush_requested: false,
            toolbar_drag_pending_apply: false,
            last_toolbar_drag_apply: None,
            pending_activation_token: None,
            startup_activation_token: None,
            pending_freeze_on_start: false,
            frozen_enabled: false,
            has_seen_surface_enter: false,
            preferred_output_identity: None,
            xdg_fullscreen: false,
            xdg_frozen_fullscreen_state: XdgFrozenFullscreenState::Inactive,
            xdg_frozen_fullscreen_requested_at: None,
            main_surface_uses_overlay_layer: false,
            overlay_suppression: OverlaySuppression::None,
            overlay_suppression_keyboard_policy: OverlaySuppressionKeyboardPolicy::Release,
            overlay_capture_barrier: OverlayCaptureBarrier::default(),
            overlay_clickthrough: false,
            overlay_ready: false,
            suppress_next_release: false,
            pending_toast_press: false,
            pending_status_hud_press: false,
            pending_zoom_chip_press: crate::ui::ZoomChipPress::None,
            suppress_focus_exit_until: None,
            xdg_close_guard_until: None,
            xdg_explicit_close_requested: false,
            render_profile_ui_baseline: Vec::new(),
            prev_ui_toast_damage: None,
            prev_preset_toast_damage: None,
            blocked_feedback_was_active: false,
            prev_text_edit_entry_damage: None,
            prev_status_hud_damage: None,
            prev_zoom_chip_damage: None,
            prev_tool_preview_damage: None,
            top_strip_fade: crate::ui::toolbar::snapshot::fade::TopStripFade::new(),
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
