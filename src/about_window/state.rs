use log::debug;
use smithay_client_toolkit::seat::pointer::CursorIcon;
use wayland_client::Connection;

use super::{ABOUT_HEIGHT, ABOUT_WIDTH, AboutWindowState};

impl AboutWindowState {
    pub(super) fn new(
        registry_state: super::RegistryState,
        compositor_state: super::CompositorState,
        shm: super::Shm,
        output_state: super::OutputState,
        seat_state: super::SeatState,
        xdg_shell: super::XdgShell,
        window: super::Window,
    ) -> Self {
        Self {
            registry_state,
            compositor_state,
            shm,
            output_state,
            seat_state,
            xdg_shell,
            window,
            pool: None,
            width: ABOUT_WIDTH,
            height: ABOUT_HEIGHT,
            scale: 1,
            configured: false,
            should_exit: false,
            needs_redraw: true,
            link_regions: Vec::new(),
            hover_index: None,
            themed_pointer: None,
        }
    }

    pub(super) fn link_index_at(&self, pos: (f64, f64)) -> Option<usize> {
        self.link_regions.iter().position(|link| link.contains(pos))
    }

    pub(super) fn update_hover(&mut self, pos: (f64, f64)) {
        let next = self.link_index_at(pos);
        if next != self.hover_index {
            self.hover_index = next;
            self.needs_redraw = true;
        }
    }

    pub(super) fn update_cursor(&self, conn: &Connection) {
        if let Some(pointer) = self.themed_pointer.as_ref() {
            let icon = if self.hover_index.is_some() {
                CursorIcon::Pointer
            } else {
                CursorIcon::Default
            };
            if let Err(err) = pointer.set_cursor(conn, icon) {
                debug!("Failed to set cursor icon: {}", err);
            }
        }
    }
}
