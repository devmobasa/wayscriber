//! Identified capacity-one completion transport for event-loop runtime operations.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};

use crate::backend::wayland::RuntimeWakeHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::backend::wayland) struct RuntimeOperationId(u64);

impl fmt::Display for RuntimeOperationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone)]
pub(in crate::backend::wayland) struct RuntimeOperationIdSource {
    next: Arc<Mutex<Option<u64>>>,
}

impl RuntimeOperationIdSource {
    pub(in crate::backend::wayland) fn new() -> Self {
        Self {
            next: Arc::new(Mutex::new(Some(1))),
        }
    }

    fn allocate(&self) -> Result<RuntimeOperationId, RuntimeOperationSubmitError> {
        let mut next = self
            .next
            .lock()
            .map_err(|_| RuntimeOperationSubmitError::Unhealthy)?;
        let value = next.ok_or(RuntimeOperationSubmitError::IdentityExhausted)?;
        *next = value.checked_add(1);
        Ok(RuntimeOperationId(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) enum RuntimeOperationSubmitError {
    Busy { active_id: RuntimeOperationId },
    IdentityExhausted,
    Unhealthy,
    SpawnFailed { reason: String },
}

impl fmt::Display for RuntimeOperationSubmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy { active_id } => {
                write!(formatter, "runtime operation {active_id} is still active")
            }
            Self::IdentityExhausted => formatter.write_str("runtime operation IDs exhausted"),
            Self::Unhealthy => formatter.write_str("runtime operation controller is unhealthy"),
            Self::SpawnFailed { reason } => {
                write!(
                    formatter,
                    "failed to spawn runtime operation worker: {reason}"
                )
            }
        }
    }
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct RuntimeOperationSubmitFailure<C> {
    error: RuntimeOperationSubmitError,
    context: C,
}

impl<C> RuntimeOperationSubmitFailure<C> {
    pub(in crate::backend::wayland) fn into_parts(self) -> (RuntimeOperationSubmitError, C) {
        (self.error, self.context)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::backend::wayland) enum RuntimeOperationPoll<C, T> {
    Idle,
    Pending {
        id: RuntimeOperationId,
    },
    Ready {
        id: RuntimeOperationId,
        context: C,
        outcome: T,
    },
    ProducerFailed {
        id: RuntimeOperationId,
        context: C,
        reason: String,
    },
    Disconnected {
        id: RuntimeOperationId,
        context: C,
    },
}

enum ProducerMessage<T> {
    Ready {
        id: RuntimeOperationId,
        outcome: T,
    },
    Failed {
        id: RuntimeOperationId,
        reason: String,
    },
}

struct ActiveOperation<C, T> {
    id: RuntimeOperationId,
    context: C,
    receiver: Receiver<ProducerMessage<T>>,
}

pub(in crate::backend::wayland) struct RuntimeOperationController<C, T> {
    ids: RuntimeOperationIdSource,
    runtime_wake: RuntimeWakeHandle,
    active: Option<ActiveOperation<C, T>>,
    healthy: bool,
}

impl<C, T> RuntimeOperationController<C, T>
where
    T: Send + 'static,
{
    pub(in crate::backend::wayland) fn new(
        ids: RuntimeOperationIdSource,
        runtime_wake: RuntimeWakeHandle,
    ) -> Self {
        Self {
            ids,
            runtime_wake,
            active: None,
            healthy: true,
        }
    }

    pub(in crate::backend::wayland) fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Refuse future work after a caller detects a completion identity breach.
    pub(in crate::backend::wayland) fn mark_unhealthy(&mut self) {
        self.healthy = false;
    }

    pub(in crate::backend::wayland) fn try_submit(
        &mut self,
        context: C,
        thread_name: &'static str,
        operation: impl FnOnce() -> T + Send + 'static,
    ) -> Result<RuntimeOperationId, RuntimeOperationSubmitFailure<C>> {
        self.try_submit_with_spawner(context, operation, |job| {
            std::thread::Builder::new()
                .name(thread_name.to_string())
                .spawn(job)
                .map(drop)
        })
    }

    fn try_submit_with_spawner(
        &mut self,
        context: C,
        operation: impl FnOnce() -> T + Send + 'static,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<()>,
    ) -> Result<RuntimeOperationId, RuntimeOperationSubmitFailure<C>> {
        if !self.healthy {
            return Err(RuntimeOperationSubmitFailure {
                error: RuntimeOperationSubmitError::Unhealthy,
                context,
            });
        }
        if let Some(active) = &self.active {
            return Err(RuntimeOperationSubmitFailure {
                error: RuntimeOperationSubmitError::Busy {
                    active_id: active.id,
                },
                context,
            });
        }
        let id = match self.ids.allocate() {
            Ok(id) => id,
            Err(error) => {
                if error == RuntimeOperationSubmitError::Unhealthy {
                    self.healthy = false;
                }
                return Err(RuntimeOperationSubmitFailure { error, context });
            }
        };
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let runtime_wake = self.runtime_wake.clone();
        let job = Box::new(move || {
            let guard = RuntimeOperationExitGuard::new(id, sender, runtime_wake);
            let message = match catch_unwind(AssertUnwindSafe(operation)) {
                Ok(outcome) => ProducerMessage::Ready { id, outcome },
                Err(payload) => ProducerMessage::Failed {
                    id,
                    reason: panic_reason(payload),
                },
            };
            guard.publish(message);
        });
        if let Err(err) = spawn(job) {
            return Err(RuntimeOperationSubmitFailure {
                error: RuntimeOperationSubmitError::SpawnFailed {
                    reason: err.to_string(),
                },
                context,
            });
        }
        self.active = Some(ActiveOperation {
            id,
            context,
            receiver,
        });
        Ok(id)
    }

    pub(in crate::backend::wayland) fn poll(&mut self) -> RuntimeOperationPoll<C, T> {
        let Some(active) = self.active.take() else {
            return RuntimeOperationPoll::Idle;
        };
        let active_id = active.id;
        match active.receiver.try_recv() {
            Err(TryRecvError::Empty) => {
                self.active = Some(active);
                RuntimeOperationPoll::Pending { id: active_id }
            }
            Err(TryRecvError::Disconnected) => RuntimeOperationPoll::Disconnected {
                id: active.id,
                context: active.context,
            },
            Ok(ProducerMessage::Ready { id, outcome }) if id == active_id => {
                RuntimeOperationPoll::Ready {
                    id,
                    context: active.context,
                    outcome,
                }
            }
            Ok(ProducerMessage::Failed { id, reason }) if id == active_id => {
                RuntimeOperationPoll::ProducerFailed {
                    id,
                    context: active.context,
                    reason,
                }
            }
            Ok(ProducerMessage::Ready { id, .. } | ProducerMessage::Failed { id, .. }) => {
                self.healthy = false;
                RuntimeOperationPoll::ProducerFailed {
                    id: active.id,
                    context: active.context,
                    reason: format!(
                        "runtime operation worker reported transport identity {id}, expected {}",
                        active.id
                    ),
                }
            }
        }
    }
}

fn panic_reason(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "runtime operation worker panicked with a non-string payload".to_string()
    }
}

struct RuntimeOperationExitGuard<T> {
    id: RuntimeOperationId,
    sender: Option<SyncSender<ProducerMessage<T>>>,
    runtime_wake: RuntimeWakeHandle,
    terminal_published: bool,
}

impl<T> RuntimeOperationExitGuard<T> {
    fn new(
        id: RuntimeOperationId,
        sender: SyncSender<ProducerMessage<T>>,
        runtime_wake: RuntimeWakeHandle,
    ) -> Self {
        Self {
            id,
            sender: Some(sender),
            runtime_wake,
            terminal_published: false,
        }
    }

