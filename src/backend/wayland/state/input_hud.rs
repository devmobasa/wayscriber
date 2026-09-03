//! Runtime ownership of the input HUD's capture source.
//!
//! The HUD's overlay source needs no runtime plumbing — the protocol handlers
//! already report what wayscriber receives. The system source does: it is a
//! reader thread that must start when the HUD turns on in system mode and stop
//! on disable, on a failure, and at overlay exit. This module is the single
//! place that reconciles the two, so the effective source is always a function
//! of `{HUD enabled} x {configured mode} x {probe result}`.

use super::WaylandState;
use crate::config::InputHudMode;
use crate::input::state::{InputHudActiveSource, Toast, ToastPriority};

/// Shown when system mode was requested from a build that cannot serve it.
/// The `input` group hint would mislead here: no permission change makes a
/// binary compiled without the feature read `/dev/input`. Feature builds
/// describe their own failures (see `SystemInputFailure`).
#[cfg(not(feature = "input-monitor"))]
const SYSTEM_CAPTURE_HINT: &str = "System-wide capture is not available in this build \
(compiled without the 'input-monitor' feature).";

/// What the configured mode resolves to for a given build/permission state.
///
/// `auto` degrades silently because it is the default; `system` is an explicit
/// request, so its degradation is announced.
/// Outcome of reconciling the reader thread for a wanted system source.
///
/// Without the `input-monitor` feature there is no reader to reconcile, so
/// only `Failed` is ever produced; the arms stay so both builds share one
/// reconciliation path.
#[cfg_attr(not(feature = "input-monitor"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
enum MonitorStart {
    /// The reader has reported `Ready`; the HUD is on the system source.
    Live,
    /// The reader is spawned but has not proven it can capture yet.
    Pending,
    /// The reader could not be spawned at all; carries the message to show,
    /// because a pipe or thread that could not be created is an OS resource
    /// problem the `input` group has nothing to do with.
    Failed(String),
}

/// System-reader lifecycle and reconciliation latches for the input HUD.
pub(in crate::backend::wayland) struct InputHudRuntime {
    #[cfg(feature = "input-monitor")]
    monitor_wake: crate::backend::wayland::RuntimeWakeHandle,
    #[cfg(feature = "input-monitor")]
    monitor: Option<crate::backend::wayland::input_monitor::InputMonitor>,
    system_warned: bool,
    announce_pending: bool,
    last_request: Option<(bool, InputHudMode)>,
}

impl InputHudRuntime {
    pub(super) fn new(monitor_wake: crate::backend::wayland::RuntimeWakeHandle) -> Self {
        #[cfg(not(feature = "input-monitor"))]
        let _ = monitor_wake;
        Self {
            #[cfg(feature = "input-monitor")]
            monitor_wake,
            #[cfg(feature = "input-monitor")]
            monitor: None,
            system_warned: false,
            announce_pending: false,
            last_request: None,
        }
    }

    fn should_resync(&self, request: (bool, InputHudMode)) -> bool {
        self.last_request != Some(request)
    }

    fn note_request(&mut self, request: (bool, InputHudMode)) {
        self.last_request = Some(request);
    }

    fn request_announce(&mut self) {
        self.announce_pending = true;
    }

    fn take_announce(&mut self) -> bool {
        std::mem::take(&mut self.announce_pending)
    }

    fn clear_announce(&mut self) {
        self.announce_pending = false;
    }

    fn clear_system_warning(&mut self) {
        self.system_warned = false;
    }

    fn note_system_warning(&mut self) -> bool {
        if self.system_warned {
            return false;
        }
        self.system_warned = true;
        true
    }

    #[cfg(feature = "input-monitor")]
    fn start(&mut self) -> MonitorStart {
        if let Some(monitor) = self.monitor.as_ref() {
            return if monitor.is_ready() {
                MonitorStart::Live
            } else {
                MonitorStart::Pending
            };
        }
        match crate::backend::wayland::input_monitor::InputMonitor::start(self.monitor_wake.clone())
        {
            Ok(monitor) => {
                self.monitor = Some(monitor);
                log::info!("Input HUD: waiting for system-wide capture to come up");
                MonitorStart::Pending
            }
            Err(err) => MonitorStart::Failed(
                crate::backend::wayland::input_monitor::SystemInputFailure::StartFailed(
                    err.to_string(),
                )
                .user_message(),
            ),
        }
    }

