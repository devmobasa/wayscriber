use smithay_client_toolkit::shell::xdg::window::{Window, WindowConfigure, WindowHandler};
use wayland_client::{Connection, QueueHandle};

use super::super::{ABOUT_HEIGHT, ABOUT_WIDTH, AboutWindowState};

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
        let width = configure
            .new_size
            .0
            .map(|w| w.get())
            .unwrap_or(ABOUT_WIDTH)
            .max(1);
        let height = configure
            .new_size
            .1
            .map(|h| h.get())
            .unwrap_or(ABOUT_HEIGHT)
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
