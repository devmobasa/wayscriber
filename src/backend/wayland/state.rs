// Holds the live Wayland protocol state shared by the backend loop and the handler
// submodules; provides rendering, capture routing, and overlay helpers used across them.
use crate::draw::Color;
use anyhow::{Context, Result};
use log::{debug, info, warn};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::{
    activation::{ActivationHandler, ActivationState, RequestData},
    compositor::CompositorState,
    globals::ProvidesBoundGlobal,
    output::OutputState,
    registry::RegistryState,
    seat::{
        SeatState,
        pointer::{PointerData, ThemedPointer},
        pointer_constraints::PointerConstraintsState,
        relative_pointer::RelativePointerState,
    },
    shell::{
        wlr_layer::{KeyboardInteractivity, LayerShell},
        xdg::XdgShell,
    },
    shm::Shm,
};
use std::time::{Duration, Instant};
use std::{collections::HashSet, sync::OnceLock};
use wayland_client::{
    Proxy, QueueHandle,
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};
#[cfg(tablet)]
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_manager_v2::ZwpTabletManagerV2, zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2,
    zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2, zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2,
    zwp_tablet_pad_v2::ZwpTabletPadV2, zwp_tablet_seat_v2::ZwpTabletSeatV2,
    zwp_tablet_tool_v2::ZwpTabletToolV2, zwp_tablet_v2::ZwpTabletV2,
};
use wayland_protocols::wp::{
    pointer_constraints::zv1::client::{
        zwp_locked_pointer_v1::ZwpLockedPointerV1, zwp_pointer_constraints_v1,
    },
    relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1,
};

#[cfg(tablet)]
use crate::input::tablet::TabletSettings;
use crate::{
    backend::ExitAfterCaptureMode,
    capture::{
        CaptureDestination, CaptureManager,
        file::{FileSaveConfig, expand_tilde},
        types::CaptureType,
    },
    config::{Action, ColorSpec, Config},
    input::{BoardMode, DrawingState, EraserMode, InputState, Tool, ZoomAction},
    notification,
    session::SessionOptions,
    ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot},
    util::Rect,
};

use self::data::{MoveDrag, StateData};
pub use self::data::{MoveDragKind, OverlaySuppression};
use super::{
    capture::CaptureState,
    frozen::FrozenState,
    overlay_passthrough::set_surface_clickthrough,
    session::SessionState,
    surface::SurfaceState,
    toolbar::{
        ToolbarSurfaceManager,
        hit::{drag_intent_for_hit, intent_for_hit},
        layout::{side_size, top_size},
        render::{render_side_palette, render_top_strip},
    },
    toolbar_intent::intent_to_event,
    zoom::ZoomState,
};

mod activation;
mod capture;
mod core;
mod data;
mod render;
mod toolbar;
mod zoom;

#[cfg(test)]
mod tests;

type ScreencopyManager = wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

pub(in crate::backend::wayland) struct WaylandGlobals {
    pub registry_state: RegistryState,
    pub compositor_state: CompositorState,
    pub layer_shell: Option<LayerShell>,
    pub xdg_shell: Option<XdgShell>,
    pub activation: Option<ActivationState>,
    pub shm: Shm,
    pub pointer_constraints_state: PointerConstraintsState,
    pub relative_pointer_state: RelativePointerState,
    pub output_state: OutputState,
    pub seat_state: SeatState,
}

pub(in crate::backend::wayland) struct WaylandStateInit {
    pub globals: WaylandGlobals,
    pub config: Config,
    pub input_state: InputState,
    pub capture_manager: CaptureManager,
    pub session_options: Option<SessionOptions>,
    pub tokio_handle: tokio::runtime::Handle,
    pub exit_after_capture_mode: ExitAfterCaptureMode,
    pub frozen_enabled: bool,
    pub preferred_output_identity: Option<String>,
    pub xdg_fullscreen: bool,
    pub pending_freeze_on_start: bool,
    pub screencopy_manager: Option<ScreencopyManager>,
    #[cfg(tablet)]
    pub tablet_manager: Option<ZwpTabletManagerV2>,
}

