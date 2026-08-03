// Dispatch handlers for ext-image-copy-capture objects used by frozen mode.
use log::debug;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::{
    image_capture_source::v1::client::{
        ext_image_capture_source_v1::{Event as SourceEvent, ExtImageCaptureSourceV1},
        ext_output_image_capture_source_manager_v1::{
            Event as OutputSourceManagerEvent, ExtOutputImageCaptureSourceManagerV1,
        },
    },
    image_copy_capture::v1::client::{
        ext_image_copy_capture_frame_v1::{Event as FrameEvent, ExtImageCopyCaptureFrameV1},
        ext_image_copy_capture_manager_v1::{Event as ManagerEvent, ExtImageCopyCaptureManagerV1},
        ext_image_copy_capture_session_v1::{Event as SessionEvent, ExtImageCopyCaptureSessionV1},
    },
};

use super::super::frozen::FrozenCaptureBackend;
use super::super::state::WaylandState;

impl Dispatch<ExtImageCopyCaptureManagerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ExtImageCopyCaptureManagerV1,
        event: ManagerEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        debug!("Ext-image-copy manager event ignored: {event:?}");
    }
}

impl Dispatch<ExtOutputImageCaptureSourceManagerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ExtOutputImageCaptureSourceManagerV1,
        event: OutputSourceManagerEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        debug!("Ext output capture source manager event ignored: {event:?}");
    }
}

impl Dispatch<ExtImageCaptureSourceV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ExtImageCaptureSourceV1,
        event: SourceEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        debug!("Ext image capture source event ignored: {event:?}");
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ExtImageCopyCaptureSessionV1,
        event: SessionEvent,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let should_fallback = state.frozen.handle_ext_session_event(event, qh);
        if should_fallback {
            state.continue_frozen_capture_after_failure(FrozenCaptureBackend::ExtImageCopy, qh);
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ExtImageCopyCaptureFrameV1,
        event: FrameEvent,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if state
            .frozen
            .handle_ext_frame_event(event, qh, &mut state.input_state)
        {
            state.continue_frozen_capture_after_failure(FrozenCaptureBackend::ExtImageCopy, qh);
        }
    }
}
