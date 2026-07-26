//! Identified capacity-one completion transport for event-loop clipboard operations.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, Sender, SyncSender, TryRecvError, TrySendError};
use std::thread::JoinHandle;

use crate::backend::wayland::RuntimeWakeSender;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::backend::wayland) struct ClipboardOperationId(u64);

impl fmt::Display for ClipboardOperationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

pub(in crate::backend::wayland) struct ClipboardOperationIdSource {
    next: Option<u64>,
}

impl ClipboardOperationIdSource {
    pub(in crate::backend::wayland) fn new() -> Self {
        Self { next: Some(1) }
    }

    fn allocate(&mut self) -> Result<ClipboardOperationId, ClipboardSubmitError> {
        let value = self.next.ok_or(ClipboardSubmitError::IdentityExhausted)?;
        self.next = value.checked_add(1);
        Ok(ClipboardOperationId(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) enum ClipboardSubmitError {
    Busy { active_id: ClipboardOperationId },
    IdentityExhausted,
    Unhealthy,
    WakeUnavailable { reason: String },
    SpawnFailed { reason: String },
}

impl fmt::Display for ClipboardSubmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy { active_id } => {
                write!(formatter, "clipboard operation {active_id} is still active")
            }
            Self::IdentityExhausted => formatter.write_str("clipboard operation IDs exhausted"),
            Self::Unhealthy => formatter.write_str("clipboard completion controller is unhealthy"),
            Self::WakeUnavailable { reason } => {
                write!(formatter, "clipboard runtime wake is unavailable: {reason}")
            }
            Self::SpawnFailed { reason } => {
                write!(formatter, "failed to spawn clipboard producer: {reason}")
            }
        }
    }
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct ClipboardSubmitFailure<C> {
    error: ClipboardSubmitError,
    context: C,
}

impl<C> ClipboardSubmitFailure<C> {
    pub(in crate::backend::wayland) fn into_parts(self) -> (ClipboardSubmitError, C) {
        (self.error, self.context)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::backend::wayland) enum ClipboardPoll<C, T> {
    Idle,
    Pending {
        id: ClipboardOperationId,
    },
    Ready {
        id: ClipboardOperationId,
        context: C,
        outcome: T,
    },
    ProducerFailed {
        id: ClipboardOperationId,
        context: C,
        reason: String,
    },
    Cancelled {
        id: ClipboardOperationId,
        context: C,
    },
    Disconnected {
        id: ClipboardOperationId,
        context: C,
    },
}

#[derive(Debug)]
pub(in crate::backend::wayland) enum ClipboardShutdown<C> {
    Idle,
    Cancelled {
        id: ClipboardOperationId,
        context: C,
    },
}

enum ProducerMessage<T> {
    Ready {
        id: ClipboardOperationId,
        outcome: T,
    },
    Failed {
        id: ClipboardOperationId,
        reason: String,
    },
    Cancelled {
        id: ClipboardOperationId,
    },
}

struct ActiveOperation<C, T> {
    id: ClipboardOperationId,
    context: C,
    receiver: Receiver<ProducerMessage<T>>,
    cancel: Sender<()>,
    worker: JoinHandle<()>,
}

pub(in crate::backend::wayland) struct ClipboardCancellation {
    receiver: Receiver<()>,
    cancelled: bool,
}

impl ClipboardCancellation {
    pub(in crate::backend::wayland) fn is_cancelled(&mut self) -> bool {
        if self.cancelled {
            return true;
        }
        self.cancelled = match self.receiver.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => true,
            Err(TryRecvError::Empty) => false,
        };
        self.cancelled
    }
}

pub(in crate::backend::wayland) struct ClipboardOperationController<C, T> {
    runtime_wake: RuntimeWakeSender,
    active: Option<ActiveOperation<C, T>>,
    retired_workers: Vec<JoinHandle<()>>,
    healthy: bool,
}

impl<C, T> ClipboardOperationController<C, T>
where
    T: Send + 'static,
{
    pub(in crate::backend::wayland) fn new(runtime_wake: RuntimeWakeSender) -> Self {
        Self {
            runtime_wake,
            active: None,
            retired_workers: Vec::new(),
            healthy: true,
        }
    }

    pub(in crate::backend::wayland) fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub(in crate::backend::wayland) fn try_submit(
        &mut self,
        ids: &mut ClipboardOperationIdSource,
        context: C,
        thread_name: &'static str,
        operation: impl FnOnce(&mut ClipboardCancellation) -> T + Send + 'static,
    ) -> Result<ClipboardOperationId, ClipboardSubmitFailure<C>> {
        self.try_submit_with_spawner(ids, context, operation, |job| {
            std::thread::Builder::new()
                .name(thread_name.to_string())
                .spawn(job)
        })
    }

    fn try_submit_with_spawner(
        &mut self,
        ids: &mut ClipboardOperationIdSource,
        context: C,
        operation: impl FnOnce(&mut ClipboardCancellation) -> T + Send + 'static,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>>,
    ) -> Result<ClipboardOperationId, ClipboardSubmitFailure<C>> {
        self.try_submit_with_message_factory(
            ids,
            context,
            move |id, cancellation| {
                if cancellation.is_cancelled() {
                    return ProducerMessage::Cancelled { id };
                }
                match catch_unwind(AssertUnwindSafe(|| operation(cancellation))) {
                    Ok(_) if cancellation.is_cancelled() => ProducerMessage::Cancelled { id },
                    Ok(outcome) => ProducerMessage::Ready { id, outcome },
                    Err(payload) => ProducerMessage::Failed {
                        id,
                        reason: panic_reason(payload),
                    },
                }
            },
            spawn,
        )
    }

    fn try_submit_with_message_factory(
        &mut self,
        ids: &mut ClipboardOperationIdSource,
        context: C,
        message_factory: impl FnOnce(
            ClipboardOperationId,
            &mut ClipboardCancellation,
        ) -> ProducerMessage<T>
        + Send
        + 'static,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>>,
    ) -> Result<ClipboardOperationId, ClipboardSubmitFailure<C>> {
        self.reap_finished_workers();
        if !self.healthy {
            return Err(ClipboardSubmitFailure {
                error: ClipboardSubmitError::Unhealthy,
                context,
            });
        }
        if let Some(active) = &self.active {
            return Err(ClipboardSubmitFailure {
                error: ClipboardSubmitError::Busy {
                    active_id: active.id,
                },
                context,
            });
        }
        let id = match ids.allocate() {
            Ok(id) => id,
            Err(error) => return Err(ClipboardSubmitFailure { error, context }),
        };
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let (cancel, cancel_receiver) = std::sync::mpsc::channel();
        let runtime_wake = match self.runtime_wake.try_duplicate() {
            Ok(runtime_wake) => runtime_wake,
            Err(error) => {
                return Err(ClipboardSubmitFailure {
                    error: ClipboardSubmitError::WakeUnavailable {
                        reason: error.to_string(),
                    },
                    context,
                });
            }
        };
        let job = Box::new(move || {
            let guard = ClipboardProducerExitGuard::new(id, sender, runtime_wake);
            let mut cancellation = ClipboardCancellation {
                receiver: cancel_receiver,
                cancelled: false,
            };
            let message = message_factory(id, &mut cancellation);
            guard.publish(message);
        });
        let worker = match spawn(job) {
            Ok(worker) => worker,
            Err(err) => {
                return Err(ClipboardSubmitFailure {
                    error: ClipboardSubmitError::SpawnFailed {
                        reason: err.to_string(),
                    },
                    context,
                });
            }
        };
        self.active = Some(ActiveOperation {
            id,
            context,
            receiver,
            cancel,
            worker,
        });
        Ok(id)
    }

    #[cfg(test)]
    fn try_submit_failure(
        &mut self,
        ids: &mut ClipboardOperationIdSource,
        context: C,
        reason: String,
    ) -> Result<ClipboardOperationId, ClipboardSubmitFailure<C>> {
        self.try_submit_with_message_factory(
            ids,
            context,
            move |id, _cancellation| ProducerMessage::Failed { id, reason },
            |job| std::thread::Builder::new().spawn(job),
        )
    }

    pub(in crate::backend::wayland) fn poll(&mut self) -> ClipboardPoll<C, T> {
        self.reap_finished_workers();
        let Some(active) = self.active.take() else {
            return ClipboardPoll::Idle;
        };
        let ActiveOperation {
            id: active_id,
            context,
            receiver,
            cancel,
            worker,
        } = active;
        match receiver.try_recv() {
            Err(TryRecvError::Empty) => {
                self.active = Some(ActiveOperation {
                    id: active_id,
                    context,
                    receiver,
                    cancel,
                    worker,
                });
                ClipboardPoll::Pending { id: active_id }
            }
            Err(TryRecvError::Disconnected) => {
                drop(cancel);
                self.retire_active_worker(worker);
                ClipboardPoll::Disconnected {
                    id: active_id,
                    context,
                }
            }
            Ok(ProducerMessage::Ready { id, outcome }) if id == active_id => {
                drop(cancel);
                self.retire_active_worker(worker);
                ClipboardPoll::Ready {
                    id,
                    context,
                    outcome,
                }
            }
            Ok(ProducerMessage::Failed { id, reason }) if id == active_id => {
                drop(cancel);
                self.retire_active_worker(worker);
                ClipboardPoll::ProducerFailed {
                    id,
                    context,
                    reason,
                }
            }
            Ok(ProducerMessage::Cancelled { id }) if id == active_id => {
                drop(cancel);
                self.retire_active_worker(worker);
                ClipboardPoll::Cancelled { id, context }
            }
            Ok(
                ProducerMessage::Ready { id, .. }
                | ProducerMessage::Failed { id, .. }
                | ProducerMessage::Cancelled { id },
            ) => {
                drop(cancel);
                self.retire_active_worker(worker);
                self.healthy = false;
                ClipboardPoll::ProducerFailed {
                    id: active_id,
                    context,
                    reason: format!(
                        "clipboard producer reported transport identity {id}, expected {}",
                        active_id
                    ),
                }
            }
        }
    }

    fn reap_finished_workers(&mut self) {
        let mut pending = Vec::with_capacity(self.retired_workers.len());
        for worker in std::mem::take(&mut self.retired_workers) {
            if worker.is_finished() {
                if worker.join().is_err() {
                    self.healthy = false;
                    log::error!("Clipboard producer failed outside its operation guard");
                }
            } else {
                pending.push(worker);
            }
        }
        self.retired_workers = pending;
    }

    fn retire_active_worker(&mut self, worker: JoinHandle<()>) {
        self.retired_workers.push(worker);
    }

    pub(in crate::backend::wayland) fn request_shutdown(&mut self) -> ClipboardShutdown<C> {
        let Some(active) = self.active.take() else {
            return ClipboardShutdown::Idle;
        };
        let ActiveOperation {
            id,
            context,
            receiver,
            cancel,
            worker,
        } = active;
        let _ = cancel.send(());
        drop(receiver);
        self.retire_active_worker(worker);
        ClipboardShutdown::Cancelled { id, context }
    }

    pub(in crate::backend::wayland) fn finish_shutdown(&mut self) {
        for worker in std::mem::take(&mut self.retired_workers) {
            if worker.join().is_err() {
                log::error!("Clipboard producer failed while the runtime was shutting down");
            }
        }
    }
}

impl<C, T> Drop for ClipboardOperationController<C, T> {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            let ActiveOperation {
                context,
                cancel,
                worker,
                ..
            } = active;
            drop(context);
            let _ = cancel.send(());
            if worker.join().is_err() {
                log::error!("Clipboard producer failed while the runtime was shutting down");
            }
        }
        for worker in std::mem::take(&mut self.retired_workers) {
            if worker.join().is_err() {
                log::error!("Completed clipboard producer failed during final reaping");
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
        "clipboard producer panicked with a non-string payload".to_string()
    }
}

struct ClipboardProducerExitGuard<T> {
    // Field order is deliberate: an unwinding producer closes the channel
    // before the exit wake is emitted.
    sender: SyncSender<ProducerMessage<T>>,
    exit_wake: ClipboardProducerExitWake,
}

struct ClipboardProducerExitWake {
    id: ClipboardOperationId,
    runtime_wake: RuntimeWakeSender,
    terminal_published: bool,
}

impl<T> ClipboardProducerExitGuard<T> {
    fn new(
        id: ClipboardOperationId,
        sender: SyncSender<ProducerMessage<T>>,
        runtime_wake: RuntimeWakeSender,
    ) -> Self {
        Self {
            sender,
            exit_wake: ClipboardProducerExitWake {
                id,
                runtime_wake,
                terminal_published: false,
            },
        }
    }