/// Internal Wayland state shared across modules.
pub(super) struct WaylandState {
    // Wayland protocol objects
    pub(super) registry_state: RegistryState,
    pub(super) compositor_state: CompositorState,
    pub(super) layer_shell: Option<LayerShell>,
    pub(super) xdg_shell: Option<XdgShell>,
    pub(super) activation: Option<ActivationState>,
    pub(super) shm: Shm,
    #[allow(dead_code)] // Kept for potential future pointer lock support
    pub(super) pointer_constraints_state: PointerConstraintsState,
    #[allow(dead_code)] // Kept for potential future pointer lock support
    pub(super) relative_pointer_state: RelativePointerState,
    pub(super) output_state: OutputState,
    pub(super) seat_state: SeatState,

    // Surface and buffer management
    pub(super) surface: SurfaceState,
    pub(super) toolbar: ToolbarSurfaceManager,
    data: StateData,

    // Configuration
    pub(super) config: Config,

    // Input state
    pub(super) input_state: InputState,
    // Next scheduled tick for UI animations (toasts/highlights/preset feedback).
    pub(super) ui_animation_next_tick: Option<Instant>,
    // Animation interval; None means uncapped (render every frame while active).
    pub(super) ui_animation_interval: Option<Duration>,

    // Capture manager
    pub(super) capture: CaptureState,
    pub(super) frozen: FrozenState,
    pub(super) zoom: ZoomState,

    // Overlay behavior
    pub(super) exit_after_capture_mode: ExitAfterCaptureMode,

    // Pointer cursor
    pub(super) themed_pointer: Option<ThemedPointer<PointerData>>,
    pub(super) locked_pointer: Option<ZwpLockedPointerV1>,
    pub(super) relative_pointer: Option<ZwpRelativePointerV1>,

    // Tablet / stylus (feature-gated)
    #[cfg(tablet)]
    pub(super) tablet_manager: Option<ZwpTabletManagerV2>,
    #[cfg(tablet)]
    pub(super) tablet_seats: Vec<ZwpTabletSeatV2>,
    #[cfg(tablet)]
    pub(super) tablets: Vec<ZwpTabletV2>,
    #[cfg(tablet)]
    pub(super) tablet_tools: Vec<ZwpTabletToolV2>,
    #[cfg(tablet)]
    pub(super) tablet_pads: Vec<ZwpTabletPadV2>,
    #[cfg(tablet)]
    pub(super) tablet_pad_groups: Vec<ZwpTabletPadGroupV2>,
    #[cfg(tablet)]
    pub(super) tablet_pad_rings: Vec<ZwpTabletPadRingV2>,
    #[cfg(tablet)]
    pub(super) tablet_pad_strips: Vec<ZwpTabletPadStripV2>,
    #[cfg(tablet)]
    pub(super) tablet_settings: TabletSettings,
    #[cfg(tablet)]
    pub(super) tablet_found_logged: bool,
    #[cfg(tablet)]
    pub(super) stylus_tip_down: bool,
    #[cfg(tablet)]
    pub(super) stylus_on_overlay: bool,
    #[cfg(tablet)]
    pub(super) stylus_on_toolbar: bool,
    #[cfg(tablet)]
    pub(super) stylus_base_thickness: Option<f64>,
    #[cfg(tablet)]
    pub(super) stylus_pressure_thickness: Option<f64>,
    #[cfg(tablet)]
    pub(super) stylus_surface: Option<wl_surface::WlSurface>,
    #[cfg(tablet)]
    pub(super) stylus_last_pos: Option<(f64, f64)>,
    #[cfg(tablet)]
    pub(super) stylus_peak_thickness: Option<f64>,

    // Session persistence
    pub(super) session: SessionState,

    // Tokio runtime handle for async operations
    pub(super) tokio_handle: tokio::runtime::Handle,
}

impl WaylandState {
    fn ui_animation_interval_from_fps(fps: u32) -> Option<Duration> {
        if fps == 0 {
            None
        } else {
            Some(Duration::from_secs_f64(1.0 / fps as f64))
        }
    }

