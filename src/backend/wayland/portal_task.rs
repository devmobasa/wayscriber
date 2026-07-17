//! Owned capacity-one task used by frozen and zoom portal fallbacks.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};
use std::time::{Duration, Instant};

use super::RuntimeWakeHandle;

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
    worker: Option<tokio::task::JoinHandle<()>>,
    expected_cancel: Arc<AtomicBool>,
    started_at: Instant,
}

impl<T> PortalTask<T>
where
    T: Send + 'static,
{
    pub(super) fn spawn(
        runtime: &tokio::runtime::Handle,
        runtime_wake: RuntimeWakeHandle,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Self {
        Self::spawn_at(runtime, runtime_wake, Instant::now(), future)
    }

    fn spawn_at(
        runtime: &tokio::runtime::Handle,
        runtime_wake: RuntimeWakeHandle,
        started_at: Instant,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let expected_cancel = Arc::new(AtomicBool::new(false));
        let task_cancel = Arc::clone(&expected_cancel);
        let worker = runtime.spawn(async move {
            let guard = PortalTaskExitGuard::new(sender, runtime_wake, task_cancel);
            let mut tasks = tokio::task::JoinSet::new();
            tasks.spawn(future);
            let message = match tasks.join_next().await {
                Some(Ok(value)) => PortalMessage::Ready(value),
                Some(Err(error)) => PortalMessage::Failed(format!("portal task failed: {error}")),
                None => PortalMessage::Failed("portal task ended without a result".to_string()),
            };
            guard.publish(message);
        });
        Self {
            receiver,
            worker: Some(worker),
            expected_cancel,
            started_at,
        }
    }

    #[cfg(test)]
    pub(super) fn spawn_at_for_test(
        runtime: &tokio::runtime::Handle,
        runtime_wake: RuntimeWakeHandle,
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
            expected_cancel: Arc::new(AtomicBool::new(false)),
            started_at,
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
        self.expected_cancel.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
    }
}

impl<T> Drop for PortalTask<T> {
    fn drop(&mut self) {
        self.expected_cancel.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
    }
}

struct PortalTaskExitGuard<T> {
    sender: Option<SyncSender<PortalMessage<T>>>,
    runtime_wake: RuntimeWakeHandle,
    expected_cancel: Arc<AtomicBool>,
    terminal_published: bool,
}

impl<T> PortalTaskExitGuard<T> {
    fn new(
        sender: SyncSender<PortalMessage<T>>,
        runtime_wake: RuntimeWakeHandle,
        expected_cancel: Arc<AtomicBool>,
    ) -> Self {
        Self {
            sender: Some(sender),
            runtime_wake,
            expected_cancel,
            terminal_published: false,
        }
    }

    fn publish(mut self, message: PortalMessage<T>) {
        let result = self
            .sender
            .as_ref()
            .expect("portal sender retained until terminal publication")
            .try_send(message);
        self.sender.take();
        self.terminal_published = true;
        match result {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(_)) => {
                log::error!("Portal task found an impossible full terminal channel");
            }
        }
        if let Err(error) = self.runtime_wake.wake() {
            log::error!("Failed to wake runtime for portal completion: {error}");
        }
    }
}

impl<T> Drop for PortalTaskExitGuard<T> {
    fn drop(&mut self) {
        if self.terminal_published {
            return;
        }
        self.sender.take();
        if !self.expected_cancel.load(Ordering::Acquire)
            && let Err(error) = self.runtime_wake.wake()
        {
            log::error!("Failed to wake runtime for disconnected portal task: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;

    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;

    fn wait_for_wake(wake: &RuntimeWakeSource) {
        let mut pollfd = libc::pollfd {
            fd: wake.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: the descriptor and pollfd remain valid during the bounded wait.
        assert_eq!(unsafe { libc::poll(&mut pollfd, 1, 1_000) }, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn publishes_before_waking() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut task =
            PortalTask::spawn(&tokio::runtime::Handle::current(), wake.handle(), async {
                17
            });
        wait_for_wake(&wake);
        assert!(matches!(task.poll(), PortalPoll::Ready(17)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn panic_is_an_explicit_failure_before_wake() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut task =
            PortalTask::<u64>::spawn(&tokio::runtime::Handle::current(), wake.handle(), async {
                panic!("expected portal panic")
            });
        wait_for_wake(&wake);
        assert!(matches!(
            task.poll(),
            PortalPoll::Failed(reason) if reason.contains("expected portal panic")
        ));
    }

    #[tokio::test]
    async fn expected_cancel_aborts_without_waking_failure() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut task = PortalTask::spawn(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            async move {
                std::future::pending::<()>().await;
                1
            },
        );
        task.cancel();
        tokio::task::yield_now().await;
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
        let wake = RuntimeWakeSource::new().unwrap();
        let start = Instant::now();
        let task = PortalTask::spawn_at(
            &tokio::runtime::Handle::current(),
            wake.handle(),
            start,
            std::future::pending::<()>(),
        );
        assert_eq!(task.timeout(start), PORTAL_CAPTURE_TIMEOUT);
        assert!(task.timed_out(start + PORTAL_CAPTURE_TIMEOUT));
    }
}