    fn publish(self, message: ProducerMessage<T>) {
        let mut exit_wake = self.exit_wake;
        let result = {
            let sender = self.sender;
            sender.try_send(message)
        };
        exit_wake.terminal_published = true;
        match result {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(_)) => {
                log::error!(
                    "Clipboard producer {} found an impossible full terminal channel",
                    exit_wake.id
                );
            }
        }
        if let Err(err) = exit_wake.runtime_wake.wake() {
            log::error!(
                "Failed to wake runtime for clipboard operation {}: {err}",
                exit_wake.id
            );
        }
    }
}

impl Drop for ClipboardProducerExitWake {
    fn drop(&mut self) {
        if self.terminal_published {
            return;
        }
        if let Err(err) = self.runtime_wake.wake() {
            log::error!(
                "Failed to wake runtime for disconnected clipboard operation {}: {err}",
                self.id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::os::fd::AsRawFd;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;

    struct ControllerFixture<C, T> {
        wake: RuntimeWakeSource,
        ids: ClipboardOperationIdSource,
        controller: ClipboardOperationController<C, T>,
    }

    fn controller<C, T: Send + 'static>() -> ControllerFixture<C, T> {
        let wake = RuntimeWakeSource::new()
            .expect("clipboard completion fixture creates a runtime eventfd");
        let controller = ClipboardOperationController::new(
            wake.try_sender()
                .expect("clipboard completion fixture duplicates its runtime eventfd"),
        );
        ControllerFixture {
            wake,
            ids: ClipboardOperationIdSource::new(),
            controller,
        }
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
    fn root_source_allocates_distinct_ids_across_controllers() {
        let wake = RuntimeWakeSource::new()
            .expect("cross-controller identity fixture creates a runtime eventfd");
        let mut ids = ClipboardOperationIdSource::new();
        let mut publish = ClipboardOperationController::new(
            wake.try_sender()
                .expect("cross-controller fixture duplicates its publish eventfd"),
        );
        let mut paste = ClipboardOperationController::new(
            wake.try_sender()
                .expect("cross-controller fixture duplicates its paste eventfd"),
        );

        let publish_id = publish
            .try_submit(&mut ids, "publish", "test-publish", |_| 1)
            .expect("publish fixture has an available operation identity");
        let paste_id = paste
            .try_submit(&mut ids, "paste", "test-paste", |_| 2)
            .expect("paste fixture has an available operation identity");
        assert_eq!(publish_id, ClipboardOperationId(1));
        assert_eq!(paste_id, ClipboardOperationId(2));
    }

    #[test]
    fn unread_result_remains_busy_until_matching_consumption() -> Result<(), &'static str> {
        let ControllerFixture {
            wake,
            mut ids,
            mut controller,
        } = controller::<&'static str, u32>();
        let first = controller
            .try_submit(&mut ids, "first", "test-first", |_| 7)
            .expect("busy fixture has an available first operation identity");
        wait_for_wake(&wake);
        let Err(failure) = controller.try_submit(&mut ids, "second", "test-second", |_| 8) else {
            return Err("unread completion fixture unexpectedly accepted a second job");
        };
        assert_eq!(
            failure.into_parts().0,
            ClipboardSubmitError::Busy { active_id: first }
        );
        assert_eq!(
            controller.poll(),
            ClipboardPoll::Ready {
                id: first,
                context: "first",
                outcome: 7,
            }
        );
        let next = controller
            .try_submit(&mut ids, "third", "test-third", |_| 9)
            .expect("consumed completion fixture accepts the next operation");
        assert_eq!(next, ClipboardOperationId(2));
        Ok(())
    }

    #[test]
    fn pending_operation_rejects_submission_as_busy() -> Result<(), &'static str> {
        let ControllerFixture {
            wake,
            mut ids,
            mut controller,
        } = controller::<&'static str, u32>();
        let (release_tx, release_rx) = mpsc::channel();
        let first = controller
            .try_submit(&mut ids, "first", "test-pending", move |_| {
                release_rx
                    .recv()
                    .expect("pending fixture retains its release sender");
                7
            })
            .expect("pending fixture has an available first operation identity");

        let Err(failure) = controller.try_submit(&mut ids, "second", "test-busy", |_| 8) else {
            return Err("pending operation fixture unexpectedly accepted a second job");
        };
        assert_eq!(
            failure.into_parts(),
            (ClipboardSubmitError::Busy { active_id: first }, "second",)
        );

        release_tx
            .send(())
            .expect("pending fixture retains its worker receiver");
        wait_for_wake(&wake);
        assert!(matches!(controller.poll(), ClipboardPoll::Ready { .. }));
        Ok(())
    }

    #[test]
    fn completion_is_visible_before_its_wake() {
        let ControllerFixture {
            wake,
            mut ids,
            mut controller,
        } = controller::<u64, u64>();
        let id = controller
            .try_submit(&mut ids, 11, "test-ready", |_| 29)
            .expect("ready fixture has an available operation identity");
        wait_for_wake(&wake);
        assert_eq!(
            controller.poll(),
            ClipboardPoll::Ready {
                id,
                context: 11,
                outcome: 29,
            }
        );
    }

    #[test]
    fn completion_unblocks_an_existing_runtime_poll() {
        let ControllerFixture {
            wake,
            mut ids,
            mut controller,
        } = controller::<u64, u64>();
        let (release_tx, release_rx) = mpsc::channel();
        let id = controller
            .try_submit(&mut ids, 5, "test-waiting", move |_| {
                release_rx
                    .recv()
                    .expect("poll wake fixture retains its release sender");
                13
            })
            .expect("poll wake fixture has an available operation identity");
        let (tid_tx, tid_rx) = mpsc::channel();
        let poller = std::thread::spawn(move || {
            // SAFETY: gettid has no preconditions and is used only to observe this test thread.
            let tid = unsafe { libc::syscall(libc::SYS_gettid) as libc::pid_t };
            tid_tx
                .send(tid)
                .expect("poll wake fixture retains its thread-id receiver");
            wait_for_wake(&wake);
        });
        let tid = tid_rx
            .recv()
            .expect("poll wake fixture thread publishes its thread ID");
        wait_until_thread_is_blocked_in_poll(tid);
        release_tx
            .send(())
            .expect("poll wake fixture retains its worker receiver");
        poller
            .join()
            .expect("poll wake fixture thread finishes without failure");
        assert_eq!(
            controller.poll(),
            ClipboardPoll::Ready {
                id,
                context: 5,
                outcome: 13,
            }
        );
    }

    #[test]
    fn injected_producer_failure_publishes_failure_and_wakes() {
        let ControllerFixture {
            wake,
            mut ids,
            mut controller,
        } = controller::<u64, u64>();
        let id = controller
            .try_submit_failure(&mut ids, 17, "injected producer failure".to_string())
            .expect("producer failure fixture has an available operation identity");
        wait_for_wake(&wake);
        assert_eq!(
            controller.poll(),
            ClipboardPoll::ProducerFailed {
                id,
                context: 17,
                reason: "injected producer failure".to_string(),
            }
        );
    }

    #[test]
    fn exit_guard_disconnects_before_waking() {
        let wake = RuntimeWakeSource::new().expect("disconnect fixture creates a runtime eventfd");
        let mut controller = ClipboardOperationController::<u64, u64>::new(
            wake.try_sender()
                .expect("disconnect fixture duplicates its controller eventfd"),
        );
        let id = ClipboardOperationId(9);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let (cancel, _cancel_receiver) = std::sync::mpsc::channel();
        controller.active = Some(ActiveOperation {
            id,
            context: 31,
            receiver,
            cancel,
            worker: std::thread::spawn(|| {}),
        });
        drop(ClipboardProducerExitGuard::new(
            id,
            sender,
            wake.try_sender()
                .expect("disconnect fixture duplicates its producer eventfd"),
        ));
        wait_for_wake(&wake);
        assert_eq!(
            controller.poll(),
            ClipboardPoll::Disconnected { id, context: 31 }
        );
    }

    #[test]
    fn spawn_failure_installs_no_active_identity_and_returns_context() -> Result<(), &'static str> {
        let ControllerFixture {
            wake,
            mut ids,
            mut controller,
        } = controller::<u64, u64>();
        let Err(failure) = controller.try_submit_with_spawner(
            &mut ids,
            41,
            |_| 1,
            |_job| Err(std::io::Error::other("injected spawn failure")),
        ) else {
            return Err("spawn-failure fixture unexpectedly accepted its job");
        };
        assert_eq!(
            failure.into_parts(),
            (
                ClipboardSubmitError::SpawnFailed {
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
                .try_submit(&mut ids, 42, "test-after-spawn-failure", |_| 2)
                .expect("spawn-failure fixture accepts the next job"),
            ClipboardOperationId(2)
        );
        Ok(())
    }

    #[test]
    fn identity_mismatch_restores_active_context_and_disables_controller()
    -> Result<(), &'static str> {
        let ControllerFixture {
            wake: _wake,
            mut ids,
            mut controller,
        } = controller::<u64, u64>();
        let active_id = ClipboardOperationId(3);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let (cancel, _cancel_receiver) = std::sync::mpsc::channel();
        controller.active = Some(ActiveOperation {
            id: active_id,
            context: 43,
            receiver,
            cancel,
            worker: std::thread::spawn(|| {}),
        });
        sender
            .try_send(ProducerMessage::Ready {
                id: ClipboardOperationId(4),
                outcome: 99,
            })
            .expect("identity-mismatch fixture retains its active receiver");
        assert!(matches!(
            controller.poll(),
            ClipboardPoll::ProducerFailed {
                id,
                context: 43,
                ..
            } if id == active_id
        ));
        let Err(failure) = controller.try_submit(&mut ids, 44, "test-unhealthy", |_| 1) else {
            return Err("unhealthy controller fixture unexpectedly accepted another job");
        };
        assert_eq!(failure.into_parts().0, ClipboardSubmitError::Unhealthy);
        Ok(())
    }

    #[test]
    fn maximum_identity_is_used_once_without_wrapping() -> Result<(), &'static str> {
        let ControllerFixture {
            wake,
            mut ids,
            mut controller,
        } = controller::<u64, u64>();
        ids.next = Some(u64::MAX);
        let id = controller
            .try_submit(&mut ids, 1, "test-max-id", |_| 2)
            .expect("maximum-identity fixture has its final operation identity");
        assert_eq!(id, ClipboardOperationId(u64::MAX));
        wait_for_wake(&wake);
        assert!(matches!(controller.poll(), ClipboardPoll::Ready { .. }));
        let Err(failure) = controller.try_submit(&mut ids, 3, "test-exhausted", |_| 4) else {
            return Err("exhausted identity fixture unexpectedly accepted another job");
        };
        assert_eq!(
            failure.into_parts().0,
            ClipboardSubmitError::IdentityExhausted
        );
        Ok(())
    }

