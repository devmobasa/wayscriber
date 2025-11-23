// Handles xdg-shell window configure/close events for the fallback path when
// layer-shell is unavailable (e.g., GNOME).
use log::{debug, info};
use smithay_client_toolkit::shell::xdg::window::{Window, WindowConfigure, WindowHandler};
use wayland_client::{Connection, QueueHandle};

use super::super::state::WaylandState;

impl WindowHandler for WaylandState {
    fn request_close(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _window: &Window) {
        info!("xdg window close requested by compositor");
        self.input_state.should_exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        let fallback_dimensions = self
            .output_state
            .outputs()
            .next()
            .and_then(|output| self.output_state.info(&output))
            .and_then(|info| {
                if let Some((w, h)) = info.logical_size {
                    Some((w.max(1) as u32, h.max(1) as u32))
                } else {
                    info.modes
                        .iter()
                        .find(|mode| mode.current || mode.preferred)
                        .or_else(|| info.modes.first())
                        .map(|mode| {
                            (
                                mode.dimensions.0.max(1) as u32,
                                mode.dimensions.1.max(1) as u32,
                            )
                        })
                }
            })
            .unwrap_or_else(|| (self.surface.width().max(1), self.surface.height().max(1)));

        let width = configure
            .new_size
            .0
            .map(|w| w.get())
            .unwrap_or(fallback_dimensions.0);
        let height = configure
            .new_size
            .1
            .map(|h| h.get())
            .unwrap_or(fallback_dimensions.1);

        if self.xdg_fullscreen {
            if let Some(output) = self.preferred_fullscreen_output() {
                // Reassert fullscreen on the preferred output every configure in case
                // the compositor picked a different monitor initially.
                window.set_fullscreen(Some(&output));
            } else if !configure.is_fullscreen() {
                window.set_fullscreen(None);
            }
        } else {
            // Keep the window maximized; some compositors may unmaximize on mode switches.
            window.set_maximized();
        }

        if self.surface.update_dimensions(width, height) {
            info!("xdg window configured: {}x{}", width, height);
        } else {
            debug!(
                "xdg window configure acknowledged without size change ({}x{})",
                width, height
            );
        }

        self.surface.set_configured(true);
        self.input_state
            .update_screen_dimensions(self.surface.width(), self.surface.height());
        let (phys_w, phys_h) = self.surface.physical_dimensions();
        self.frozen
            .handle_resize(phys_w, phys_h, &mut self.input_state);

        if let Some(geo) = crate::backend::wayland::frozen_geometry::OutputGeometry::update_from(
            None, // logical position is not available here
            Some((self.surface.width() as i32, self.surface.height() as i32)),
            (self.surface.width(), self.surface.height()),
            self.surface.scale(),
        ) {
            self.frozen.set_active_geometry(Some(geo));
        }

        self.input_state.needs_redraw = true;
    }
}
