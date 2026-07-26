use std::time::Duration;

use wayland_client::EventQueue;

use super::super::super::state::WaylandState;
use super::super::helpers::dispatch_with_timeout;
use super::super::runtime_wake::RuntimeWakeSource;
use super::super::signals::OverlaySignalState;
use super::process_tray_actions_and_sync;

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
    signals: &mut OverlaySignalState<'_>,
) -> Result<(), anyhow::Error> {
    route_woken_persistence(state);
    state.drain_runtime_ui_completions();
    signals
        .drain_events()
        .map_err(|error| anyhow::anyhow!("overlay signal source failed: {error}"))?;

    if signals.exit_requested() {
        state.input_state.should_exit = true;
    }
    if signals.take_tray_action_requested() {
        process_tray_actions_and_sync(state);
    }
    Ok(())
}

pub(super) fn dispatch_events(
    event_queue: &mut EventQueue<WaylandState>,
    state: &mut WaylandState,
    runtime_wake: &RuntimeWakeSource,
    signals: &mut OverlaySignalState<'_>,
    animation_timeout: Option<Duration>,
) -> Result<(), anyhow::Error> {
    let signal_fd = signals
        .poll_raw_fd()
        .map_err(|error| anyhow::anyhow!("overlay signal descriptor failed: {error}"))?;
    dispatch_with_timeout(
        event_queue,
        state,
        runtime_wake,
        signal_fd,
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

    fn wait_for_runtime_wake(runtime_wake: &RuntimeWakeSource) -> std::io::Result<()> {
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
                if pollfd.revents & libc::POLLIN != 0 {
                    return Ok(());
                }
                return Err(std::io::Error::other(format!(
                    "runtime wake returned unexpected readiness {:#x}",
                    pollfd.revents
                )));
            }
            if ready == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "persistence worker did not wake the production route",
                ));
            }
            let err = std::io::Error::last_os_error();
            if err.kind() != std::io::ErrorKind::Interrupted {
                return Err(err);
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
        let dirty_window = session
            .prepare_autosave_submission()
            .expect("fixture dirty session prepares one autosave submission");

        let runtime_wake =
            RuntimeWakeSource::new().expect("fixture creates its persistence runtime wake");
        let temp = crate::test_temp::tempdir().expect("isolated catalog fixture");
        let mut controller = PersistenceController::start(
            runtime_wake
                .try_sender()
                .expect("test duplicates its persistence runtime eventfd"),
            crate::session::catalog::SessionCatalog::at_path(temp.path().join("sessions.json")),
        )
        .expect("test starts its persistence controller");
        let request_id = controller
            .try_submit(0, PersistenceOperation::PanicForTest)
            .expect("fixture submits its injected worker failure");
        session.commit_autosave_submission(request_id, dirty_window);

        wait_for_runtime_wake(&runtime_wake)
            .expect("fixture persistence worker wakes the production route");
        runtime_wake
            .drain()
            .expect("fixture drains its persistence runtime wake");
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
