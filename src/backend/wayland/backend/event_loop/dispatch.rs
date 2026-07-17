use std::time::Duration;

use wayland_client::{EventQueue, backend::WaylandError};

use super::super::super::state::WaylandState;
use super::super::helpers::dispatch_with_timeout;
use super::super::runtime_wake::RuntimeWakeSource;
use super::super::signals::OverlaySignalState;
use super::super::tray::process_tray_action;

trait PersistenceWakeDrain {
    fn drain_woken_persistence(&mut self) -> Result<(), anyhow::Error>;
}

impl PersistenceWakeDrain for WaylandState {
    fn drain_woken_persistence(&mut self) -> Result<(), anyhow::Error> {
        super::session_save::drain_persistence_completion(self)
    }
}

fn route_woken_persistence(state: &mut impl PersistenceWakeDrain) {
    if let Err(err) = state.drain_woken_persistence() {
        log::warn!("Failed to apply woken persistence completion: {err}");
    }
}

fn route_woken_sources(
    state: &mut WaylandState,
    signals: &mut OverlaySignalState,
) -> Result<(), anyhow::Error> {
    route_woken_persistence(state);

    if signals.exit_requested() {
        state.input_state.should_exit = true;
    }
    let _signal_hint = signals.take_tray_action_requested();
    if process_tray_action(state) {
        state.sync_overlay_interactivity();
    }
    if let Some(failure) = signals.failure() {
        return Err(anyhow::anyhow!("overlay signal listener failed: {failure}"));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureReadOutcome {
    Readable,
    WouldBlock,
}

trait CaptureDispatchOps {
    fn process_runtime_wake(&mut self) -> Result<(), anyhow::Error>;
    fn dispatch_pending(&mut self) -> Result<(), anyhow::Error>;
    fn flush(&mut self) -> Result<(), anyhow::Error>;
    fn prepare_read(&mut self) -> Result<Option<CaptureReadOutcome>, anyhow::Error>;
}

struct RealCaptureDispatchOps<'a> {
    event_queue: &'a mut EventQueue<WaylandState>,
    state: &'a mut WaylandState,
    runtime_wake: &'a RuntimeWakeSource,
    signals: &'a mut OverlaySignalState,
}

impl CaptureDispatchOps for RealCaptureDispatchOps<'_> {
    fn process_runtime_wake(&mut self) -> Result<(), anyhow::Error> {
        let drain = self
            .runtime_wake
            .drain()
            .map_err(|err| anyhow::anyhow!("failed to drain runtime wake descriptor: {err}"))?;
        if drain.reads > 0 {
            route_woken_sources(self.state, self.signals)?;
        }
        Ok(())
    }

    fn dispatch_pending(&mut self) -> Result<(), anyhow::Error> {
        self.event_queue
            .dispatch_pending(self.state)
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("Wayland event queue error: {}", e))
    }

    fn flush(&mut self) -> Result<(), anyhow::Error> {
        self.event_queue
            .flush()
            .map_err(|e| anyhow::anyhow!("Wayland flush error: {}", e))
    }

    fn prepare_read(&mut self) -> Result<Option<CaptureReadOutcome>, anyhow::Error> {
        let Some(guard) = self.event_queue.prepare_read() else {
            return Ok(None);
        };

        match guard.read() {
            Ok(_) => Ok(Some(CaptureReadOutcome::Readable)),
            Err(WaylandError::Io(err)) if err.kind() == std::io::ErrorKind::WouldBlock => {
                Ok(Some(CaptureReadOutcome::WouldBlock))
            }
            Err(err) => Err(anyhow::anyhow!("Wayland read error: {}", err)),
        }
    }
}

fn dispatch_capture_active(ops: &mut impl CaptureDispatchOps) -> Result<(), anyhow::Error> {
    ops.process_runtime_wake()?;
    ops.dispatch_pending()?;
    ops.flush()?;

    if matches!(ops.prepare_read()?, Some(CaptureReadOutcome::Readable)) {
        ops.dispatch_pending()?;
    }

    Ok(())
}