    #[cfg(not(feature = "input-monitor"))]
    fn start(&mut self) -> MonitorStart {
        MonitorStart::Failed(SYSTEM_CAPTURE_HINT.to_string())
    }

    #[cfg(feature = "input-monitor")]
    fn stop(&mut self) -> bool {
        self.monitor.take().is_some()
    }

    #[cfg(not(feature = "input-monitor"))]
    fn stop(&mut self) -> bool {
        false
    }

    #[cfg(feature = "input-monitor")]
    fn drain(&mut self) -> Vec<crate::backend::wayland::input_monitor::SystemInputEvent> {
        self.monitor
            .as_mut()
            .map(crate::backend::wayland::input_monitor::InputMonitor::drain)
            .unwrap_or_default()
    }

    #[cfg(feature = "input-monitor")]
    fn mark_ready(&mut self) {
        if let Some(monitor) = self.monitor.as_mut() {
            monitor.mark_ready();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResolvedInputHudSource {
    Overlay,
    System,
    /// Overlay, and the user asked for system: warn once on enable.
    OverlayWithWarning,
}

pub(super) fn resolve_input_hud_source(
    mode: InputHudMode,
    system_available: bool,
) -> ResolvedInputHudSource {
    match mode {
        InputHudMode::Overlay => ResolvedInputHudSource::Overlay,
        InputHudMode::Auto => {
            if system_available {
                ResolvedInputHudSource::System
            } else {
                ResolvedInputHudSource::Overlay
            }
        }
        InputHudMode::System => {
            if system_available {
                ResolvedInputHudSource::System
            } else {
                ResolvedInputHudSource::OverlayWithWarning
            }
        }
    }
}

impl WaylandState {
    /// Reconcile the reader thread only when the HUD request actually moved.
    ///
    /// Called once per event-loop pass, after every drain that could have
    /// changed it -- a queued toggle, a runtime-UI rollback, a seed refresh --
    /// so no individual path has to remember to sync. `sync_input_monitor`
    /// itself probes for readable devices, which is why this is a comparison
    /// rather than an unconditional call.
    pub(in crate::backend::wayland) fn sync_input_monitor_if_changed(&mut self) {
        let request = (
            self.input_state.input_hud_enabled(),
            self.input_state.input_hud_configured_mode(),
        );
        if self.input_hud.should_resync(request) || self.input_state.has_input_hud_source_announce()
        {
            self.sync_input_monitor();
        }
    }

    /// Reconcile the reader thread with the HUD's live enabled flag and mode.
    ///
    /// Cheap and idempotent, so every path that can flip the toggle (keyboard
    /// action, command palette, toolbar checkbox, presenter mode) can call it
    /// unconditionally.
    pub(in crate::backend::wayland) fn sync_input_monitor(&mut self) {
        self.input_hud.note_request((
            self.input_state.input_hud_enabled(),
            self.input_state.input_hud_configured_mode(),
        ));
        if self.input_state.take_input_hud_source_announce() {
            self.input_hud.request_announce();
        }
        let wanted = self.input_state.input_hud_enabled().then(|| {
            resolve_input_hud_source(
                self.input_state.input_hud_configured_mode(),
                crate::backend::wayland::input_monitor::system_input_available(),
            )
        });

        // System capture was requested (explicitly or resolved) but is not
        // running after this reconciliation; the message explains why, and
        // stands in for the source announcement.
        let mut system_denied = None;
        // The reader is spawned but has not reported `Ready` yet. The HUD
        // stays on the overlay source (so input keeps being reported) and the
        // announcement waits for the handshake to resolve.
        let mut awaiting_reader = false;
        match wanted {
            Some(ResolvedInputHudSource::System) => match self.input_hud.start() {
                MonitorStart::Live => self.input_hud.clear_system_warning(),
                MonitorStart::Pending => {
                    self.input_hud.clear_system_warning();
                    awaiting_reader = true;
                }
                MonitorStart::Failed(message) => {
                    let _ = self
                        .input_state
                        .set_input_hud_source(InputHudActiveSource::Overlay);
                    system_denied = Some(message);
                }
            },
            Some(ResolvedInputHudSource::OverlayWithWarning) => {
                if self.input_hud.stop() {
                    log::info!("Input HUD: system-wide capture stopped");
                }
                let _ = self
                    .input_state
                    .set_input_hud_source(InputHudActiveSource::Overlay);
                // The probe reported nothing readable, but "no nodes at all"
                // and "nodes I may not open" need different advice, and the
                // reader that would have classified them never starts on this
                // path.
                system_denied = Some(self.system_capture_denied_message());
            }
            Some(ResolvedInputHudSource::Overlay) | None => {
                if self.input_hud.stop() {
                    log::info!("Input HUD: system-wide capture stopped");
                }
                let _ = self
                    .input_state
                    .set_input_hud_source(InputHudActiveSource::Overlay);
                self.input_hud.clear_system_warning();
                // A HUD that is off (or overlay-only by configuration) has
                // nothing pending to announce later.
                if wanted.is_none() {
                    self.input_hud.clear_announce();
                }
            }
        }

        if let Some(message) = system_denied {
            // Not gated on a source *change*: at startup and on enable the
            // active source is already Overlay, and an explicit system
            // request must still explain its fallback.
            self.report_system_capture_denied(&message);
        } else if !awaiting_reader {
            self.announce_input_hud_source_if_pending();
        }
    }

    /// Toast the source the HUD ended up with, if a runtime enable asked for
    /// it. Called once the source is settled — after the reader's `Ready` or
    /// its failure — never while the handshake is still outstanding.
    fn announce_input_hud_source_if_pending(&mut self) {
        if !self.input_hud.take_announce() {
            return;
        }
        if !self.input_state.input_hud_enabled() {
            return;
        }
        let message = match self.input_state.input_hud_active_source() {
            InputHudActiveSource::System => "Input HUD: system-wide input",
            InputHudActiveSource::Overlay => "Input HUD: overlay input only",
        };
        log::info!("{message}");
        self.input_state
            .push_toast(ToastPriority::Info, "input-hud", Toast::info(message));
    }

    /// Why a preflight-denied system request failed, in the user's terms.
    ///
    /// The reader thread classifies its own failures, but the probe path
    /// rejects the request before any reader exists, so the same distinction
    /// is drawn here: unreadable nodes point at the `input` group, an empty
    /// `/dev/input` does not.
    #[cfg(feature = "input-monitor")]
    fn system_capture_denied_message(&self) -> String {
        use crate::backend::wayland::input_monitor::{
            EventNodeAccess, SystemInputFailure, current_seat, event_node_access,
        };

        let seat = current_seat();
        match event_node_access() {
            EventNodeAccess::Unreadable => {
                SystemInputFailure::DevicesUnreadable { seat }.user_message()
            }
            EventNodeAccess::None => SystemInputFailure::NoUsableDevices { seat }.user_message(),
            // `Readable` cannot reach this path (the probe would have allowed
            // system mode); `Unknown` means udev could not attribute devices
            // to the seat. Neither supports naming a cause.
            EventNodeAccess::Unknown | EventNodeAccess::Readable => {
                SystemInputFailure::Unavailable { seat }.user_message()
            }
        }
    }

    #[cfg(not(feature = "input-monitor"))]
    fn system_capture_denied_message(&self) -> String {
        SYSTEM_CAPTURE_HINT.to_string()
    }

    /// Report that system capture is unavailable.
    ///
    /// `auto` is documented to degrade silently — it is the default, and a
    /// machine that simply cannot do system capture must not nag on every
    /// enable — so it only logs, and the pending enable still announces the
    /// overlay source it actually got. An explicit `system` request is a
    /// different contract: it gets a toast, and that toast carries the real
    /// reason (empty seat, unusable layout, read error) instead of always
    /// reciting the `input` group hint.
    fn report_system_capture_denied(&mut self, message: &str) {
        log::warn!("{message}");
        if self.input_state.input_hud_configured_mode() != InputHudMode::System {
            self.announce_input_hud_source_if_pending();
            return;
        }
        // The warning stands in for the source announcement.
        self.input_hud.clear_announce();
        if !self.input_hud.note_system_warning() {
            return;
        }
        self.input_state.push_toast(
            ToastPriority::Info,
            "input-hud",
            Toast::warning(message.to_string()),
        );
    }

    /// Push every chip the reader thread produced since the last wake into the
    /// HUD. A terminal failure tears the monitor down and falls back to the
    /// overlay source with the same guidance the probe path shows.
    #[cfg(feature = "input-monitor")]
    pub(in crate::backend::wayland) fn drain_system_input_events(&mut self) {
        use crate::backend::wayland::input_monitor::SystemInputEvent;

        let events = self.input_hud.drain();
        let mut failure = None;
        let mut became_ready = false;
        for event in events {
            match event {
                SystemInputEvent::Ready => {
                    // The reader proved it can capture: take over reporting.
                    // Switching the source drops any chips the overlay hooks
                    // produced during the handshake, so the row never mixes
                    // the two sources.
                    became_ready = true;
                    let _ = self
                        .input_state
                        .set_input_hud_source(InputHudActiveSource::System);
                    log::info!("Input HUD: system-wide capture started");
                }
                SystemInputEvent::Key {
                    label,
                    bare_modifier,
                } => self
                    .input_state
                    .note_input_hud_system_key(label, bare_modifier),
                SystemInputEvent::Mouse { label } => {
                    self.input_state.note_input_hud_system_mouse(label)
                }
                SystemInputEvent::Scroll { label } => {
                    self.input_state.note_input_hud_system_scroll(label)
                }
                SystemInputEvent::Failed(reason) => {
                    failure = Some(reason);
                    break;
                }
            }
        }
        if became_ready {
            self.input_hud.mark_ready();
        }
        if let Some(reason) = failure {
            if self.input_hud.stop() {
                log::info!("Input HUD: system-wide capture stopped");
            }
            let _ = self
                .input_state
                .set_input_hud_source(InputHudActiveSource::Overlay);
            // A fresh failure always deserves its own report; the latch only
            // stops a following sync from repeating it. This is also where a
            // never-ready reader lands, so the enable that was waiting on the
            // handshake is answered here.
            self.input_hud.clear_system_warning();
            self.report_system_capture_denied(&reason.user_message());
        } else if became_ready {
            // The deferred enable announcement, now that the source is real.
            self.announce_input_hud_source_if_pending();
        }
    }

    #[cfg(not(feature = "input-monitor"))]
    pub(in crate::backend::wayland) fn drain_system_input_events(&mut self) {}

    /// Stop the reader thread at overlay exit.
    pub(in crate::backend::wayland) fn shutdown_input_monitor(&mut self) {
        if self.input_hud.stop() {
            log::info!("Input HUD: system-wide capture stopped");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> InputHudRuntime {
        let wake = crate::backend::wayland::RuntimeWakeSource::new()
            .expect("runtime wake source")
            .handle();
        InputHudRuntime::new(wake)
    }

    #[test]
    fn request_reconciliation_tracks_the_last_settled_request() {
        let mut runtime = runtime();
        let request = (true, InputHudMode::System);

        assert!(runtime.should_resync(request));
        runtime.note_request(request);
        assert!(!runtime.should_resync(request));
        assert!(runtime.should_resync((false, InputHudMode::System)));
    }

    #[test]
    fn announce_and_warning_latches_are_consumed_or_reset_once() {
        let mut runtime = runtime();

        runtime.request_announce();
        assert!(runtime.take_announce());
        assert!(!runtime.take_announce());
        assert!(runtime.note_system_warning());
        assert!(!runtime.note_system_warning());
        runtime.clear_system_warning();
        assert!(runtime.note_system_warning());
    }

    #[test]
    fn stopping_without_a_live_reader_reports_no_transition() {
        assert!(!runtime().stop());
    }

    /// Mode resolution is a pure table: overlay never reads `/dev/input`, auto
    /// degrades silently, and system announces its fallback.
    #[test]
    fn mode_resolution_covers_every_mode_and_probe_result() {
        assert_eq!(
            resolve_input_hud_source(InputHudMode::Overlay, true),
            ResolvedInputHudSource::Overlay
        );
        assert_eq!(
            resolve_input_hud_source(InputHudMode::Overlay, false),
            ResolvedInputHudSource::Overlay
        );
        assert_eq!(
            resolve_input_hud_source(InputHudMode::Auto, true),
            ResolvedInputHudSource::System
        );
        assert_eq!(
            resolve_input_hud_source(InputHudMode::Auto, false),
            ResolvedInputHudSource::Overlay
        );
        assert_eq!(
            resolve_input_hud_source(InputHudMode::System, true),
            ResolvedInputHudSource::System
        );
        assert_eq!(
            resolve_input_hud_source(InputHudMode::System, false),
            ResolvedInputHudSource::OverlayWithWarning
        );
    }
}