    const TOP_MARGIN_RIGHT: f64 = 12.0;
    const TOP_BASE_MARGIN_TOP: f64 = 12.0;
    const TOP_MARGIN_BOTTOM: f64 = 0.0;
    const SIDE_BASE_MARGIN_TOP: f64 = 24.0;
    const SIDE_MARGIN_BOTTOM: f64 = 24.0;
    const SIDE_BASE_MARGIN_LEFT: f64 = 24.0;
    const SIDE_MARGIN_RIGHT: f64 = 0.0;
    const INLINE_TOP_Y: f64 = 16.0;
    const INLINE_SIDE_X: f64 = 24.0;
    const TOOLBAR_CONFIGURE_FAIL_THRESHOLD: u32 = 180;
    const INLINE_TOP_PUSH: f64 = 16.0;
    const ZOOM_STEP_KEY: f64 = 1.2;
    const ZOOM_STEP_SCROLL: f64 = 1.1;
    pub(super) const ZOOM_PAN_STEP: f64 = 32.0;
    pub(super) const ZOOM_PAN_STEP_LARGE: f64 = 96.0;
}

impl WaylandState {
    pub(super) fn update_ui_animation_tick(&mut self, now: Instant, active: bool) {
        if !active {
            self.ui_animation_next_tick = None;
            return;
        }
        if let Some(interval) = self.ui_animation_interval {
            self.ui_animation_next_tick = Some(now + interval);
        } else {
            self.ui_animation_next_tick = None;
        }
    }

    pub(super) fn ui_animation_timeout(&self, now: Instant) -> Option<Duration> {
        if self.ui_animation_interval.is_none() {
            return None;
        }
        self.ui_animation_next_tick
            .map(|next| next.saturating_duration_since(now))
    }

    pub(super) fn ui_animation_due(&self, now: Instant) -> bool {
        if self.ui_animation_interval.is_none() {
            return false;
        }
        self.ui_animation_next_tick
            .is_some_and(|next| now >= next)
    }
}

#[allow(dead_code)]
fn resolve_damage_regions(width: i32, height: i32, mut regions: Vec<Rect>) -> Vec<Rect> {
    regions.retain(Rect::is_valid);

    if regions.is_empty()
        && width > 0
        && height > 0
        && let Some(full) = Rect::new(0, 0, width, height)
    {
        regions.push(full);
    }

    regions
}

#[allow(dead_code)]
fn scale_damage_regions(regions: Vec<Rect>, scale: i32) -> Vec<Rect> {
    if scale <= 1 {
        return regions;
    }

    regions
        .into_iter()
        .filter_map(|r| {
            let x = r.x.saturating_mul(scale);
            let y = r.y.saturating_mul(scale);
            let w = r.width.saturating_mul(scale);
            let h = r.height.saturating_mul(scale);

            Rect::new(x, y, w, h)
        })
        .collect()
}

fn damage_summary(regions: &[Rect]) -> String {
    if regions.is_empty() {
        return "[]".to_string();
    }

    let mut parts = Vec::with_capacity(regions.len());
    for r in regions.iter().take(5) {
        parts.push(format!("({},{}) {}x{}", r.x, r.y, r.width, r.height));
    }
    if regions.len() > 5 {
        parts.push(format!("... +{} more", regions.len() - 5));
    }
    parts.join(", ")
}

fn parse_boolish_env(raw: &str) -> bool {
    let v = raw.to_ascii_lowercase();
    !(v.is_empty() || v == "0" || v == "false" || v == "off")
}

fn parse_debug_damage_env(raw: &str) -> bool {
    parse_boolish_env(raw)
}

fn debug_damage_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        parse_debug_damage_env(&std::env::var("WAYSCRIBER_DEBUG_DAMAGE").unwrap_or_default())
    })
}

pub(super) fn surface_id(surface: &wl_surface::WlSurface) -> u32 {
    surface.id().protocol_id()
}

pub(super) fn debug_toolbar_drag_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        parse_boolish_env(&std::env::var("WAYSCRIBER_DEBUG_TOOLBAR_DRAG").unwrap_or_default())
    })
}

fn drag_log(message: impl AsRef<str>) {
    if debug_toolbar_drag_logging_enabled() {
        log::info!("{}", message.as_ref());
    }
}

fn force_inline_env_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        parse_boolish_env(&std::env::var("WAYSCRIBER_FORCE_INLINE_TOOLBARS").unwrap_or_default())
    })
}

fn force_inline_toolbars_requested_with_env(config: &Config, env_force_inline: bool) -> bool {
    config.ui.toolbar.force_inline || env_force_inline
}

fn force_inline_toolbars_requested(config: &Config) -> bool {
    force_inline_toolbars_requested_with_env(config, force_inline_env_enabled())
}
