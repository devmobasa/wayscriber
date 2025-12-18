use smithay_client_toolkit::seat::relative_pointer::{RelativeMotionEvent, RelativePointerHandler};
use wayland_client::{Connection, QueueHandle, protocol::wl_pointer};
use wayland_protocols::wp::relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1;

use super::super::state::WaylandState;

impl RelativePointerHandler for WaylandState {
    fn relative_pointer_motion(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _relative_pointer: &ZwpRelativePointerV1,
        _pointer: &wl_pointer::WlPointer,
        event: RelativeMotionEvent,
    ) {
        if !self.pointer_lock_active() || !self.is_move_dragging() {
            log::info!(
                "relative motion ignored: lock_active={}, drag_active={}",
                self.pointer_lock_active(),
                self.is_move_dragging()
            );
            return;
        }

        let Some(kind) = self.active_move_drag_kind() else {
            return;
        };

        log::info!(
            "relative drag: kind={:?}, delta=({:.3}, {:.3}), utime={}, offsets=({}, {})/({}, {})",
            kind,
            event.delta.0,
            event.delta.1,
            event.utime,
            self.toolbar_top_offset(),
            self.toolbar_top_offset_y(),
            self.toolbar_side_offset_x(),
            self.toolbar_side_offset()
        );

        self.apply_toolbar_relative_delta(kind, event.delta);
    }
}
