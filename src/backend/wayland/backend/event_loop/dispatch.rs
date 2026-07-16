use std::time::Duration;

use wayland_client::EventQueue;

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

pub(super) fn dispatch_events(
    event_queue: &mut EventQueue<WaylandState>,
    state: &mut WaylandState,
    runtime_wake: &RuntimeWakeSource,
    signals: &mut OverlaySignalState,
    animation_timeout: Option<Duration>,
) -> Result<(), anyhow::Error> {
    dispatch_with_timeout(
        event_queue,
        state,
        runtime_wake,
        |state| route_woken_sources(state, signals),
        animation_timeout,
    )
    .map_err(|e| anyhow::anyhow!("Wayland event queue error: {}", e))
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