pub(super) fn dispatch_events(
    event_queue: &mut EventQueue<WaylandState>,
    state: &mut WaylandState,
    runtime_wake: &RuntimeWakeSource,
    signals: &mut OverlaySignalState,
    capture_active: bool,
    animation_timeout: Option<Duration>,
) -> Result<(), anyhow::Error> {
    if capture_active {
        let mut ops = RealCaptureDispatchOps {
            event_queue,
            state,
            runtime_wake,
            signals,
        };
        dispatch_capture_active(&mut ops)
    } else {
        dispatch_with_timeout(
            event_queue,
            state,
            runtime_wake,
            |state| route_woken_sources(state, signals),
            animation_timeout,
        )
        .map_err(|e| anyhow::anyhow!("Wayland event queue error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;
    use std::path::PathBuf;
    use std::time::Instant;

    use super::*;
    use crate::backend::wayland::session::{
        PersistenceCompletion, PersistenceController, PersistenceOperation, SessionState,
    };
    use crate::session::SessionOptions;

    struct FakeCaptureDispatchOps {
        runtime_wake_calls: usize,
        dispatch_calls: usize,
        flush_calls: usize,
        prepare_calls: usize,
        dispatch_error_on_call: Option<usize>,
        flush_error: Option<anyhow::Error>,
        prepare_result: Result<Option<CaptureReadOutcome>, anyhow::Error>,
    }

    impl FakeCaptureDispatchOps {
        fn new(prepare_result: Result<Option<CaptureReadOutcome>, anyhow::Error>) -> Self {
            Self {
                runtime_wake_calls: 0,
                dispatch_calls: 0,
                flush_calls: 0,
                prepare_calls: 0,
                dispatch_error_on_call: None,
                flush_error: None,
                prepare_result,
            }
        }
    }

    impl CaptureDispatchOps for FakeCaptureDispatchOps {
        fn process_runtime_wake(&mut self) -> Result<(), anyhow::Error> {
            self.runtime_wake_calls += 1;
            Ok(())
        }

        fn dispatch_pending(&mut self) -> Result<(), anyhow::Error> {
            self.dispatch_calls += 1;
            if self.dispatch_error_on_call == Some(self.dispatch_calls) {
                return Err(anyhow::anyhow!("dispatch failed"));
            }
            Ok(())
        }

        fn flush(&mut self) -> Result<(), anyhow::Error> {
            self.flush_calls += 1;
            if let Some(err) = self.flush_error.take() {
                return Err(err);
            }
            Ok(())
        }

        fn prepare_read(&mut self) -> Result<Option<CaptureReadOutcome>, anyhow::Error> {
            self.prepare_calls += 1;
            match &self.prepare_result {
                Ok(value) => Ok(*value),
                Err(err) => Err(anyhow::anyhow!(err.to_string())),
            }
        }
    }

    #[test]
    fn capture_dispatch_reads_and_dispatches_again() {
        let mut ops = FakeCaptureDispatchOps::new(Ok(Some(CaptureReadOutcome::Readable)));
        dispatch_capture_active(&mut ops).unwrap();

        assert_eq!(ops.dispatch_calls, 2);
        assert_eq!(ops.flush_calls, 1);
        assert_eq!(ops.prepare_calls, 1);
    }

    #[test]
    fn capture_dispatch_processes_runtime_wake() {
        let mut ops = FakeCaptureDispatchOps::new(Ok(None));

        dispatch_capture_active(&mut ops).unwrap();

        assert_eq!(ops.runtime_wake_calls, 1);
    }

    #[test]
    fn capture_dispatch_would_block_skips_second_dispatch() {
        let mut ops = FakeCaptureDispatchOps::new(Ok(Some(CaptureReadOutcome::WouldBlock)));
        dispatch_capture_active(&mut ops).unwrap();

        assert_eq!(ops.dispatch_calls, 1);
        assert_eq!(ops.flush_calls, 1);
        assert_eq!(ops.prepare_calls, 1);
    }

    #[test]
    fn capture_dispatch_without_prepared_read_dispatches_once() {
        let mut ops = FakeCaptureDispatchOps::new(Ok(None));
        dispatch_capture_active(&mut ops).unwrap();

        assert_eq!(ops.dispatch_calls, 1);
        assert_eq!(ops.flush_calls, 1);
        assert_eq!(ops.prepare_calls, 1);
    }

    #[test]
    fn capture_dispatch_propagates_flush_error() {
        let mut ops = FakeCaptureDispatchOps::new(Ok(None));
        ops.flush_error = Some(anyhow::anyhow!("flush failed"));

        let err = dispatch_capture_active(&mut ops).unwrap_err();
        assert!(err.to_string().contains("flush failed"));
        assert_eq!(ops.dispatch_calls, 1);
        assert_eq!(ops.prepare_calls, 0);
    }

    #[test]
    fn capture_dispatch_propagates_read_error() {
        let mut ops = FakeCaptureDispatchOps::new(Err(anyhow::anyhow!("read failed")));

        let err = dispatch_capture_active(&mut ops).unwrap_err();
        assert!(err.to_string().contains("read failed"));
        assert_eq!(ops.dispatch_calls, 1);
        assert_eq!(ops.flush_calls, 1);
        assert_eq!(ops.prepare_calls, 1);
    }

    #[test]
    fn capture_dispatch_propagates_second_dispatch_error() {
        let mut ops = FakeCaptureDispatchOps::new(Ok(Some(CaptureReadOutcome::Readable)));
        ops.dispatch_error_on_call = Some(2);

        let err = dispatch_capture_active(&mut ops).unwrap_err();
        assert!(err.to_string().contains("dispatch failed"));
        assert_eq!(ops.dispatch_calls, 2);
        assert_eq!(ops.flush_calls, 1);
        assert_eq!(ops.prepare_calls, 1);
    }

    struct WorkerFailureRoute {
        controller: PersistenceController,
        session: SessionState,
        options: SessionOptions,
        drain_calls: usize,
        notification_requests: usize,
        toast_requests: usize,
        apply_calls: usize,
    }

    impl super::super::session_save::PersistenceCompletionRuntime for WorkerFailureRoute {
        fn try_receive_persistence_completion(
            &mut self,
        ) -> Result<Option<PersistenceCompletion>, anyhow::Error> {
            self.controller.try_receive()
        }

        fn apply_persistence_completion(
            &mut self,
            _completion: PersistenceCompletion,
        ) -> Result<(), anyhow::Error> {
            self.apply_calls += 1;
            Ok(())
        }

        fn persistence_session_options(&self) -> Option<SessionOptions> {
            Some(self.options.clone())
        }

        fn persistence_session(&mut self) -> &mut SessionState {
            &mut self.session
        }

        fn show_persistence_worker_failure(&mut self) {
            self.toast_requests += 1;
        }

        fn notify_persistence_worker_failure(&mut self, _err: &anyhow::Error) {
            self.notification_requests += 1;
        }
    }

    impl PersistenceWakeDrain for WorkerFailureRoute {
        fn drain_woken_persistence(&mut self) -> Result<(), anyhow::Error> {
            self.drain_calls += 1;
            super::super::session_save::drain_persistence_completion_for_runtime(self)
        }
    }

    fn wait_for_runtime_wake(runtime_wake: &RuntimeWakeSource) {
        let mut pollfd = libc::pollfd {
            fd: runtime_wake.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        loop {
            // SAFETY: pollfd and the runtime wake descriptor stay valid for this
            // bounded test wait.
            let ready = unsafe { libc::poll(&mut pollfd, 1, 1_000) };
            if ready > 0 {
                assert_ne!(pollfd.revents & libc::POLLIN, 0);
                return;
            }
            if ready == 0 {
                panic!("persistence worker did not wake the production route");
            }
            let err = std::io::Error::last_os_error();
            if err.kind() != std::io::ErrorKind::Interrupted {
                panic!("runtime wake poll failed: {err}");
            }
        }
    }

    #[test]
    fn worker_panic_reaches_production_route_and_notifies_once() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "dispatch-panic");
        options.persist_transparent = true;
        options.autosave_enabled = true;
        options.autosave_failure_backoff = Duration::from_millis(50);

        let started = Instant::now();
        let mut session = SessionState::new(Some(options.clone()));
        session.record_input_dirty(started, true);
        let dirty_window = session.prepare_autosave_submission().unwrap();

        let runtime_wake = RuntimeWakeSource::new().unwrap();
        let mut controller = PersistenceController::start(runtime_wake.handle()).unwrap();
        let request_id = controller
            .try_submit(0, PersistenceOperation::PanicForTest)
            .unwrap();
        session.commit_autosave_submission(request_id, dirty_window);

        wait_for_runtime_wake(&runtime_wake);
        runtime_wake.drain().unwrap();
        let mut route = WorkerFailureRoute {
            controller,
            session,
            options,
            drain_calls: 0,
            notification_requests: 0,
            toast_requests: 0,
            apply_calls: 0,
        };

        route_woken_persistence(&mut route);
        route_woken_persistence(&mut route);

        assert_eq!(route.drain_calls, 2);
        assert_eq!(route.notification_requests, 1);
        assert_eq!(route.toast_requests, 1);
        assert_eq!(route.apply_calls, 0);
        assert!(route.session.is_dirty());
        assert!(!route.controller.is_healthy());
        assert!(route.controller.shutdown(0).is_err());
        assert!(route.controller.is_stopped());
    }
}