    fn publish(mut self, message: ProducerMessage<T>) {
        let sender = self
            .sender
            .take()
            .expect("runtime operation worker still holds its sender until publish");
        let result = sender.try_send(message);
        self.terminal_published = true;
        match result {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(_)) => {
                log::error!(
                    "Runtime operation worker {} found an impossible full terminal channel",
                    self.id
                );
            }
        }
        if let Err(err) = self.runtime_wake.wake() {
            log::error!(
                "Failed to wake runtime for runtime operation {}: {err}",
                self.id
            );
        }
    }
}

impl<T> Drop for RuntimeOperationExitGuard<T> {
    fn drop(&mut self) {
        if self.terminal_published {
            return;
        }
        // Closing the producer side is the terminal publication for a disconnect.
        self.sender.take();
        if let Err(err) = self.runtime_wake.wake() {
            log::error!(
                "Failed to wake runtime for disconnected runtime operation {}: {err}",
                self.id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;

    fn controller<C, T: Send + 'static>() -> (RuntimeWakeSource, RuntimeOperationController<C, T>) {
        let wake = RuntimeWakeSource::new().unwrap();
        let controller =
            RuntimeOperationController::new(RuntimeOperationIdSource::new(), wake.handle());
        (wake, controller)
    }

    fn wait_for_wake(wake: &RuntimeWakeSource) {
        let mut pollfd = libc::pollfd {
            fd: wake.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: pollfd and the runtime wake descriptor remain valid for the bounded wait.
        assert_eq!(unsafe { libc::poll(&mut pollfd, 1, 1_000) }, 1);
        assert_ne!(pollfd.revents & libc::POLLIN, 0);
    }

    fn wait_until_thread_is_blocked_in_poll(tid: libc::pid_t) {
        let stat_path = format!("/proc/self/task/{tid}/stat");
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let state = std::fs::read_to_string(&stat_path).ok().and_then(|stat| {
                stat.rsplit_once(") ")
                    .and_then(|(_, suffix)| suffix.chars().next())
            });
            if state == Some('S') {
                return;
            }
            assert!(Instant::now() < deadline, "poller did not block: {state:?}");
            std::thread::yield_now();
        }
    }

    #[test]
    fn shared_source_allocates_distinct_ids_across_controllers() {
        let wake = RuntimeWakeSource::new().unwrap();
        let ids = RuntimeOperationIdSource::new();
        let mut publish = RuntimeOperationController::new(ids.clone(), wake.handle());
        let mut paste = RuntimeOperationController::new(ids, wake.handle());

        let publish_id = publish.try_submit("publish", "test-publish", || 1).unwrap();
        let paste_id = paste.try_submit("paste", "test-paste", || 2).unwrap();
        assert_eq!(publish_id, RuntimeOperationId(1));
        assert_eq!(paste_id, RuntimeOperationId(2));
    }

    #[test]
    fn unread_result_remains_busy_until_matching_consumption() {
        let (wake, mut controller) = controller::<&'static str, u32>();
        let first = controller.try_submit("first", "test-first", || 7).unwrap();
        wait_for_wake(&wake);
        let failure = controller
            .try_submit("second", "test-second", || 8)
            .unwrap_err();
        assert_eq!(
            failure.into_parts().0,
            RuntimeOperationSubmitError::Busy { active_id: first }
        );
        assert_eq!(
            controller.poll(),
            RuntimeOperationPoll::Ready {
                id: first,
                context: "first",
                outcome: 7,
            }
        );
        let next = controller.try_submit("third", "test-third", || 9).unwrap();
        assert!(next > first);
    }

    #[test]
    fn pending_operation_rejects_submission_as_busy() {
        let (wake, mut controller) = controller::<&'static str, u32>();
        let (release_tx, release_rx) = mpsc::channel();
        let first = controller
            .try_submit("first", "test-pending", move || {
                release_rx.recv().unwrap();
                7
            })
            .unwrap();

        let failure = controller
            .try_submit("second", "test-busy", || 8)
            .unwrap_err();
        assert_eq!(
            failure.into_parts(),
            (
                RuntimeOperationSubmitError::Busy { active_id: first },
                "second",
            )
        );

        release_tx.send(()).unwrap();
        wait_for_wake(&wake);
        assert!(matches!(
            controller.poll(),
            RuntimeOperationPoll::Ready { .. }
        ));
    }

    #[test]
    fn completion_is_visible_before_its_wake() {
        let (wake, mut controller) = controller::<u64, u64>();
        let id = controller.try_submit(11, "test-ready", || 29).unwrap();
        wait_for_wake(&wake);
        assert_eq!(
            controller.poll(),
            RuntimeOperationPoll::Ready {
                id,
                context: 11,
                outcome: 29,
            }
        );
    }

    #[test]
    fn completion_unblocks_an_existing_runtime_poll() {
        let (wake, mut controller) = controller::<u64, u64>();
        let (release_tx, release_rx) = mpsc::channel();
        let id = controller
            .try_submit(5, "test-waiting", move || {
                release_rx.recv().unwrap();
                13
            })
            .unwrap();
        let (tid_tx, tid_rx) = mpsc::channel();
        let poller = std::thread::spawn(move || {
            // SAFETY: gettid has no preconditions and is used only to observe this test thread.
            let tid = unsafe { libc::syscall(libc::SYS_gettid) as libc::pid_t };
            tid_tx.send(tid).unwrap();
            wait_for_wake(&wake);
        });
        let tid = tid_rx.recv().unwrap();
        wait_until_thread_is_blocked_in_poll(tid);
        release_tx.send(()).unwrap();
        poller.join().unwrap();
        assert_eq!(
            controller.poll(),
            RuntimeOperationPoll::Ready {
                id,
                context: 5,
                outcome: 13,
            }
        );
    }

    #[test]
    fn producer_panic_publishes_failure_and_wakes() {
        let (wake, mut controller) = controller::<u64, u64>();
        let id = controller
            .try_submit(17, "test-panic", || panic!("expected producer panic"))
            .unwrap();
        wait_for_wake(&wake);
        assert_eq!(
            controller.poll(),
            RuntimeOperationPoll::ProducerFailed {
                id,
                context: 17,
                reason: "expected producer panic".to_string(),
            }
        );
    }

    #[test]
    fn exit_guard_disconnects_before_waking() {
        let wake = RuntimeWakeSource::new().unwrap();
        let ids = RuntimeOperationIdSource::new();
        let mut controller = RuntimeOperationController::<u64, u64>::new(ids, wake.handle());
        let id = RuntimeOperationId(9);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        controller.active = Some(ActiveOperation {
            id,
            context: 31,
            receiver,
        });
        drop(RuntimeOperationExitGuard::new(id, sender, wake.handle()));
        wait_for_wake(&wake);
        assert_eq!(
            controller.poll(),
            RuntimeOperationPoll::Disconnected { id, context: 31 }
        );
    }

    #[test]
    fn spawn_failure_installs_no_active_identity_and_returns_context() {
        let (wake, mut controller) = controller::<u64, u64>();
        let failure = controller
            .try_submit_with_spawner(
                41,
                || 1,
                |_job| Err(std::io::Error::other("injected spawn failure")),
            )
            .unwrap_err();
        assert_eq!(
            failure.into_parts(),
            (
                RuntimeOperationSubmitError::SpawnFailed {
                    reason: "injected spawn failure".to_string(),
                },
                41,
            )
        );
        assert!(!controller.is_active());
        let mut pollfd = libc::pollfd {
            fd: wake.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: pollfd and the runtime wake descriptor are valid for this non-blocking poll.
        assert_eq!(unsafe { libc::poll(&mut pollfd, 1, 0) }, 0);
        assert_eq!(
            controller
                .try_submit(42, "test-after-spawn-failure", || 2)
                .unwrap(),
            RuntimeOperationId(2)
        );
    }

    #[test]
    fn identity_mismatch_restores_active_context_and_disables_controller() {
        let (_wake, mut controller) = controller::<u64, u64>();
        let active_id = RuntimeOperationId(3);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        controller.active = Some(ActiveOperation {
            id: active_id,
            context: 43,
            receiver,
        });
        sender
            .try_send(ProducerMessage::Ready {
                id: RuntimeOperationId(4),
                outcome: 99,
            })
            .unwrap();
        assert!(matches!(
            controller.poll(),
            RuntimeOperationPoll::ProducerFailed {
                id,
                context: 43,
                ..
            } if id == active_id
        ));
        let failure = controller
            .try_submit(44, "test-unhealthy", || 1)
            .unwrap_err();
        assert_eq!(
            failure.into_parts().0,
            RuntimeOperationSubmitError::Unhealthy
        );
    }

    #[test]
    fn caller_detected_identity_breach_disables_future_submissions() {
        let (_wake, mut controller) = controller::<u64, u64>();
        controller.mark_unhealthy();
        let failure = controller
            .try_submit(44, "test-caller-unhealthy", || 1)
            .unwrap_err();
        assert_eq!(
            failure.into_parts(),
            (RuntimeOperationSubmitError::Unhealthy, 44)
        );
    }

    #[test]
    fn maximum_identity_is_used_once_without_wrapping() {
        let (wake, mut controller) = controller::<u64, u64>();
        *controller.ids.next.lock().unwrap() = Some(u64::MAX);
        let id = controller.try_submit(1, "test-max-id", || 2).unwrap();
        assert_eq!(id, RuntimeOperationId(u64::MAX));
        wait_for_wake(&wake);
        assert!(matches!(
            controller.poll(),
            RuntimeOperationPoll::Ready { .. }
        ));
        let failure = controller
            .try_submit(3, "test-exhausted", || 4)
            .unwrap_err();
        assert_eq!(
            failure.into_parts().0,
            RuntimeOperationSubmitError::IdentityExhausted
        );
    }

    #[derive(Clone, Debug)]
    struct DropContext(Arc<AtomicUsize>);

    impl Drop for DropContext {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn dropping_pending_controller_releases_event_context_immediately() {
        let (_wake, mut controller) = controller::<DropContext, u64>();
        let drops = Arc::new(AtomicUsize::new(0));
        let (release_tx, release_rx) = mpsc::channel();
        controller
            .try_submit(DropContext(drops.clone()), "test-drop-pending", move || {
                let _ = release_rx.recv();
                1
            })
            .unwrap();
        drop(controller);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        release_tx.send(()).unwrap();
    }

    #[test]
    fn dropping_completed_unread_controller_releases_event_context() {
        let (wake, mut controller) = controller::<DropContext, u64>();
        let drops = Arc::new(AtomicUsize::new(0));
        controller
            .try_submit(DropContext(drops.clone()), "test-drop-completed", || 1)
            .unwrap();
        wait_for_wake(&wake);
        drop(controller);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
