use super::super::buffer_damage::BufferDamageTracker;
use super::super::*;
use crate::env_vars::{FORCE_INLINE_TOOLBARS_ENV, XDG_ACTIVATION_TOKEN_ENV};

impl WaylandState {
    pub(in crate::backend::wayland) fn new(init: WaylandStateInit) -> std::io::Result<Self> {
        let WaylandStateInit {
            globals,
            config,
            config_store,
            path_resolver,
            runtime_paths,
            logger,
            input_state,
            onboarding,
            palette_recents,
            capture_manager,
            session_options,
            persistence,
            session_catalog,
            runtime_ui,
            runtime_ui_unavailable,
            process_broker,
            runtime_wake,
            tokio_handle,
            exit_after_capture_mode,
            frozen_enabled,
            preferred_output_identity,
            xdg_fullscreen,
            main_surface_uses_overlay_layer,
            pending_freeze_on_start,
            screencopy_manager,
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
        let runtime_options = WaylandRuntimeOptions::from_env();

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
        let force_inline_toolbars = force_inline_toolbars_requested_with_env(
            &config,
            runtime_options.force_inline_toolbars(),
        );
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
        data.toolbar_top_offset = config.ui.toolbar.top_offset;
        data.toolbar_top_offset_y = config.ui.toolbar.top_offset_y;
        data.toolbar_side_offset = config.ui.toolbar.side_offset;
        data.toolbar_side_offset_x = config.ui.toolbar.side_offset_x;
        runtime_options.drag_log(format!(
            "load offsets from config: top_offset=({}, {}), side_offset=({}, {})",
            data.toolbar_top_offset,
            data.toolbar_top_offset_y,
            data.toolbar_side_offset,
            data.toolbar_side_offset_x
        ));
        let zoom_manager = screencopy_manager.clone();
        let ui_animation_interval =
            WaylandState::ui_animation_interval_from_fps(config.performance.ui_animation_fps);
        let theme = crate::ui::theme::Theme::from_mode(config.ui.theme.to_theme_mode());

        let buffer_count = config.performance.buffer_count as usize;
        let clipboard_operation_ids = ClipboardOperationIdSource::new();
        let clipboard_publish_wake = runtime_wake.try_duplicate()?;
        let clipboard_paste_wake = runtime_wake.try_duplicate()?;
        let clipboard_hex_copy_wake = runtime_wake.try_duplicate()?;
        let clipboard_hex_paste_wake = runtime_wake.try_duplicate()?;
        let clipboard_text_copy_wake = runtime_wake.try_duplicate()?;
        let clipboard_text_paste_wake = runtime_wake.try_duplicate()?;
        let frozen_wake = runtime_wake.try_duplicate()?;
        let zoom_wake = runtime_wake.try_duplicate()?;
        let clipboard_publish = ClipboardOperationController::new(clipboard_publish_wake);
        let clipboard_paste = ClipboardOperationController::new(clipboard_paste_wake);
        let clipboard_hex_copy = ClipboardOperationController::new(clipboard_hex_copy_wake);
        let clipboard_hex_paste = ClipboardOperationController::new(clipboard_hex_paste_wake);
        let clipboard_text_copy = ClipboardOperationController::new(clipboard_text_copy_wake);
        let clipboard_text_paste = ClipboardOperationController::new(clipboard_text_paste_wake);
        let session_dialog = super::super::toolbar::SessionFileDialogController::new(
            runtime_wake,
            process_broker.clone(),
            &path_resolver,
        );

        Ok(Self {
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
            toolbar: ToolbarSurfaceManager::new(runtime_options.debug_toolbar_color_logging()),
            data,
            buffer_damage: BufferDamageTracker::new(buffer_count),
            canvas_layer_cache: super::super::canvas_layer::CanvasLayerCache::new(),
            help_overlay_renderer: crate::ui::HelpOverlayRenderer::new(),
            spotlight_dimmed_last_frame: false,
            config,
            config_store,
            path_resolver,
            runtime_paths,
            logger,
            theme,
            runtime_ui,
            session_catalog,
            runtime_ui_unavailable,
            runtime_ui_unavailable_previews: Default::default(),
            process_broker: process_broker.clone(),
            runtime_options,
            input_state,
            clipboard_operation_ids,
            palette_recents,
            clipboard_publish,
            clipboard_paste,
            clipboard_hex_copy,
            clipboard_hex_paste,
            pending_hex_copy: None,
            clipboard_text_copy,
            pending_text_copy: Default::default(),
            clipboard_text_paste,
            pending_text_paste: Default::default(),
            gtk_toolbar: None,
            onboarding,
            ui_animation_next_tick: None,
            ui_animation_interval,
            capture: CaptureState::new(capture_manager),
            frozen: FrozenState::new_with_runtime_wake(screencopy_manager, frozen_wake),
            zoom: ZoomState::new_with_runtime_wake(zoom_manager, zoom_wake),
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
            stylus_tool_types: std::collections::HashMap::new(),
            #[cfg(feature = "tablet-input")]
            stylus_auto_switched_to_eraser: false,
            #[cfg(feature = "tablet-input")]
            stylus_pre_eraser_tool_override: None,
            session: SessionState::new(session_options),
            persistence,
            session_dialog,
            durable_action_finish: None,
            durable_action_retry_at: None,
            tokio_handle,
        })
    }
}

fn startup_activation_token_from_env() -> Option<String> {
    std::env::var(XDG_ACTIVATION_TOKEN_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
