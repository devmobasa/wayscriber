use smithay_client_toolkit::shell::xdg::window::{Window, WindowConfigure, WindowHandler};
use wayland_client::{Connection, QueueHandle};

use super::super::AboutWindowState;

impl WindowHandler for AboutWindowState {
    fn request_close(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _window: &Window) {
        self.should_exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        let (preferred_width, preferred_height) = self.preferred_size();
        let width = configure
            .new_size
            .0
            .map_or(preferred_width, |w| w.get())
            .max(1);
        let height = configure
            .new_size
            .1
            .map_or(preferred_height, |h| h.get())
            .max(1);

        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.pool = None;
        }

        self.configured = true;
        self.needs_redraw = true;
    }
}
