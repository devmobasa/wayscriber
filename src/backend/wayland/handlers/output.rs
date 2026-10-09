// Tracks monitor hotplug/updates so `WaylandState` can respond to geometry changes.
use log::{Level, log, log_enabled, trace};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use wayland_client::{Connection, Proxy, QueueHandle, protocol::wl_output};

use super::super::capture_preflight::{CAPTURE_LOG_TARGET, LogField};
use super::super::state::WaylandState;

pub(super) enum CaptureOutputEvent {
    New,
    Updated,
    Destroyed,
    SurfaceEnter,
    SurfaceLeave,
}

impl CaptureOutputEvent {
    fn phase(&self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Updated => "updated",
            Self::Destroyed => "destroyed",
            Self::SurfaceEnter => "surface-enter",
            Self::SurfaceLeave => "surface-leave",
        }
    }

    fn level(&self) -> Level {
        match self {
            Self::SurfaceEnter | Self::SurfaceLeave => Level::Info,
            Self::New | Self::Updated | Self::Destroyed => Level::Debug,
        }
    }
}

impl OutputHandler for WaylandState {
    fn output_state(&mut self) -> &mut OutputState {
        self.protocol.output_mut()
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.refresh_active_output_label();
        self.refresh_freeze_zoom_geometry();
        self.log_capture_output_event(
            CaptureOutputEvent::New,
            &output,
            self.surface.current_output().as_ref(),
        );
        self.cancel_screen_modals_if_source_changed();
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if self.surface.current_output().as_ref() == Some(&output) {
            self.refresh_active_output_label();
        }
        // Screenshot origin walks every output, so a non-active monitor that
        // is added, moved, scaled, or given logical geometry still has to
        // refresh the active crop.
        self.refresh_freeze_zoom_geometry();
        self.log_capture_output_event(
            CaptureOutputEvent::Updated,
            &output,
            self.surface.current_output().as_ref(),
        );
        self.cancel_screen_modals_if_source_changed();
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let previous_output = self.surface.current_output();
        self.surface.clear_output(&output);
        // SCTK 0.20 calls this before removing the output from OutputState, so
        // a walk of current outputs would still include it. Exclude it here;
        // there is no later callback after the removal.
        self.refresh_surface_output(previous_output.as_ref(), Some(&output));
        self.log_capture_output_event(
            CaptureOutputEvent::Destroyed,
            &output,
            previous_output.as_ref(),
        );
        self.cancel_screen_modals_if_source_changed();
    }
}

impl WaylandState {
    pub(in crate::backend::wayland::handlers) fn log_capture_output_event(
        &self,
        event: CaptureOutputEvent,
        output: &wl_output::WlOutput,
        previous: Option<&wl_output::WlOutput>,
    ) {
        let level = event.level();
        if !log_enabled!(target: CAPTURE_LOG_TARGET, level) {
            return;
        }

        let outputs = self.protocol.output();
        let metadata = outputs.info(output);
        let active = self.surface.current_output();
        let active_metadata = active.as_ref().and_then(|output| outputs.info(output));
        let previous_metadata = previous.and_then(|output| outputs.info(output));
        let phase = event.phase();

        log!(target: CAPTURE_LOG_TARGET, level,
            "capture.output phase={phase} output_object={} output_id={} output_name={} previous_output_object={} previous_output_id={} previous_output_name={} active_output_object={} active_output_id={} active_output_name={} desktop_generation={}",
            output.id().protocol_id(),
            LogField(metadata.as_ref().map(|info| info.id)),
            LogField(metadata.as_ref().and_then(|info| info.name.as_deref())),
            LogField(previous.map(|output| output.id().protocol_id())),
            LogField(previous_metadata.as_ref().map(|info| info.id)),
            LogField(previous_metadata.as_ref().and_then(|info| info.name.as_deref())),
            LogField(active.as_ref().map(|output| output.id().protocol_id())),
            LogField(active_metadata.as_ref().map(|info| info.id)),
            LogField(active_metadata.as_ref().and_then(|info| info.name.as_deref())),
            self.frozen.desktop_layout_generation()
        );
        trace!(target: CAPTURE_LOG_TARGET,
            "capture.output phase={phase} output_object={} metadata={metadata:?} surface_size={}x{} surface_scale={}",
            output.id().protocol_id(),
            self.surface.width(),
            self.surface.height(),
            self.surface.scale()
        );
    }
}
