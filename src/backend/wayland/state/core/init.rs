use super::super::buffer_damage::BufferDamageTracker;
use super::super::*;
use crate::env_vars::{FORCE_INLINE_TOOLBARS_ENV, XDG_ACTIVATION_TOKEN_ENV};

impl WaylandState {
    pub(in crate::backend::wayland) fn new(init: WaylandStateInit) -> Self {
        let WaylandStateInit {
            globals,
            config,
            input_state,
            onboarding,
            palette_recents,
            capture_manager,
            session_options,
            session_config_failed,
            persistence,
            runtime_ui,
            runtime_ui_unavailable,
            runtime_wake,
            tokio_handle,
            exit_after_capture_mode,
            frozen_enabled,
            preferred_output_identity,
            xdg_fullscreen,
            main_surface_uses_overlay_layer,
            pending_freeze_on_start,
            screencopy_manager,
            ext_image_copy_managers,
            portal_freeze_supported,
            text_input_manager,
            #[cfg(feature = "tablet-input")]
            tablet_manager,
        } = init;
        let WaylandGlobals {
            registry_state,
            compositor_state,
            layer_shell,
            xdg_shell,
            activation,
            shm,
            pointer_constraints_state,
            relative_pointer_state,
            output_state,
            seat_state,
        } = globals;

        #[cfg(feature = "tablet-input")]
        let tablet_settings = {
            TabletSettings {
                enabled: config.tablet.enabled,
                pressure_enabled: config.tablet.pressure_enabled,
                min_thickness: config.tablet.min_thickness,
                max_thickness: config.tablet.max_thickness,
            }
        };

        let mut data = StateData::new();
        data.frozen_enabled = frozen_enabled;
        data.pending_freeze_on_start = pending_freeze_on_start;
        let startup_activation_token = startup_activation_token_from_env();
        if startup_activation_token.is_some() {
            info!("Received startup activation token from launcher environment");
        }
        data.startup_activation_token = startup_activation_token;
        data.preferred_output_identity = preferred_output_identity;
        data.xdg_fullscreen = xdg_fullscreen;
        data.main_surface_uses_overlay_layer = main_surface_uses_overlay_layer;
        let force_inline_toolbars = force_inline_toolbars_requested(&config);
        data.inline_toolbars =
            layer_shell.is_none() || force_inline_toolbars || main_surface_uses_overlay_layer;
        if force_inline_toolbars {
            info!(
                "Forcing inline toolbars (config/ui.toolbar.force_inline or {FORCE_INLINE_TOOLBARS_ENV})"
            );
        }
        if main_surface_uses_overlay_layer {
            info!(
                "Using inline toolbars because the main overlay surface runs above fullscreen windows"
            );
        }
        // Authored offsets are the seeds; a retained runtime override from a
        // committed drag is layered on top. Clamping happens on the first
        // apply against real output geometry, not here, so an override from a
        // now-disconnected monitor degrades instead of being lost.
        let mut positions = crate::backend::wayland::runtime_ui_state::ToolbarPositionSnapshot {
            top: (config.ui.toolbar.top_offset, config.ui.toolbar.top_offset_y),
        };
        if let Some(runtime_ui) = runtime_ui.as_ref() {
            runtime_ui.apply_startup_positions(&mut positions);
        }
        data.toolbar_top_offset = positions.top.0;
        data.toolbar_top_offset_y = positions.top.1;
        drag_log(|| {
            format!(
                "load offsets from config seeds and runtime overrides: top_offset=({}, {})",
                data.toolbar_top_offset, data.toolbar_top_offset_y
            )
        });
        let zoom_manager = screencopy_manager.clone();
        let ui_animation_interval =
            WaylandState::ui_animation_interval_from_fps(config.performance.ui_animation_fps);

        let buffer_count = config.performance.buffer_count as usize;
        let clipboard_operation_ids = ClipboardOperationIdSource::new();
        let clipboard_publish = ClipboardOperationController::new(
            clipboard_operation_ids.clone(),
            runtime_wake.clone(),
        );
        let clipboard_paste = ClipboardOperationController::new(
            clipboard_operation_ids.clone(),
            runtime_wake.clone(),
        );
        let clipboard_hex_copy = ClipboardOperationController::new(
            clipboard_operation_ids.clone(),
            runtime_wake.clone(),
        );
        let clipboard_text_copy = ClipboardOperationController::new(
            clipboard_operation_ids.clone(),
            runtime_wake.clone(),
        );
        let clipboard_text_paste =
            ClipboardOperationController::new(clipboard_operation_ids, runtime_wake.clone());
        let ocr = crate::ocr::OcrController::new(runtime_wake.clone());

        Self {
            registry_state,
            compositor_state,
            layer_shell,
            xdg_shell,
            activation,
            shm,
            pointer_constraints_state,
            relative_pointer_state,
            output_state,
            seat_state,
            surface: SurfaceState::new(),
            toolbar: ToolbarSurfaceManager::new(),
            data,
            buffer_damage: BufferDamageTracker::new(buffer_count),
            canvas_layer_cache: super::super::canvas_layer::CanvasLayerCache::new(),
            spotlight_dimmed_last_frame: false,
            config,
            runtime_ui,
            runtime_ui_unavailable,
            runtime_ui_unavailable_previews: Default::default(),
            input_state,
            palette_recents,
            clipboard_publish,
            clipboard_paste,
            clipboard_hex_copy,
            pending_hex_copy: None,
            clipboard_text_copy,
            pending_text_copy: Default::default(),
            clipboard_text_paste,
            pending_text_paste: Default::default(),
            ocr,
            gtk_toolbar: None,
            onboarding,
            config_edits: super::super::super::config_edits::ConfigEditWorker::new(
                runtime_wake.clone(),
            ),
            ui_animation_next_tick: None,
            ui_animation_interval,
            capture: CaptureState::new(capture_manager),
            frozen: FrozenState::new_with_backends(
                screencopy_manager,
                ext_image_copy_managers,
                portal_freeze_supported,
                runtime_wake.clone(),
            ),
            zoom: ZoomState::new_with_runtime_wake(zoom_manager, runtime_wake.clone()),
            perf: perf::PerfMetrics::from_env(),
            exit_after_capture_mode,
            themed_pointer: None,
            touch: None,
            active_touch: TouchState::default(),
            active_touch_surface: None,
            current_pointer_shape: None,
            locked_pointer: None,
            relative_pointer: None,
            cursor_hidden: false,
            key_repeat_key: None,
            key_repeat_next_tick: None,
            text_input_manager,
            text_input: None,
            text_input_seat: None,
            text_input_focused: false,
            text_input_enabled: false,
            text_input_serial: 0,
            text_input_cursor_update_pending: false,
            text_input_external_change_pending: false,
            text_input_cursor_update_blocked_until: None,
            #[cfg(feature = "tablet-input")]
            tablet_manager,
            #[cfg(feature = "tablet-input")]
            tablet_seats: Vec::new(),
            #[cfg(feature = "tablet-input")]
            tablets: Vec::new(),
            #[cfg(feature = "tablet-input")]
            tablet_tools: Vec::new(),
            #[cfg(feature = "tablet-input")]
            tablet_pads: Vec::new(),
            #[cfg(feature = "tablet-input")]
            tablet_pad_groups: Vec::new(),
            #[cfg(feature = "tablet-input")]
            tablet_pad_rings: Vec::new(),
            #[cfg(feature = "tablet-input")]
            tablet_pad_strips: Vec::new(),
            #[cfg(feature = "tablet-input")]
            tablet_settings,
            #[cfg(feature = "tablet-input")]
            tablet_found_logged: false,
            #[cfg(feature = "tablet-input")]
            stylus_tip_down: false,
            #[cfg(feature = "tablet-input")]
            stylus_on_overlay: false,
            #[cfg(feature = "tablet-input")]
            stylus_on_toolbar: false,
            #[cfg(feature = "tablet-input")]
            stylus_base_thickness: None,
            #[cfg(feature = "tablet-input")]
            stylus_pressure_thickness: None,
            #[cfg(feature = "tablet-input")]
            stylus_surface: None,
            #[cfg(feature = "tablet-input")]
            stylus_last_pos: None,
            #[cfg(feature = "tablet-input")]
            stylus_peak_thickness: None,
            #[cfg(feature = "tablet-input")]
            pending_stylus_frame: crate::backend::wayland::state::PendingStylusFrame::default(),
            #[cfg(feature = "tablet-input")]
            stylus_contact_retired: false,
            #[cfg(feature = "tablet-input")]
            stylus_tool_types: std::collections::HashMap::new(),
            #[cfg(feature = "tablet-input")]
            stylus_auto_switched_to_eraser: false,
            #[cfg(feature = "tablet-input")]
            stylus_pre_eraser_tool_override: None,
            session: SessionState::new(session_options),
            session_config_failed,
            persistence,
            #[cfg(feature = "input-monitor")]
            input_monitor_wake: runtime_wake.clone(),
            #[cfg(feature = "input-monitor")]
            input_monitor: None,
            input_hud_system_warned: false,
            input_hud_announce_pending: false,
            last_input_hud_request: None,
            session_dialog: super::super::toolbar::SessionFileDialogController::new(runtime_wake),
            durable_action_finish: None,
            durable_action_retry_at: None,
            tokio_handle,
        }
    }
}

fn startup_activation_token_from_env() -> Option<String> {
    std::env::var(XDG_ACTIVATION_TOKEN_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
