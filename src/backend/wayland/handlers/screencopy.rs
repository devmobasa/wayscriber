// Dispatch handlers for wlr-screencopy objects used by frozen mode.
use log::debug;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{Event as FrameEvent, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::{Event as ManagerEvent, ZwlrScreencopyManagerV1},
};

use super::super::state::WaylandState;

impl Dispatch<ZwlrScreencopyManagerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrScreencopyManagerV1,
        event: ManagerEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        debug!("Screencopy manager event ignored: {:?}", event);
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrScreencopyFrameV1,
        event: FrameEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        state
            .frozen
            .handle_frame_event(event, &mut state.surface, &mut state.input_state);
    }
}
