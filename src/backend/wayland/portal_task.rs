//! Owned capacity-one task used by frozen and zoom portal fallbacks.

use std::future::Future;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};
use std::time::{Duration, Instant};

use super::RuntimeWakeSender;

pub(super) const PORTAL_CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

enum PortalMessage<T> {
    Ready(T),
    Failed(String),
}

pub(super) enum PortalPoll<T> {
    Pending,
    Ready(T),
    Failed(String),
    Disconnected,
}

pub(super) struct PortalTask<T> {
    receiver: Receiver<PortalMessage<T>>,
    worker: Option<PortalWorker>,
    started_at: Instant,
}

struct PortalWorker {
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
    _supervisor: tokio::task::JoinHandle<()>,
}

impl PortalWorker {
    fn cancel(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

impl<T> PortalTask<T>
where
    T: Send + 'static,
{
    pub(super) fn spawn(
        runtime: &tokio::runtime::Handle,
        runtime_wake: RuntimeWakeSender,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Self {
        Self::spawn_at(runtime, runtime_wake, Instant::now(), future)
    }

    fn spawn_at(
        runtime: &tokio::runtime::Handle,
        runtime_wake: RuntimeWakeSender,
        started_at: Instant,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Self {
        let task = runtime.spawn(future);
        Self::spawn_joined_at(runtime, runtime_wake, started_at, task)
    }

    fn spawn_joined_at(
        runtime: &tokio::runtime::Handle,
        runtime_wake: RuntimeWakeSender,
        started_at: Instant,
        mut task: tokio::task::JoinHandle<T>,
    ) -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let (cancel, mut cancellation) = tokio::sync::oneshot::channel();
        let supervisor = runtime.spawn(async move {
            let reporter = PortalTaskReporter::new(sender, runtime_wake);
            tokio::select! {
                biased;
                _ = &mut cancellation => {
                    task.abort();
                    let _ = task.await;
                    reporter.cancel();
                }
                result = &mut task => {
                    let message = match result {
                        Ok(value) => PortalMessage::Ready(value),
                        Err(error) => {
                            PortalMessage::Failed(format!("portal task failed: {error}"))
                        }
                    };
                    reporter.publish(message);
                }
            }
        });
        Self {
            receiver,
            worker: Some(PortalWorker {
                cancel: Some(cancel),
                _supervisor: supervisor,
            }),
            started_at,
        }
    }

    #[cfg(test)]
    pub(super) fn spawn_at_for_test(
        runtime: &tokio::runtime::Handle,
        runtime_wake: RuntimeWakeSender,
        started_at: Instant,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Self {
        Self::spawn_at(runtime, runtime_wake, started_at, future)
    }

    #[cfg(test)]
    pub(super) fn disconnected_for_test(started_at: Instant) -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        drop(sender);
        Self {
            receiver,
            worker: None,
            started_at,
        }
    }

    #[cfg(test)]
    pub(super) fn aborted_for_test(
        runtime: &tokio::runtime::Handle,
        runtime_wake: RuntimeWakeSender,
    ) -> Self {
        let task = runtime.spawn(std::future::pending::<T>());
        task.abort();
        Self::spawn_joined_at(runtime, runtime_wake, Instant::now(), task)
    }

    #[cfg(test)]
    pub(super) fn unexpected_supervisor_exit_for_test(
        runtime: &tokio::runtime::Handle,
        runtime_wake: RuntimeWakeSender,
    ) -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let (cancel, cancellation) = tokio::sync::oneshot::channel();
        let supervisor = runtime.spawn(async move {
            let _cancellation = cancellation;
            let _reporter = PortalTaskReporter::new(sender, runtime_wake);
        });
        Self {
            receiver,
            worker: Some(PortalWorker {
                cancel: Some(cancel),
                _supervisor: supervisor,
            }),
            started_at: Instant::now(),
        }
    }

    pub(super) fn poll(&mut self) -> PortalPoll<T> {
        match self.receiver.try_recv() {
            Ok(PortalMessage::Ready(value)) => PortalPoll::Ready(value),
            Ok(PortalMessage::Failed(reason)) => PortalPoll::Failed(reason),
            Err(TryRecvError::Empty) => PortalPoll::Pending,
            Err(TryRecvError::Disconnected) => PortalPoll::Disconnected,
        }
    }

    pub(super) fn timeout(&self, now: Instant) -> Duration {
        self.started_at
            .checked_add(PORTAL_CAPTURE_TIMEOUT)
            .map(|deadline| deadline.saturating_duration_since(now))
            .unwrap_or(Duration::ZERO)
    }

    pub(super) fn timed_out(&self, now: Instant) -> bool {
        self.timeout(now).is_zero()
    }

    pub(super) fn cancel(&mut self) {
        if let Some(worker) = &mut self.worker {
            worker.cancel();
        }
    }
}

impl<T> Drop for PortalTask<T> {
    fn drop(&mut self) {
        if let Some(worker) = &mut self.worker {
            worker.cancel();
        }
    }
}

struct PortalTaskReporter<T> {
    sender: Option<SyncSender<PortalMessage<T>>>,
    runtime_wake: RuntimeWakeSender,
}

impl<T> PortalTaskReporter<T> {
    fn new(sender: SyncSender<PortalMessage<T>>, runtime_wake: RuntimeWakeSender) -> Self {
        Self {
            sender: Some(sender),
            runtime_wake,
        }
    }

    fn publish(mut self, message: PortalMessage<T>) {
        self.publish_and_wake(message);
    }

    fn cancel(mut self) {
        self.sender.take();
    }

    fn publish_and_wake(&mut self, message: PortalMessage<T>) {
        let Some(sender) = self.sender.take() else {
            return;
        };
        let publication = sender.try_send(message);
        drop(sender);
        match publication {
            Ok(()) => {
                if let Err(error) = self.runtime_wake.wake() {
                    log::error!("Failed to wake runtime for portal completion: {error}");
                }
            }
            Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(_)) => {
                log::error!("Portal task terminal channel was already full");
                if let Err(error) = self.runtime_wake.wake() {
                    log::error!("Failed to wake runtime for queued portal completion: {error}");
                }
            }
        }
    }
}

impl<T> Drop for PortalTaskReporter<T> {
    fn drop(&mut self) {
        if self.sender.is_some() {
            self.publish_and_wake(PortalMessage::Failed(
                "portal task supervisor exited without a terminal result".to_string(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;
    use std::sync::mpsc;

    use super::*;
    use crate::backend::wayland::{RuntimeWakeSender, RuntimeWakeSource};

    fn runtime_sender(wake: &RuntimeWakeSource) -> RuntimeWakeSender {
        wake.try_sender()
            .expect("test duplicates its portal-task runtime eventfd")
    }

    fn wait_for_wake(wake: &RuntimeWakeSource) {
        let mut pollfd = libc::pollfd {
            fd: wake.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: the descriptor and pollfd remain valid during the bounded wait.
        assert_eq!(unsafe { libc::poll(&mut pollfd, 1, 1_000) }, 1);
    }

    async fn wait_for_disconnect<T>(task: &mut PortalTask<T>) -> bool
    where
        T: Send + 'static,
    {
        for _ in 0..100 {
            match task.poll() {
                PortalPoll::Disconnected => return true,
                PortalPoll::Pending => tokio::task::yield_now().await,
                PortalPoll::Ready(_) | PortalPoll::Failed(_) => return false,
            }
        }
        false
    }

    struct DropNotice(mpsc::Sender<()>);

    impl Drop for DropNotice {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn publishes_and_drops_the_sender_before_waking() {
        let wake = RuntimeWakeSource::new().expect("test creates a portal-task runtime eventfd");
        let mut task = PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            runtime_sender(&wake),
            async { 17 },
        );
        wait_for_wake(&wake);
        assert!(matches!(task.poll(), PortalPoll::Ready(17)));
        assert!(matches!(task.poll(), PortalPoll::Disconnected));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn aborted_child_is_an_explicit_failure_before_wake() {
        let wake = RuntimeWakeSource::new().expect("test creates a portal-task runtime eventfd");
        let mut task = PortalTask::<u64>::aborted_for_test(
            &tokio::runtime::Handle::current(),
            runtime_sender(&wake),
        );
        wait_for_wake(&wake);
        assert!(matches!(
            task.poll(),
            PortalPoll::Failed(reason) if reason.starts_with("portal task failed:")
        ));
        assert!(matches!(task.poll(), PortalPoll::Disconnected));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn unexpected_supervisor_exit_is_a_typed_failure_before_wake() {
        let wake = RuntimeWakeSource::new().expect("test creates a portal-task runtime eventfd");
        let mut task = PortalTask::<u64>::unexpected_supervisor_exit_for_test(
            &tokio::runtime::Handle::current(),
            runtime_sender(&wake),
        );
        wait_for_wake(&wake);
        assert!(matches!(
            task.poll(),
            PortalPoll::Failed(reason)
                if reason == "portal task supervisor exited without a terminal result"
        ));
        assert!(matches!(task.poll(), PortalPoll::Disconnected));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn expected_cancel_drops_the_child_without_waking_failure() {
        let wake = RuntimeWakeSource::new().expect("test creates a portal-task runtime eventfd");
        let (drop_sender, drop_receiver) = mpsc::channel();
        let drop_notice = DropNotice(drop_sender);
        let mut task = PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            runtime_sender(&wake),
            async move {
                let _drop_notice = drop_notice;
                std::future::pending::<()>().await;
                1
            },
        );
        task.cancel();
        drop_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("test cancellation drops its pending portal future");
        let mut pollfd = libc::pollfd {
            fd: wake.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: the descriptor and pollfd are valid for this non-blocking poll.
        assert_eq!(unsafe { libc::poll(&mut pollfd, 1, 0) }, 0);
        assert!(wait_for_disconnect(&mut task).await);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn owner_drop_requests_silent_teardown() {
        let wake = RuntimeWakeSource::new().expect("test creates a portal-task runtime eventfd");
        let (drop_sender, drop_receiver) = mpsc::channel();
        let drop_notice = DropNotice(drop_sender);
        let task = PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            runtime_sender(&wake),
            async move {
                let _drop_notice = drop_notice;
                std::future::pending::<()>().await;
            },
        );

        drop(task);

        drop_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("test owner drop tears down its pending portal future");
        let mut pollfd = libc::pollfd {
            fd: wake.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: the descriptor and pollfd are valid for this non-blocking poll.
        assert_eq!(unsafe { libc::poll(&mut pollfd, 1, 0) }, 0);
    }

    #[tokio::test]
    async fn deadline_uses_injected_start_instant() {
        let wake = RuntimeWakeSource::new().expect("test creates a portal-task runtime eventfd");
        let start = Instant::now();
        let task = PortalTask::spawn_at(
            &tokio::runtime::Handle::current(),
            runtime_sender(&wake),
            start,
            std::future::pending::<()>(),
        );
        assert_eq!(task.timeout(start), PORTAL_CAPTURE_TIMEOUT);
        assert!(task.timed_out(start + PORTAL_CAPTURE_TIMEOUT));
    }
}
