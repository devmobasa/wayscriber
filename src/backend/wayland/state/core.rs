use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn new(init: WaylandStateInit) -> Self {
        let WaylandStateInit {
            globals,
            config,
            input_state,
            capture_manager,
            session_options,
            tokio_handle,
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
            capture: CaptureState::new(capture_manager),
            frozen: FrozenState::new(screencopy_manager),
            zoom: ZoomState::new(zoom_manager),
            themed_pointer: None,
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

    pub(in crate::backend::wayland) fn current_mouse(&self) -> (i32, i32) {
        (self.data.current_mouse_x, self.data.current_mouse_y)
    }

    pub(in crate::backend::wayland) fn set_current_mouse(&mut self, x: i32, y: i32) {
        self.data.current_mouse_x = x;
        self.data.current_mouse_y = y;
    }

    pub(in crate::backend::wayland) fn overlay_suppressed(&self) -> bool {
        self.data.overlay_suppression != OverlaySuppression::None
    }

    fn apply_overlay_clickthrough(&mut self, clickthrough: bool) {
        if let Some(wl_surface) = self.surface.wl_surface().cloned() {
            set_surface_clickthrough(&self.compositor_state, &wl_surface, clickthrough);
        }
        self.toolbar
            .set_suppressed(&self.compositor_state, clickthrough);
    }

    pub(in crate::backend::wayland) fn enter_overlay_suppression(
        &mut self,
        reason: OverlaySuppression,
    ) {
        if self.data.overlay_suppression != OverlaySuppression::None {
            return;
        }
        self.data.overlay_suppression = reason;
        self.apply_overlay_clickthrough(true);
        self.input_state.needs_redraw = true;
        self.toolbar.mark_dirty();
    }

    pub(in crate::backend::wayland) fn exit_overlay_suppression(
        &mut self,
        reason: OverlaySuppression,
    ) {
        if self.data.overlay_suppression != reason {
            return;
        }
        self.data.overlay_suppression = OverlaySuppression::None;
        self.apply_overlay_clickthrough(false);
        self.input_state.needs_redraw = true;
        self.toolbar.mark_dirty();
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn has_keyboard_focus(&self) -> bool {
        self.data.has_keyboard_focus
    }

    pub(in crate::backend::wayland) fn set_keyboard_focus(&mut self, value: bool) {
        self.data.has_keyboard_focus = value;
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn has_pointer_focus(&self) -> bool {
        self.data.has_pointer_focus
    }

    pub(in crate::backend::wayland) fn set_pointer_focus(&mut self, value: bool) {
        self.data.has_pointer_focus = value;
    }

    pub(in crate::backend::wayland) fn current_seat(&self) -> Option<wl_seat::WlSeat> {
        self.data.current_seat.clone()
    }

    #[allow(dead_code)] // Kept for potential future pointer lock support
    pub(in crate::backend::wayland) fn current_pointer(&self) -> Option<wl_pointer::WlPointer> {
        self.themed_pointer.as_ref().map(|p| p.pointer().clone())
    }

    pub(in crate::backend::wayland) fn current_seat_id(&self) -> Option<u32> {
        self.data
            .current_seat
            .as_ref()
            .map(|seat| seat.id().protocol_id())
    }

    pub(in crate::backend::wayland) fn set_current_seat(&mut self, seat: Option<wl_seat::WlSeat>) {
        self.data.current_seat = seat;
    }

    pub(in crate::backend::wayland) fn last_activation_serial(&self) -> Option<u32> {
        self.data.last_activation_serial
    }

    pub(in crate::backend::wayland) fn set_last_activation_serial(&mut self, serial: Option<u32>) {
        self.data.last_activation_serial = serial;
    }

    pub(in crate::backend::wayland) fn current_keyboard_interactivity(
        &self,
    ) -> Option<KeyboardInteractivity> {
        self.data.current_keyboard_interactivity
    }

    pub(in crate::backend::wayland) fn set_current_keyboard_interactivity(
        &mut self,
        interactivity: Option<KeyboardInteractivity>,
    ) {
        self.data.current_keyboard_interactivity = interactivity;
    }

    pub(in crate::backend::wayland) fn frozen_enabled(&self) -> bool {
        self.data.frozen_enabled
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn set_frozen_enabled(&mut self, value: bool) {
        self.data.frozen_enabled = value;
    }

    pub(in crate::backend::wayland) fn pending_freeze_on_start(&self) -> bool {
        self.data.pending_freeze_on_start
    }

    pub(in crate::backend::wayland) fn set_pending_freeze_on_start(&mut self, value: bool) {
        self.data.pending_freeze_on_start = value;
    }

    pub(in crate::backend::wayland) fn pending_activation_token(&self) -> Option<String> {
        self.data.pending_activation_token.clone()
    }

    pub(in crate::backend::wayland) fn set_pending_activation_token(
        &mut self,
        token: Option<String>,
    ) {
        self.data.pending_activation_token = token;
    }

    pub(in crate::backend::wayland) fn preferred_output_identity(&self) -> Option<&str> {
        self.data.preferred_output_identity.as_deref()
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn set_preferred_output_identity(
        &mut self,
        value: Option<String>,
    ) {
        self.data.preferred_output_identity = value;
    }

    pub(in crate::backend::wayland) fn xdg_fullscreen(&self) -> bool {
        self.data.xdg_fullscreen
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn set_xdg_fullscreen(&mut self, value: bool) {
        self.data.xdg_fullscreen = value;
    }

    pub(in crate::backend::wayland) fn session_options(&self) -> Option<&SessionOptions> {
        self.session.options()
    }

    pub(in crate::backend::wayland) fn session_options_mut(
        &mut self,
    ) -> Option<&mut SessionOptions> {
        self.session.options_mut()
    }

    pub(in crate::backend::wayland) fn preferred_fullscreen_output(
        &self,
    ) -> Option<wl_output::WlOutput> {
        if let Some(preferred) = self.preferred_output_identity()
            && let Some(output) = self.output_state.outputs().find(|output| {
                self.output_identity_for(output)
                    .map(|id| id.eq_ignore_ascii_case(preferred))
                    .unwrap_or(false)
            })
        {
            return Some(output);
        }

        self.surface
            .current_output()
            .or_else(|| self.output_state.outputs().next())
    }

    pub(in crate::backend::wayland) fn output_identity_for(
        &self,
        output: &wl_output::WlOutput,
    ) -> Option<String> {
        let info = self.output_state.info(output)?;

        let mut components: Vec<String> = Vec::new();

        if let Some(name) = info.name.filter(|s| !s.is_empty()) {
            components.push(name);
        }

        if !info.make.is_empty() {
            components.push(info.make);
        }

        if !info.model.is_empty() {
            components.push(info.model);
        }

        if components.is_empty() {
            components.push(format!("id{}", info.id));
        }

        Some(components.join("-"))
    }
}
