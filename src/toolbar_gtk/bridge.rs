//! Bounded, wake-driven transport and lifecycle ownership for the GTK toolbar thread.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::backend::wayland::RuntimeWakeHandle;

use super::{GtkToolbarDragPhase, GtkToolbarFeedback, GtkToolbarKind, GtkToolbarUpdate};

pub(super) const GTK_FEEDBACK_CAPACITY: usize = 64;
pub(super) const GTK_FEEDBACK_DRAIN_LIMIT: usize = 64;
const GTK_THREAD_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

const STATUS_STARTING: u8 = 0;
const STATUS_READY: u8 = 1;
const STATUS_FAILED: u8 = 2;
const STATUS_STOPPING: u8 = 3;
const STATUS_STOPPED: u8 = 4;

#[derive(Clone)]
pub(super) struct BridgeHealth {
    status: Arc<AtomicU8>,
    runtime_wake: RuntimeWakeHandle,
}

impl BridgeHealth {
    pub(super) fn new(runtime_wake: RuntimeWakeHandle) -> Self {
        Self {
            status: Arc::new(AtomicU8::new(STATUS_STARTING)),
            runtime_wake,
        }
    }

    pub(super) fn mark_ready(&self) -> bool {
        self.status
            .compare_exchange(
                STATUS_STARTING,
                STATUS_READY,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    pub(super) fn fail(&self, reason: impl AsRef<str>) {
        let mut current = self.status.load(Ordering::Acquire);
        loop {
            match current {
                STATUS_FAILED | STATUS_STOPPING | STATUS_STOPPED => return,
                STATUS_STARTING | STATUS_READY => {
                    match self.status.compare_exchange_weak(
                        current,
                        STATUS_FAILED,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => {
                            log::warn!("{}", reason.as_ref());
                            if let Err(err) = self.runtime_wake.wake() {
                                log::error!("Failed to wake runtime for GTK terminal state: {err}");
                            }
                            return;
                        }
                        Err(observed) => current = observed,
                    }
                }
                _ => unreachable!("unknown GTK bridge health state {current}"),
            }
        }
    }

    fn begin_stopping(&self) {
        let mut current = self.status.load(Ordering::Acquire);
        while matches!(current, STATUS_STARTING | STATUS_READY) {
            match self.status.compare_exchange_weak(
                current,
                STATUS_STOPPING,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(observed) => current = observed,
            }
        }
    }

    pub(super) fn stopping(&self) -> bool {
        matches!(
            self.status.load(Ordering::Acquire),
            STATUS_STOPPING | STATUS_STOPPED
        )
    }

    fn mark_stopped(&self) {
        let _ = self.status.compare_exchange(
            STATUS_STOPPING,
            STATUS_STOPPED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    pub(super) fn failed(&self) -> bool {
        self.status.load(Ordering::Acquire) == STATUS_FAILED
    }

    fn wake_owner(&self) -> std::io::Result<()> {
        self.runtime_wake.wake()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FeedbackPublishError {
    Closed,
    Failed,
}

struct FeedbackMailboxState {
    queue: VecDeque<GtkToolbarFeedback>,
    accepting: bool,
}

struct FeedbackMailbox {
    state: Mutex<FeedbackMailboxState>,
    health: BridgeHealth,
}

#[derive(Clone)]
pub(super) struct FeedbackPublisher {
    mailbox: Arc<FeedbackMailbox>,
}

impl FeedbackPublisher {
    fn new(health: BridgeHealth) -> Self {
        Self {
            mailbox: Arc::new(FeedbackMailbox {
                state: Mutex::new(FeedbackMailboxState {
                    queue: VecDeque::with_capacity(GTK_FEEDBACK_CAPACITY),
                    accepting: true,
                }),
                health,
            }),
        }
    }

    pub(super) fn publish(&self, feedback: GtkToolbarFeedback) -> Result<(), FeedbackPublishError> {
        let mut overflowed = false;
        {
            let mut state = match self.mailbox.state.lock() {
                Ok(state) => state,
                Err(poisoned) => {
                    let mut state = poisoned.into_inner();
                    state.accepting = false;
                    drop(state);
                    self.mailbox.health.fail(
                        "GTK feedback mailbox state was poisoned; restoring built-in toolbars",
                    );
                    return Err(FeedbackPublishError::Failed);
                }
            };
            if !state.accepting {
                return Err(if self.mailbox.health.failed() {
                    FeedbackPublishError::Failed
                } else {
                    FeedbackPublishError::Closed
                });
            }

            if state.queue.len() < GTK_FEEDBACK_CAPACITY {
                state.queue.push_back(feedback);
            } else if let Some(kind) = feedback.move_kind() {
                let mut replacement = None;
                for (index, queued) in state.queue.iter().enumerate().rev() {
                    if queued.is_non_coalescible_boundary() {
                        break;
                    }
                    if queued.move_kind() == Some(kind) {
                        replacement = Some(index);
                        break;
                    }
                }
                match replacement {
                    Some(index) => state.queue[index] = feedback,
                    None => {
                        state.accepting = false;
                        overflowed = true;
                    }
                }
            } else {
                let reclaim = feedback
                    .drag_kind()
                    .and_then(|kind| oldest_move_in_current_segment(&state.queue, kind))
                    .or_else(|| {
                        state
                            .queue
                            .iter()
                            .position(|queued| queued.move_kind().is_some())
                    });
                if let Some(index) = reclaim {
                    state.queue.remove(index);
                    state.queue.push_back(feedback);
                } else {
                    state.accepting = false;
                    overflowed = true;
                }
            }
        }

        if overflowed {
            self.mailbox.health.fail(
                "GTK feedback mailbox exhausted by ordered feedback; restoring built-in toolbars",
            );
            return Err(FeedbackPublishError::Failed);
        }

        if let Err(err) = self.mailbox.health.wake_owner() {
            self.close_admission();
            self.mailbox.health.fail(format!(
                "GTK feedback could not wake the runtime ({err}); restoring built-in toolbars"
            ));
            return Err(FeedbackPublishError::Failed);
        }
        Ok(())
    }

    fn drain(&self, limit: usize) -> Vec<GtkToolbarFeedback> {
        let (drained, residual) = {
            let mut state = match self.mailbox.state.lock() {
                Ok(state) => state,
                Err(poisoned) => {
                    let mut state = poisoned.into_inner();
                    state.accepting = false;
                    drop(state);
                    self.mailbox.health.fail(
                        "GTK feedback mailbox state was poisoned; restoring built-in toolbars",
                    );
                    return Vec::new();
                }
            };
            let take = limit.min(state.queue.len());
            let drained = state.queue.drain(..take).collect::<Vec<_>>();
            (drained, !state.queue.is_empty())
        };
        if residual && let Err(err) = self.mailbox.health.wake_owner() {
            self.close_admission();
            self.mailbox.health.fail(format!(
                "Residual GTK feedback could not wake the runtime ({err}); restoring built-in toolbars"
            ));
        }
        drained
    }

    fn close_admission(&self) {
        match self.mailbox.state.lock() {
            Ok(mut state) => state.accepting = false,
            Err(poisoned) => poisoned.into_inner().accepting = false,
        }
    }

    #[cfg(test)]
    fn pending_len(&self) -> usize {
        self.mailbox
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .queue
            .len()
    }
}

impl GtkToolbarFeedback {
    fn move_kind(&self) -> Option<GtkToolbarKind> {
        match self {
            Self::SetTopOffset {
                phase: GtkToolbarDragPhase::Move,
                ..
            } => Some(GtkToolbarKind::Top),
            Self::SetSideOffset {
                phase: GtkToolbarDragPhase::Move,
                ..
            } => Some(GtkToolbarKind::Side),
            _ => None,
        }
    }

    fn is_non_coalescible_boundary(&self) -> bool {
        self.move_kind().is_none()
    }

    fn drag_kind(&self) -> Option<GtkToolbarKind> {
        match self {
            Self::SetTopOffset { .. } => Some(GtkToolbarKind::Top),
            Self::SetSideOffset { .. } => Some(GtkToolbarKind::Side),
            Self::Event { .. } => None,
        }
    }
}

fn oldest_move_in_current_segment(
    queue: &VecDeque<GtkToolbarFeedback>,
    kind: GtkToolbarKind,
) -> Option<usize> {
    let segment_start = queue
        .iter()
        .rposition(GtkToolbarFeedback::is_non_coalescible_boundary)
        .map_or(0, |index| index + 1);
    queue
        .iter()
        .enumerate()
        .skip(segment_start)
        .find_map(|(index, feedback)| (feedback.move_kind() == Some(kind)).then_some(index))
}

struct LatestValueSender<T> {
    sender: async_channel::Sender<T>,
    last_sent: Option<T>,
}

impl<T: Clone + PartialEq> LatestValueSender<T> {
    fn new(sender: async_channel::Sender<T>) -> Self {
        Self {
            sender,
            last_sent: None,
        }
    }

    fn publish(&mut self, value: T) -> Result<bool, async_channel::SendError<T>> {
        if self.last_sent.as_ref() == Some(&value) {
            return Ok(false);
        }
        self.sender.force_send(value.clone())?;
        self.last_sent = Some(value);
        Ok(true)
    }

    fn close(&self) {
        self.sender.close();
    }
}

struct ThreadCompletion(std::sync::mpsc::Sender<()>);

impl Drop for ThreadCompletion {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThreadShutdownOutcome {
    Joined,
    Panicked,
    TimedOut,
}

fn finish_thread_within(
    thread: &mut Option<JoinHandle<()>>,
    completion: &std::sync::mpsc::Receiver<()>,
    timeout: Duration,
) -> ThreadShutdownOutcome {
    let Some(handle) = thread.as_ref() else {
        return ThreadShutdownOutcome::Joined;
    };
    let deadline = Instant::now() + timeout;
    let completion_wait = deadline.saturating_duration_since(Instant::now());
    let _ = completion.recv_timeout(completion_wait);
    while !handle.is_finished() && Instant::now() < deadline {
        std::thread::yield_now();
    }
    if !handle.is_finished() {
        thread.take();
        return ThreadShutdownOutcome::TimedOut;
    }
    match thread.take().expect("thread checked above").join() {
        Ok(()) => ThreadShutdownOutcome::Joined,
        Err(_) => ThreadShutdownOutcome::Panicked,
    }
}

/// Main-thread owner of the GTK toolbar bridge and GTK thread.
pub struct GtkToolbarBridge {
    updates: LatestValueSender<GtkToolbarUpdate>,
    feedback: FeedbackPublisher,
    health: BridgeHealth,
    thread: Option<JoinHandle<()>>,
    completion: std::sync::mpsc::Receiver<()>,
}

impl GtkToolbarBridge {
    /// Spawns the GTK thread. Returns `None` only when the OS thread cannot be
    /// created; GTK-level failures are published asynchronously and wake the runtime.
    pub fn spawn(runtime_wake: RuntimeWakeHandle) -> Option<Self> {
        let (update_tx, update_rx) = async_channel::bounded(1);
        let (completion_tx, completion_rx) = std::sync::mpsc::channel();
        let health = BridgeHealth::new(runtime_wake);
        let feedback = FeedbackPublisher::new(health.clone());
        let thread_health = health.clone();
        let thread_feedback = feedback.clone();
        let spawned = std::thread::Builder::new()
            .name("gtk-toolbar".into())
            .spawn(move || {
                let _completion = ThreadCompletion(completion_tx);
                super::runtime::run(update_rx, thread_feedback, thread_health);
            });
        match spawned {
            Ok(thread) => Some(Self {
                updates: LatestValueSender::new(update_tx),
                feedback,
                health,
                thread: Some(thread),
                completion: completion_rx,
            }),
            Err(err) => {
                log::error!("Failed to spawn GTK toolbar thread: {err}");
                None
            }
        }
    }

    /// True once the GTK thread or bounded feedback bridge reported terminal failure.
    pub fn failed(&self) -> bool {
        self.health.failed()
    }

    /// Drains one bounded pass. Residual feedback retains a coalesced runtime wake.
    pub fn drain_feedback(&self) -> Vec<GtkToolbarFeedback> {
        self.feedback.drain(GTK_FEEDBACK_DRAIN_LIMIT)
    }

    /// Publishes the newest complete update and replaces an unread older update.
    pub fn maybe_send(&mut self, update: GtkToolbarUpdate) {
        if self.updates.publish(update).is_err() {
            self.health
                .fail("GTK toolbar update receiver disconnected; restoring built-in toolbars");
        }
    }
}

impl Drop for GtkToolbarBridge {
    fn drop(&mut self) {
        self.feedback.close_admission();
        self.health.begin_stopping();
        self.updates.close();
        match finish_thread_within(
            &mut self.thread,
            &self.completion,
            GTK_THREAD_SHUTDOWN_TIMEOUT,
        ) {
            ThreadShutdownOutcome::Joined => self.health.mark_stopped(),
            ThreadShutdownOutcome::Panicked => self
                .health
                .fail("GTK toolbar thread panicked during shutdown"),
            ThreadShutdownOutcome::TimedOut => {
                log::warn!(
                    "GTK toolbar thread did not stop within {:?}; detaching it safely",
                    GTK_THREAD_SHUTDOWN_TIMEOUT
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;
    use std::sync::mpsc;

    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;
    use crate::ui::toolbar::ToolbarEvent;

    const SURFACE: super::super::GtkToolbarSurfaceSize = super::super::GtkToolbarSurfaceSize {
        width: 200,
        height: 80,
    };

    fn drag(kind: GtkToolbarKind, phase: GtkToolbarDragPhase, seq: u64) -> GtkToolbarFeedback {
        match kind {
            GtkToolbarKind::Top => GtkToolbarFeedback::SetTopOffset {
                x: seq as f64,
                y: 0.0,
                surface_size: SURFACE,
                seq,
                phase,
            },
            GtkToolbarKind::Side => GtkToolbarFeedback::SetSideOffset {
                x: 0.0,
                y: seq as f64,
                surface_size: SURFACE,
                seq,
                phase,
            },
        }
    }

    fn event() -> GtkToolbarFeedback {
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::Undo,
            rebind_requested: false,
        }
    }

    fn channel() -> (RuntimeWakeSource, BridgeHealth, FeedbackPublisher) {
        let wake = RuntimeWakeSource::new().unwrap();
        let health = BridgeHealth::new(wake.handle());
        let publisher = FeedbackPublisher::new(health.clone());
        (wake, health, publisher)
    }

    fn wake_is_readable(source: &RuntimeWakeSource) -> bool {
        let mut pollfd = libc::pollfd {
            fd: source.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: pollfd and the source descriptor are valid for this non-blocking poll.
        let ready = unsafe { libc::poll(&mut pollfd, 1, 0) };
        assert!(ready >= 0);
        ready > 0 && pollfd.revents & libc::POLLIN != 0
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
            assert!(
                Instant::now() < deadline,
                "polling thread did not block (last state: {state:?})"
            );
            std::thread::yield_now();
        }
    }

    #[test]
    fn latest_value_replaces_unread_state_and_sender_drop_is_observable() {
        let (tx, rx) = async_channel::bounded(1);
        let mut latest = LatestValueSender::new(tx);
        assert_eq!(latest.publish(1), Ok(true));
        assert_eq!(latest.publish(2), Ok(true));
        assert_eq!(latest.publish(2), Ok(false));
        assert_eq!(rx.recv_blocking(), Ok(2));
        drop(latest);
        assert!(rx.recv_blocking().is_err());
    }

    #[test]
    fn compatible_move_replaces_only_within_the_current_segment() {
        let (_wake, health, publisher) = channel();
        publisher
            .publish(drag(GtkToolbarKind::Top, GtkToolbarDragPhase::Start, 1))
            .unwrap();
        for seq in 2..=GTK_FEEDBACK_CAPACITY as u64 {
            publisher
                .publish(drag(GtkToolbarKind::Top, GtkToolbarDragPhase::Move, seq))
                .unwrap();
        }
        publisher
            .publish(drag(GtkToolbarKind::Top, GtkToolbarDragPhase::Move, 100))
            .unwrap();

        let drained = publisher.drain(GTK_FEEDBACK_CAPACITY);
        assert_eq!(drained.len(), GTK_FEEDBACK_CAPACITY);
        assert_eq!(
            drained.last(),
            Some(&drag(GtkToolbarKind::Top, GtkToolbarDragPhase::Move, 100))
        );
        assert!(!health.failed());
    }

    #[test]
    fn drag_end_reclaims_an_older_move_and_preserves_ordered_boundaries() {
        let (_wake, health, publisher) = channel();
        let start = drag(GtkToolbarKind::Top, GtkToolbarDragPhase::Start, 1);
        publisher.publish(start.clone()).unwrap();
        for seq in 2..=GTK_FEEDBACK_CAPACITY as u64 {
            publisher
                .publish(drag(GtkToolbarKind::Top, GtkToolbarDragPhase::Move, seq))
                .unwrap();
        }
        let end = drag(GtkToolbarKind::Top, GtkToolbarDragPhase::End, 65);
        publisher.publish(end.clone()).unwrap();

        let drained = publisher.drain(GTK_FEEDBACK_CAPACITY);
        assert_eq!(drained.len(), GTK_FEEDBACK_CAPACITY);
        assert_eq!(drained.first(), Some(&start));
        assert_eq!(drained.last(), Some(&end));
        assert!(!drained.contains(&drag(GtkToolbarKind::Top, GtkToolbarDragPhase::Move, 2)));
        assert!(!health.failed());
    }

    #[test]
    fn move_never_crosses_an_event_boundary_or_toolbar_kind() {
        let (_wake, health, publisher) = channel();
        for seq in 0..(GTK_FEEDBACK_CAPACITY - 2) as u64 {
            publisher
                .publish(drag(GtkToolbarKind::Top, GtkToolbarDragPhase::Move, seq))
                .unwrap();
        }
        publisher.publish(event()).unwrap();
        publisher
            .publish(drag(GtkToolbarKind::Side, GtkToolbarDragPhase::Move, 1))
            .unwrap();

        assert_eq!(
            publisher.publish(drag(GtkToolbarKind::Top, GtkToolbarDragPhase::Move, 999)),
            Err(FeedbackPublishError::Failed)
        );
        assert!(health.failed());
        assert_eq!(publisher.pending_len(), GTK_FEEDBACK_CAPACITY);
    }

    #[test]
    fn non_coalescible_capacity_exhaustion_publishes_failure_before_wake() {
        let (wake, health, publisher) = channel();
        for _ in 0..GTK_FEEDBACK_CAPACITY {
            publisher.publish(event()).unwrap();
        }
        wake.drain().unwrap();
        assert!(!wake_is_readable(&wake));

        assert_eq!(
            publisher.publish(event()),
            Err(FeedbackPublishError::Failed)
        );
        assert!(health.failed());
        assert!(wake_is_readable(&wake));
        assert_eq!(
            publisher.publish(event()),
            Err(FeedbackPublishError::Failed)
        );
    }

    #[test]
    fn feedback_is_committed_before_its_runtime_wake() {
        let (wake, _health, publisher) = channel();
        publisher.publish(event()).unwrap();
        assert!(wake_is_readable(&wake));
        assert_eq!(publisher.pending_len(), 1);
        assert_eq!(publisher.drain(1), vec![event()]);
    }

    #[test]
    fn feedback_unblocks_a_runtime_already_waiting_in_poll() {
        let (wake, _health, publisher) = channel();
        let (tid_tx, tid_rx) = mpsc::channel();
        let poller = std::thread::spawn(move || {
            // SAFETY: gettid has no preconditions and is used only to observe this test thread.
            let tid = unsafe { libc::syscall(libc::SYS_gettid) as libc::pid_t };
            tid_tx.send(tid).unwrap();
            let mut pollfd = libc::pollfd {
                fd: wake.poll_fd().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: pollfd and the source descriptor remain valid for the bounded wait.
            let ready = unsafe { libc::poll(&mut pollfd, 1, 1_000) };
            assert_eq!(ready, 1);
            assert_ne!(pollfd.revents & libc::POLLIN, 0);
        });

        let tid = tid_rx.recv().unwrap();
        wait_until_thread_is_blocked_in_poll(tid);
        publisher.publish(event()).unwrap();
        poller.join().unwrap();
        assert_eq!(publisher.pending_len(), 1);
    }

    #[test]
    fn bounded_drain_preserves_order_and_self_wakes_for_residual_work() {
        let (wake, _health, publisher) = channel();
        for seq in 1..=3 {
            publisher
                .publish(drag(GtkToolbarKind::Top, GtkToolbarDragPhase::Move, seq))
                .unwrap();
        }
        wake.drain().unwrap();

        assert_eq!(
            publisher.drain(2),
            vec![
                drag(GtkToolbarKind::Top, GtkToolbarDragPhase::Move, 1),
                drag(GtkToolbarKind::Top, GtkToolbarDragPhase::Move, 2),
            ]
        );
        assert_eq!(publisher.pending_len(), 1);
        assert!(wake_is_readable(&wake));
    }

    #[test]
    fn completed_thread_is_joined_without_initializing_gtk() {
        let (completion_tx, completion_rx) = mpsc::channel();
        let mut thread = Some(std::thread::spawn(move || {
            let _completion = ThreadCompletion(completion_tx);
        }));
        assert_eq!(
            finish_thread_within(&mut thread, &completion_rx, Duration::from_secs(1)),
            ThreadShutdownOutcome::Joined
        );
        assert!(thread.is_none());
    }

    #[test]
    fn stuck_thread_is_detached_at_the_bounded_deadline() {
        let (completion_tx, completion_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let mut thread = Some(std::thread::spawn(move || {
            let _completion = ThreadCompletion(completion_tx);
            let _ = release_rx.recv();
        }));
        assert_eq!(
            finish_thread_within(&mut thread, &completion_rx, Duration::from_millis(1)),
            ThreadShutdownOutcome::TimedOut
        );
        assert!(thread.is_none());
        release_tx.send(()).unwrap();
    }
}
