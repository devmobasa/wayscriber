use crate::backend::wayland::toolbar::{ToolbarFocusTarget, hit::HitRegion};

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
    Frozen,
    Zoom,
    ColorPick,
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
    pub(super) inline_top_focus_index: Option<usize>,
    pub(super) inline_side_focus_index: Option<usize>,
    pub(super) toolbar_focus_target: Option<ToolbarFocusTarget>,
    pub(super) toolbar_top_offset: f64,
    pub(super) toolbar_top_offset_y: f64,
    pub(super) toolbar_side_offset: f64,
    pub(super) toolbar_side_offset_x: f64,
    pub(super) toolbar_configure_miss_count: u32,
    pub(super) last_applied_top_margin: Option<i32>,
    pub(super) last_applied_side_margin: Option<i32>,
    pub(super) last_applied_top_margin_top: Option<i32>,
    pub(super) last_applied_side_margin_left: Option<i32>,
    pub(super) toolbar_move_drag: Option<MoveDrag>,
    pub(super) active_drag_kind: Option<MoveDragKind>,
    pub(super) drag_top_base_x: Option<f64>,
    pub(super) drag_top_base_y: Option<f64>,
    pub(super) pending_activation_token: Option<String>,
    pub(super) pending_freeze_on_start: bool,
    pub(super) frozen_enabled: bool,
    pub(super) preferred_output_identity: Option<String>,
    pub(super) xdg_fullscreen: bool,
    pub(super) overlay_suppression: OverlaySuppression,
    /// True when surface is configured and has keyboard focus; keys are blocked until ready.
    pub(super) overlay_ready: bool,
}

impl StateData {
    pub fn new() -> Self {
        Self {
            has_keyboard_focus: false,
            has_pointer_focus: false,
            current_mouse_x: 0,
            current_mouse_y: 0,
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
            inline_top_focus_index: None,
            inline_side_focus_index: None,
            toolbar_focus_target: None,
            toolbar_top_offset: 0.0,
            toolbar_top_offset_y: 0.0,
            toolbar_side_offset: 0.0,
            toolbar_side_offset_x: 0.0,
            toolbar_configure_miss_count: 0,
            last_applied_top_margin: None,
            last_applied_side_margin: None,
            last_applied_top_margin_top: None,
            last_applied_side_margin_left: None,
            toolbar_move_drag: None,
            active_drag_kind: None,
            drag_top_base_x: None,
            drag_top_base_y: None,
            pending_activation_token: None,
            pending_freeze_on_start: false,
            frozen_enabled: false,
            preferred_output_identity: None,
            xdg_fullscreen: false,
            overlay_suppression: OverlaySuppression::None,
            overlay_ready: false,
        }
    }
}
