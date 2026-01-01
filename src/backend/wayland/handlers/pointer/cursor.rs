use log::warn;
use smithay_client_toolkit::seat::pointer::CursorIcon;
use wayland_client::Connection;

use super::*;

impl WaylandState {
    pub(super) fn update_pointer_cursor(&mut self, toolbar_hover: bool, conn: &Connection) {
        if let Some(pointer) = self.themed_pointer.as_ref() {
            let icon = if toolbar_hover {
                CursorIcon::Default
            } else {
                CursorIcon::Crosshair
            };
            if self.current_pointer_shape != Some(icon) {
                if let Err(err) = pointer.set_cursor(conn, icon) {
                    warn!("Failed to set cursor icon: {}", err);
                } else {
                    self.current_pointer_shape = Some(icon);
                }
            }
        }
    }
}