    #[derive(Debug)]
    struct DropContext(mpsc::Sender<()>);

    impl Drop for DropContext {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }

    #[test]
    fn dropping_pending_controller_cancels_and_joins_before_returning() {
        let ControllerFixture {
            wake: _wake,
            mut ids,
            mut controller,
        } = controller::<DropContext, u64>();
        let (drop_tx, drop_rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();
        let (worker_exit_tx, worker_exit_rx) = mpsc::channel();
        controller
            .try_submit(
                &mut ids,
                DropContext(drop_tx),
                "test-drop-pending",
                move |cancellation| {
                    let _ = started_tx.send(());
                    while !cancellation.is_cancelled() {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    let _ = worker_exit_tx.send(());
                    1
                },
            )
            .expect("pending-drop fixture has an available operation identity");
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("pending-drop worker announces entry into its operation");
        drop(controller);
        drop_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("dropping a pending controller releases its event context");
        worker_exit_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("controller drop joins its cooperatively cancelled worker");
    }

    #[test]
    fn coordinated_root_shutdown_cancels_every_worker_before_joining_any() {
        let wake = RuntimeWakeSource::new()
            .expect("coordinated-shutdown fixture creates a runtime eventfd");
        let mut ids = ClipboardOperationIdSource::new();
        let mut first = ClipboardOperationController::new(
            wake.try_sender()
                .expect("coordinated-shutdown fixture duplicates its first eventfd"),
        );
        let mut second = ClipboardOperationController::new(
            wake.try_sender()
                .expect("coordinated-shutdown fixture duplicates its second eventfd"),
        );
        let (started_tx, started_rx) = mpsc::channel();
        let second_started_tx = started_tx.clone();
        let (second_cancelled_tx, second_cancelled_rx) = mpsc::channel();
        let (first_observed_tx, first_observed_rx) = mpsc::channel();

        first
            .try_submit(&mut ids, "first", "test-shutdown-first", move |cancel| {
                let _ = started_tx.send("first");
                while !cancel.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(1));
                }
                let observed_second = second_cancelled_rx
                    .recv_timeout(Duration::from_secs(1))
                    .is_ok();
                let _ = first_observed_tx.send(observed_second);
                1
            })
            .expect("coordinated-shutdown fixture submits its first worker");
        second
            .try_submit(&mut ids, "second", "test-shutdown-second", move |cancel| {
                let _ = second_started_tx.send("second");
                while !cancel.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(1));
                }
                let _ = second_cancelled_tx.send(());
                2
            })
            .expect("coordinated-shutdown fixture submits its second worker");
        let mut started = [
            started_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("first coordinated worker announces startup"),
            started_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("second coordinated worker announces startup"),
        ];
        started.sort_unstable();
        assert_eq!(started, ["first", "second"]);

        let first_shutdown = first.request_shutdown();
        let second_shutdown = second.request_shutdown();
        assert!(matches!(
            first_shutdown,
            ClipboardShutdown::Cancelled {
                context: "first",
                ..
            }
        ));
        assert!(matches!(
            second_shutdown,
            ClipboardShutdown::Cancelled {
                context: "second",
                ..
            }
        ));
        first.finish_shutdown();
        second.finish_shutdown();
        assert!(
            first_observed_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("first worker reports whether the second was cancelled before its join")
        );
    }

    #[test]
    fn controller_drop_waits_for_bounded_broker_call_before_owner_teardown() {
        let process_broker_owner = crate::process_broker::start_for_runtime()
            .expect("broker-teardown fixture starts its explicit process broker owner");
        let process_broker = process_broker_owner.handle();
        let ControllerFixture {
            wake: _wake,
            mut ids,
            mut controller,
        } = controller::<u64, anyhow::Result<crate::process_broker::BrokerOutput>>();
        let (started_tx, started_rx) = mpsc::channel();
        controller
            .try_submit(
                &mut ids,
                1,
                "test-broker-bounded-drop",
                move |_cancellation| {
                    let _ = started_tx.send(());
                    process_broker.run(
                        crate::process_broker::HelperKind::TestSleep,
                        OsStr::new("sleep"),
                        [OsStr::new("0.1")],
                        Vec::new(),
                        Duration::from_secs(1),
                        1024,
                    )
                },
            )
            .expect("broker-teardown fixture submits its bounded broker operation");
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("broker-teardown worker announces entry into its bounded call");

        let started = Instant::now();
        drop(controller);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "controller teardown exceeded the broker operation deadline"
        );

        let health = process_broker_owner.handle().run(
            crate::process_broker::HelperKind::TestCat,
            OsStr::new("cat"),
            std::iter::empty::<&OsStr>(),
            b"healthy".to_vec(),
            Duration::from_secs(1),
            1024,
        );
        assert!(
            matches!(health, Ok(output) if output.status == 0 && output.stdout == b"healthy"),
            "broker owner must remain healthy until clipboard producers are joined"
        );
        drop(process_broker_owner);
    }

    #[test]
    fn dropping_completed_unread_controller_releases_event_context() {
        let ControllerFixture {
            wake,
            mut ids,
            mut controller,
        } = controller::<DropContext, u64>();
        let (drop_tx, drop_rx) = mpsc::channel();
        controller
            .try_submit(&mut ids, DropContext(drop_tx), "test-drop-completed", |_| 1)
            .expect("completed-drop fixture has an available operation identity");
        wait_for_wake(&wake);
        drop(controller);
        drop_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("dropping an unread completion releases its event context");
    }
}
