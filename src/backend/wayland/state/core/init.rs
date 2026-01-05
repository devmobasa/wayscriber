use super::super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn new(init: WaylandStateInit) -> Self {
        let WaylandStateInit {
            globals,
            config,
            input_state,
            onboarding,
            capture_manager,
            session_options,
            tokio_handle,
            exit_after_capture_mode,
            frozen_enabled,
            preferred_output_identity,
            xdg_fullscreen,
            pending_freeze_on_start,
            screencopy_manager,
            #[cfg(tablet)]
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

        #[cfg(tablet)]
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
        data.preferred_output_identity = preferred_output_identity;
        data.xdg_fullscreen = xdg_fullscreen;
        let force_inline_toolbars = force_inline_toolbars_requested(&config);
        data.inline_toolbars = layer_shell.is_none() || force_inline_toolbars;
        if force_inline_toolbars {
            info!(
                "Forcing inline toolbars (config/ui.toolbar.force_inline or WAYSCRIBER_FORCE_INLINE_TOOLBARS)"
            );
        }
        data.toolbar_top_offset = config.ui.toolbar.top_offset;
        data.toolbar_top_offset_y = config.ui.toolbar.top_offset_y;
        data.toolbar_side_offset = config.ui.toolbar.side_offset;
        data.toolbar_side_offset_x = config.ui.toolbar.side_offset_x;
        drag_log(format!(
            "load offsets from config: top_offset=({}, {}), side_offset=({}, {})",
            data.toolbar_top_offset,
            data.toolbar_top_offset_y,
            data.toolbar_side_offset,
            data.toolbar_side_offset_x
        ));
        let zoom_manager = screencopy_manager.clone();
        let ui_animation_interval =
            WaylandState::ui_animation_interval_from_fps(config.performance.ui_animation_fps);

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
            config,
            input_state,
            onboarding,
            ui_animation_next_tick: None,
            ui_animation_interval,
            capture: CaptureState::new(capture_manager),
            frozen: FrozenState::new(screencopy_manager),
            zoom: ZoomState::new(zoom_manager),
            exit_after_capture_mode,
            themed_pointer: None,
            current_pointer_shape: None,
            locked_pointer: None,
            relative_pointer: None,
            #[cfg(tablet)]
            tablet_manager,
            #[cfg(tablet)]
            tablet_seats: Vec::new(),
            #[cfg(tablet)]
            tablets: Vec::new(),
            #[cfg(tablet)]
            tablet_tools: Vec::new(),
            #[cfg(tablet)]
            tablet_pads: Vec::new(),
            #[cfg(tablet)]
            tablet_pad_groups: Vec::new(),
            #[cfg(tablet)]
            tablet_pad_rings: Vec::new(),
            #[cfg(tablet)]
            tablet_pad_strips: Vec::new(),
            #[cfg(tablet)]
            tablet_settings,
            #[cfg(tablet)]
            tablet_found_logged: false,
            #[cfg(tablet)]
            stylus_tip_down: false,
            #[cfg(tablet)]
            stylus_on_overlay: false,
            #[cfg(tablet)]
            stylus_on_toolbar: false,
            #[cfg(tablet)]
            stylus_base_thickness: None,
            #[cfg(tablet)]
            stylus_pressure_thickness: None,
            #[cfg(tablet)]
            stylus_surface: None,
            #[cfg(tablet)]
            stylus_last_pos: None,
            #[cfg(tablet)]
            stylus_peak_thickness: None,
            session: SessionState::new(session_options),
            tokio_handle,
        }
    }
}
