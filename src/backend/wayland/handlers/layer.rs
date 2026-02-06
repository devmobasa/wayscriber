// Responds to layer-shell configure/close events, keeping dimensions in sync with the compositor.
use log::{debug, info, warn};
use smithay_client_toolkit::shell::{
    WaylandSurface,
    wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
};
use wayland_client::{Connection, Proxy, QueueHandle};

use super::super::state::WaylandState;
use crate::session;

impl LayerShellHandler for WaylandState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
        if self.toolbar.is_toolbar_layer(layer) {
            info!("Toolbar surface closed by compositor; hiding toolbar");
            let _ = self.input_state.set_toolbar_visible(false);
            self.toolbar.set_visible(false);
            self.refresh_keyboard_interactivity();
            return;
        }

        info!("Layer surface closed by compositor");
        self.input_state.should_exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if self.toolbar.handle_configure(&configure, layer) {
            log::info!(
                "Toolbar layer configure: size={}x{}, layer={}",
                configure.new_size.0,
                configure.new_size.1,
                layer.wl_surface().id().protocol_id()
            );
            self.input_state.needs_redraw = true;
            return;
        }

        if configure.new_size.0 > 0 && configure.new_size.1 > 0 {
            let prev_dims = (self.surface.width(), self.surface.height());
            debug!(
                "Layer surface configure: new logical size {}x{} (previous {}x{}), scale {}",
                configure.new_size.0,
                configure.new_size.1,
                prev_dims.0,
                prev_dims.1,
                self.surface.scale()
            );
            let size_changed = self
                .surface
                .update_dimensions(configure.new_size.0, configure.new_size.1);

            if size_changed {
                info!("Surface size changed - recreating SlotPool");
                self.buffer_damage.mark_all_full();
            }

            self.input_state
                .update_screen_dimensions(self.surface.width(), self.surface.height());
            let (phys_w, phys_h) = self.surface.physical_dimensions();
            self.frozen
                .handle_resize(phys_w, phys_h, &mut self.input_state);
            self.zoom
                .handle_resize(phys_w, phys_h, &mut self.input_state);

            // Refresh active geometry for portal fallback cropping using latest logical size/scale.
            if let Some(geo) = crate::backend::wayland::frozen_geometry::OutputGeometry::update_from(
                None, // logical position is not available here
                Some((self.surface.width() as i32, self.surface.height() as i32)),
                (self.surface.width(), self.surface.height()),
                self.surface.scale(),
            ) {
                self.frozen.set_active_geometry(Some(geo.clone()));
                self.zoom.set_active_geometry(Some(geo));
            }
        }

        self.surface.set_configured(true);
        self.refresh_active_output_label();
        self.input_state.needs_redraw = true;

        // Mark overlay ready if we already have keyboard focus (configure came after enter)
        if self.has_keyboard_focus() && !self.is_overlay_ready() {
            self.set_overlay_ready(true);
            debug!("Overlay ready for keybinds (from configure)");
        }

        let (phys_w, phys_h) = self.surface.physical_dimensions();
        self.frozen
            .handle_resize(phys_w, phys_h, &mut self.input_state);
        self.zoom
            .handle_resize(phys_w, phys_h, &mut self.input_state);

        // Re-apply toolbar offsets now that we have a configured surface size; avoids clamping to 0
        // on startup before the compositor provides dimensions.
        self.sync_toolbar_visibility(qh);

        // Fallback: on xdg-only environments we might never get surface_enter before configure.
        // Try to load a session snapshot once even without output identity; compositor handler
        // will still reload with a concrete identity if it arrives later.
        if !self.session.is_loaded()
            && let Some(options) = self.session_options_mut()
        {
            let load_result = session::load_snapshot(options);
            let mut load_succeeded = false;
            let current_options = self.session_options().cloned();
            match load_result {
                Ok(Some(snapshot)) => {
                    load_succeeded = true;
                    if let Some(ref opts) = current_options {
                        debug!(
                            "Restoring session (fallback) from {}",
                            opts.session_file_path().display()
                        );
                        session::apply_snapshot(&mut self.input_state, snapshot, opts);
                    }
                }
                Ok(None) => {
                    load_succeeded = true;
                    if let Some(ref opts) = current_options {
                        debug!(
                            "No session data found for {} (fallback load)",
                            opts.session_file_path().display()
                        );
                    }
                }
                Err(err) => {
                    warn!("Fallback session load failed: {}", err);
                }
            }
            // Mark loaded to avoid reapplying on every configure when load succeeded; compositor
            // enter still reloads when a concrete output identity arrives because that changes
            // the identity and triggers a fresh load.
            if load_succeeded {
                self.session.mark_loaded();
            }
            self.input_state.needs_redraw = true;
        }
    }
}
