//! Bounded, wake-driven transport and lifecycle ownership for the GTK toolbar thread.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::thread::JoinHandle;

use tokio::sync::{oneshot, watch};

use crate::backend::wayland::RuntimeWakeSender;
use crate::config::ToolbarRebindModifier;
use crate::ui::toolbar::ToolbarEvent;

use super::{GtkToolbarDragPhase, GtkToolbarFeedback, GtkToolbarKind, GtkToolbarUpdate};

pub(super) const GTK_FEEDBACK_CAPACITY: usize = 64;
pub(super) const GTK_FEEDBACK_DRAIN_LIMIT: usize = 64;

/// Failure reporting endpoint owned by one GTK-thread component.
///
/// The receiver and the authoritative bridge state stay on the backend
/// thread. Each producer gets its own fallibly duplicated wake descriptor,
/// so no shared health flag or synchronized writer is needed.
pub(super) struct ThreadReporter {
    failures: Sender<String>,
    runtime_wake: RuntimeWakeSender,
}

impl ThreadReporter {
    fn new(failures: Sender<String>, runtime_wake: RuntimeWakeSender) -> Self {
        Self {
            failures,
            runtime_wake,
        }
    }

    fn wake_owner(&self) -> std::io::Result<()> {
        self.runtime_wake.wake()
    }

    fn report_failure(&self, reason: impl Into<String>) {
        let reason = reason.into();
        log::warn!("{reason}");
        if self.failures.send(reason).is_ok()
            && let Err(err) = self.wake_owner()
        {
            log::error!("Failed to wake runtime for GTK terminal state: {err}");
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FeedbackPublishError {
    Closed,
    Failed,
}

enum FeedbackMailboxCommand {
    Publish {
        feedback: GtkToolbarFeedback,
        reply: SyncSender<Result<(), FeedbackPublishError>>,
    },
    Drain {
        limit: usize,
        reply: SyncSender<Vec<GtkToolbarFeedback>>,
    },
    SetRebindState {
        modifier: ToolbarRebindModifier,
        active: bool,
    },
    CaptureClickModifiers(ClickModifiers),
    FinishPointerClick,
    PublishEvent {
        event: ToolbarEvent,
        reply: SyncSender<Result<(), FeedbackPublishError>>,
    },
    Shutdown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ClickModifiers {
    control: bool,
    shift: bool,
    alt: bool,
}

#[derive(Debug, Default)]
struct FeedbackClickState {
    rebind_modifier: ToolbarRebindModifier,
    backend_rebind_active: bool,
    click_rebind_requested: bool,
    click_in_progress: bool,
    click_shift: bool,
}

impl FeedbackClickState {
    fn set_rebind_state(&mut self, modifier: ToolbarRebindModifier, active: bool) {
        self.rebind_modifier = modifier;
        self.backend_rebind_active = active;
        if active {
            self.click_rebind_requested = true;
        } else if !self.click_in_progress {
            self.click_rebind_requested = false;
        }
    }

    fn capture_click_modifiers(&mut self, modifiers: ClickModifiers) {
        self.click_in_progress = true;
        self.click_shift = modifiers.shift;
        if self
            .rebind_modifier
            .matches(modifiers.control, modifiers.shift, modifiers.alt)
        {
            self.click_rebind_requested = true;
        }
    }

    fn finish_pointer_click(&mut self) {
        self.click_in_progress = false;
        self.click_shift = false;
        if !self.backend_rebind_active {
            self.click_rebind_requested = false;
        }
    }

    fn resolve_event(&mut self, event: ToolbarEvent) -> GtkToolbarFeedback {
        let rebind_requested = std::mem::take(&mut self.click_rebind_requested);
        let shift_click = std::mem::take(&mut self.click_shift);
        self.click_in_progress = false;
        // GTK runs on its own Wayland connection, so the backend cannot read
        // this click's modifiers. Resolve the Shift = instant-clear upgrade
        // at capture time and forward the resolved variant.
        let event = match event {
            ToolbarEvent::ClearCanvas { instant } => ToolbarEvent::ClearCanvas {
                instant: instant || shift_click,
            },
            other => other,
        };
        GtkToolbarFeedback::Event {
            event,
            rebind_requested,
        }
    }
}

/// GTK-thread feedback endpoint.
///
/// Widget callbacks send one request at a time to the root-owned mailbox
/// worker and wait for its typed result. The worker is the only place that
/// owns admission, ordering, and coalescing state.
#[derive(Clone)]
pub(super) struct FeedbackPublisher {
    commands: SyncSender<FeedbackMailboxCommand>,
}

impl FeedbackPublisher {
    fn new(commands: SyncSender<FeedbackMailboxCommand>) -> Self {
        Self { commands }
    }

    pub(super) fn publish(&self, feedback: GtkToolbarFeedback) -> Result<(), FeedbackPublishError> {
        let (reply, result) = sync_channel(0);
        if self
            .commands
            .send(FeedbackMailboxCommand::Publish { feedback, reply })
            .is_err()
        {
            return Err(FeedbackPublishError::Closed);
        }
        match result.recv() {
            Ok(result) => result,
            Err(_) => Err(FeedbackPublishError::Failed),
        }
    }

    pub(super) fn set_rebind_state(
        &self,
        modifier: ToolbarRebindModifier,
        active: bool,
    ) -> Result<(), FeedbackPublishError> {
        self.commands
            .send(FeedbackMailboxCommand::SetRebindState { modifier, active })
            .map_err(|_| FeedbackPublishError::Closed)
    }

    pub(super) fn capture_click_modifiers(
        &self,
        control: bool,
        shift: bool,
        alt: bool,
    ) -> Result<(), FeedbackPublishError> {
        self.commands
            .send(FeedbackMailboxCommand::CaptureClickModifiers(
                ClickModifiers {
                    control,
                    shift,
                    alt,
                },
            ))
            .map_err(|_| FeedbackPublishError::Closed)
    }

    pub(super) fn finish_pointer_click(&self) -> Result<(), FeedbackPublishError> {
        self.commands
            .send(FeedbackMailboxCommand::FinishPointerClick)
            .map_err(|_| FeedbackPublishError::Closed)
    }

    pub(super) fn publish_event(&self, event: ToolbarEvent) -> Result<(), FeedbackPublishError> {
        let (reply, result) = sync_channel(0);
        if self
            .commands
            .send(FeedbackMailboxCommand::PublishEvent { event, reply })
            .is_err()
        {
            return Err(FeedbackPublishError::Closed);
        }
        match result.recv() {
            Ok(result) => result,
            Err(_) => Err(FeedbackPublishError::Failed),
        }
    }
}

struct FeedbackMailbox<'reporter> {
    queue: VecDeque<GtkToolbarFeedback>,
    click_state: FeedbackClickState,
    accepting: bool,
    reporter: &'reporter ThreadReporter,
}

impl<'reporter> FeedbackMailbox<'reporter> {
    fn new(reporter: &'reporter ThreadReporter) -> Self {
        Self {
            queue: VecDeque::with_capacity(GTK_FEEDBACK_CAPACITY),
            click_state: FeedbackClickState::default(),
            accepting: true,
            reporter,
        }
    }

    fn publish(&mut self, feedback: GtkToolbarFeedback) -> Result<(), FeedbackPublishError> {
        if !self.accepting {
            return Err(FeedbackPublishError::Failed);
        }

        let mut overflowed = false;
        if self.queue.len() < GTK_FEEDBACK_CAPACITY {
            self.queue.push_back(feedback);
        } else if let Some(kind) = feedback.move_kind() {
            let mut replacement = None;
            for (index, queued) in self.queue.iter().enumerate().rev() {
                if queued.is_non_coalescible_boundary() {
                    break;
                }
                if queued.move_kind() == Some(kind) {
                    replacement = Some(index);
                    break;
                }
            }
            match replacement {
                Some(index) => self.queue[index] = feedback,
                None => {
                    let reclaim = self
                        .queue
                        .iter()
                        .position(|queued| queued.move_kind() == Some(kind))
                        .or_else(|| {
                            self.queue
                                .iter()
                                .position(|queued| queued.move_kind().is_some())
                        });
                    if let Some(index) = reclaim {
                        self.queue.remove(index);
                        self.queue.push_back(feedback);
                    } else {
                        self.accepting = false;
                        overflowed = true;
                    }
                }
            }
        } else {
            let reclaim = feedback
                .drag_kind()
                .and_then(|kind| oldest_move_in_current_segment(&self.queue, kind))
                .or_else(|| {
                    self.queue
                        .iter()
                        .position(|queued| queued.move_kind().is_some())
                });
            if let Some(index) = reclaim {
                self.queue.remove(index);
                self.queue.push_back(feedback);
            } else {
                self.accepting = false;
                overflowed = true;
            }
        }

        if overflowed {
            self.reporter.report_failure(
                "GTK feedback mailbox exhausted by ordered feedback; restoring built-in toolbars",
            );
            return Err(FeedbackPublishError::Failed);
        }

        if let Err(err) = self.reporter.wake_owner() {
            self.accepting = false;
            self.reporter.report_failure(format!(
                "GTK feedback could not wake the runtime ({err}); restoring built-in toolbars"
            ));
            return Err(FeedbackPublishError::Failed);
        }
        Ok(())
    }

    fn drain(&mut self, limit: usize) -> Vec<GtkToolbarFeedback> {
        let take = limit.min(self.queue.len());
        let drained = self.queue.drain(..take).collect::<Vec<_>>();
        if !self.queue.is_empty()
            && let Err(err) = self.reporter.wake_owner()
        {
            self.accepting = false;
            self.reporter.report_failure(format!(
                "Residual GTK feedback could not wake the runtime ({err}); restoring built-in toolbars"
            ));
        }
        drained
    }

    fn close(&mut self) {
        self.accepting = false;
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
            Self::Event { .. }
            | Self::TopHover { .. }
            | Self::CaptureSuppressionReady { .. }
            | Self::CaptureSuppressionFailed { .. } => None,
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

struct MailboxThreadExitGuard<'reporter> {
    reporter: &'reporter ThreadReporter,
    completed: bool,
}

impl<'reporter> MailboxThreadExitGuard<'reporter> {
    fn new(reporter: &'reporter ThreadReporter) -> Self {
        Self {
            reporter,
            completed: false,
        }
    }

    fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for MailboxThreadExitGuard<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.reporter.report_failure(
                "GTK feedback mailbox thread exited unexpectedly; restoring built-in toolbars",
            );
        }
    }
}

fn run_feedback_mailbox(commands: Receiver<FeedbackMailboxCommand>, reporter: &ThreadReporter) {
    let mut mailbox = FeedbackMailbox::new(reporter);
    while let Ok(command) = commands.recv() {
        match command {
            FeedbackMailboxCommand::Publish { feedback, reply } => {
                let _ = reply.send(mailbox.publish(feedback));
            }
            FeedbackMailboxCommand::Drain { limit, reply } => {
                let _ = reply.send(mailbox.drain(limit));
            }
            FeedbackMailboxCommand::SetRebindState { modifier, active } => {
                mailbox.click_state.set_rebind_state(modifier, active);
            }
            FeedbackMailboxCommand::CaptureClickModifiers(modifiers) => {
                mailbox.click_state.capture_click_modifiers(modifiers);
            }
            FeedbackMailboxCommand::FinishPointerClick => {
                mailbox.click_state.finish_pointer_click();
            }
            FeedbackMailboxCommand::PublishEvent { event, reply } => {
                let feedback = mailbox.click_state.resolve_event(event);
                let _ = reply.send(mailbox.publish(feedback));
            }
            FeedbackMailboxCommand::Shutdown => {
                mailbox.close();
                return;
            }
        }
    }
    mailbox.close();
}

fn spawn_feedback_mailbox(
    failures: Sender<String>,
    runtime_wake: RuntimeWakeSender,
) -> std::io::Result<(SyncSender<FeedbackMailboxCommand>, JoinHandle<()>)> {
    // There are exactly two synchronous request owners: the GTK publisher and
    // the backend drain owner. Each waits for its reply before issuing another
    // request, so this channel bounds all in-flight mailbox work.
    let (commands, command_rx) = sync_channel(2);
    let thread = std::thread::Builder::new()
        .name("gtk-feedback-mailbox".into())
        .spawn(move || {
            let reporter = ThreadReporter::new(failures, runtime_wake);
            let mut guard = MailboxThreadExitGuard::new(&reporter);
            run_feedback_mailbox(command_rx, &reporter);
            guard.complete();
        })?;
    Ok((commands, thread))
}

#[cfg(test)]
pub(super) struct TestMailbox {
    wake: crate::backend::wayland::RuntimeWakeSource,
    commands: SyncSender<FeedbackMailboxCommand>,
    failures: Receiver<String>,
    publisher: FeedbackPublisher,
    thread: Option<JoinHandle<()>>,
}

#[cfg(test)]
impl TestMailbox {
    pub(super) fn publisher(&self) -> FeedbackPublisher {
        self.publisher.clone()
    }

    pub(super) fn receive_one(&self) -> Option<GtkToolbarFeedback> {
        self.drain(1).into_iter().next()
    }

    fn drain(&self, limit: usize) -> Vec<GtkToolbarFeedback> {
        let (reply, result) = sync_channel(0);
        self.commands
            .send(FeedbackMailboxCommand::Drain { limit, reply })
            .expect("test mailbox accepts its drain command");
        result
            .recv()
            .expect("test mailbox returns its drained batch")
    }

    fn stop(&mut self) {
        let _ = self.commands.send(FeedbackMailboxCommand::Shutdown);
        assert_eq!(
            finish_thread(&mut self.thread),
            ThreadShutdownOutcome::Joined
        );
    }
}

#[cfg(test)]
impl Drop for TestMailbox {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.stop();
        }
    }
}

#[cfg(test)]
pub(super) fn publisher_channel() -> TestMailbox {
    let wake = crate::backend::wayland::RuntimeWakeSource::new()
        .expect("test creates its runtime wake source");
    let (failure_tx, failure_rx) = channel();
    let (commands, thread) = spawn_feedback_mailbox(
        failure_tx,
        wake.try_sender()
            .expect("test duplicates its runtime wake descriptor"),
    )
    .expect("test starts its feedback mailbox owner");
    TestMailbox {
        wake,
        publisher: FeedbackPublisher::new(commands.clone()),
        commands,
        failures: failure_rx,
        thread: Some(thread),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LatestValueSendError {
    Closed,
}

struct LatestValueSender<T> {
    sender: Option<watch::Sender<Option<T>>>,
}

pub(super) struct LatestValueReceiver<T> {
    receiver: watch::Receiver<Option<T>>,
}

fn latest_value_channel<T>() -> (LatestValueSender<T>, LatestValueReceiver<T>) {
    let (sender, receiver) = watch::channel(None);
    (
        LatestValueSender {
            sender: Some(sender),
        },
        LatestValueReceiver { receiver },
    )
}

impl<T: PartialEq> LatestValueSender<T> {
    fn publish(&mut self, value: T) -> Result<bool, LatestValueSendError> {
        let Some(sender) = self.sender.as_ref() else {
            return Err(LatestValueSendError::Closed);
        };
        if sender.borrow().as_ref() == Some(&value) {
            return Ok(false);
        }
        sender
            .send(Some(value))
            .map_err(|_| LatestValueSendError::Closed)?;
        Ok(true)
    }

    fn close(&mut self) {
        self.sender.take();
    }
}

impl<T: Clone> LatestValueReceiver<T> {
    pub(super) async fn recv(&mut self) -> Option<T> {
        self.receiver.changed().await.ok()?;
        self.receiver.borrow_and_update().clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BridgeState {
    Active,
    Failed,
    Stopping,
    Stopped,
}

struct GtkThreadExitGuard {
    reporter: ThreadReporter,
    completed: bool,
}

impl GtkThreadExitGuard {
    fn new(reporter: ThreadReporter) -> Self {
        Self {
            reporter,
            completed: false,
        }
    }

    fn finish(&mut self, exit: super::runtime::RuntimeExit) {
        if let super::runtime::RuntimeExit::Failed(reason) = exit {
            self.reporter.report_failure(reason);
        }
        self.completed = true;
    }
}

impl Drop for GtkThreadExitGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.reporter.report_failure(
                "GTK toolbar thread exited unexpectedly; restoring built-in toolbars",
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThreadShutdownOutcome {
    Joined,
    Panicked,
}

fn finish_thread(thread: &mut Option<JoinHandle<()>>) -> ThreadShutdownOutcome {
    let Some(thread) = thread.take() else {
        return ThreadShutdownOutcome::Joined;
    };
    match thread.join() {
        Ok(()) => ThreadShutdownOutcome::Joined,
        Err(_) => ThreadShutdownOutcome::Panicked,
    }
}

/// Main-thread owner of the GTK toolbar bridge and GTK thread.
pub struct GtkToolbarBridge {
    updates: LatestValueSender<GtkToolbarUpdate>,
    feedback_mailbox: SyncSender<FeedbackMailboxCommand>,
    failures: Receiver<String>,
    shutdown: Option<oneshot::Sender<()>>,
    state: BridgeState,
    gtk_thread: Option<JoinHandle<()>>,
    mailbox_thread: Option<JoinHandle<()>>,
}

impl GtkToolbarBridge {
    /// Spawns the GTK thread. Descriptor-duplication failures are returned,
    /// `None` means the OS thread could not be created, and GTK-level failures
    /// are published asynchronously before waking the runtime.
    pub fn spawn(runtime_wake: RuntimeWakeSender) -> std::io::Result<Option<Self>> {
        let (updates, update_rx) = latest_value_channel();
        let (failure_tx, failure_rx) = channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let mailbox_wake = runtime_wake.try_duplicate()?;
        let (feedback_mailbox, mailbox_thread) =
            match spawn_feedback_mailbox(failure_tx.clone(), mailbox_wake) {
                Ok(owner) => owner,
                Err(err) => {
                    log::error!("Failed to spawn GTK feedback mailbox thread: {err}");
                    return Ok(None);
                }
            };
        let thread_reporter = ThreadReporter::new(failure_tx, runtime_wake);
        let feedback = FeedbackPublisher::new(feedback_mailbox.clone());

        let spawned = std::thread::Builder::new()
            .name("gtk-toolbar".into())
            .spawn(move || {
                let mut guard = GtkThreadExitGuard::new(thread_reporter);
                let exit = super::runtime::run(update_rx, shutdown_rx, feedback);
                guard.finish(exit);
            });

        match spawned {
            Ok(thread) => Ok(Some(Self {
                updates,
                feedback_mailbox,
                failures: failure_rx,
                shutdown: Some(shutdown_tx),
                state: BridgeState::Active,
                gtk_thread: Some(thread),
                mailbox_thread: Some(mailbox_thread),
            })),
            Err(err) => {
                log::error!("Failed to spawn GTK toolbar thread: {err}");
                let _ = feedback_mailbox.send(FeedbackMailboxCommand::Shutdown);
                let mut mailbox_thread = Some(mailbox_thread);
                if finish_thread(&mut mailbox_thread) == ThreadShutdownOutcome::Panicked {
                    log::warn!("GTK feedback mailbox thread panicked during startup cleanup");
                }
                Ok(None)
            }
        }
    }

    /// Drains one bounded pass and snapshots terminal state. A producer closes
    /// admission before publishing failure, so a second bounded pass collects
    /// every feedback value accepted immediately before failover.
    pub fn drain_feedback(&mut self) -> (Vec<GtkToolbarFeedback>, bool) {
        let mut drained = match self.drain_feedback_pass(GTK_FEEDBACK_DRAIN_LIMIT) {
            Some(drained) => drained,
            None => {
                self.mark_failed();
                Vec::new()
            }
        };
        self.observe_failures();
        if self.state == BridgeState::Failed
            && let Some(tail) = self.drain_feedback_pass(GTK_FEEDBACK_DRAIN_LIMIT)
        {
            drained.extend(tail);
        }
        (drained, self.state == BridgeState::Failed)
    }

    /// Publishes the newest complete update and replaces an unread older update.
    pub fn maybe_send(&mut self, update: GtkToolbarUpdate) {
        self.observe_failures();
        if self.state != BridgeState::Active {
            return;
        }
        if self.updates.publish(update).is_err() {
            log::warn!("GTK toolbar update receiver disconnected; restoring built-in toolbars");
            self.mark_failed();
        }
    }

    fn drain_feedback_pass(&self, limit: usize) -> Option<Vec<GtkToolbarFeedback>> {
        let (reply, drained) = sync_channel(0);
        if self
            .feedback_mailbox
            .send(FeedbackMailboxCommand::Drain { limit, reply })
            .is_err()
        {
            return None;
        }
        drained.recv().ok()
    }

    fn observe_failures(&mut self) {
        let mut failed = self.state == BridgeState::Active
            && (self
                .gtk_thread
                .as_ref()
                .is_some_and(JoinHandle::is_finished)
                || self
                    .mailbox_thread
                    .as_ref()
                    .is_some_and(JoinHandle::is_finished));
        while let Ok(_reason) = self.failures.try_recv() {
            failed = true;
        }
        if failed {
            self.mark_failed();
        }
    }

    fn mark_failed(&mut self) {
        if self.state == BridgeState::Active {
            self.state = BridgeState::Failed;
            self.request_shutdown();
        }
    }

    fn request_shutdown(&mut self) {
        self.updates.close();
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

impl Drop for GtkToolbarBridge {
    fn drop(&mut self) {
        if self.state != BridgeState::Failed {
            self.state = BridgeState::Stopping;
        }
        self.request_shutdown();
        if finish_thread(&mut self.gtk_thread) == ThreadShutdownOutcome::Panicked {
            log::warn!("GTK toolbar thread panicked during shutdown");
        }
        let _ = self.feedback_mailbox.send(FeedbackMailboxCommand::Shutdown);
        if finish_thread(&mut self.mailbox_thread) == ThreadShutdownOutcome::Panicked {
            log::warn!("GTK feedback mailbox thread panicked during shutdown");
        }
        self.state = BridgeState::Stopped;
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;
    use std::sync::mpsc::TryRecvError;
    use std::time::{Duration, Instant};

    use gtk4::glib::MainContext;

    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;
    use crate::toolbar_gtk::{GtkToolbarDragPhase, GtkToolbarKind, GtkToolbarSurfaceSize};
    use crate::ui::toolbar::ToolbarEvent;

    const SURFACE: GtkToolbarSurfaceSize = GtkToolbarSurfaceSize {
        width: 200,
        height: 80,
    };

    fn event() -> GtkToolbarFeedback {
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::Undo,
            rebind_requested: false,
        }
    }

    fn drag(seq: u64) -> GtkToolbarFeedback {
        drag_with(GtkToolbarKind::Top, GtkToolbarDragPhase::Move, seq)
    }

    fn drag_with(kind: GtkToolbarKind, phase: GtkToolbarDragPhase, seq: u64) -> GtkToolbarFeedback {
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

    #[test]
    fn latest_value_receiver_runs_on_glib_and_observes_close_after_final_value() {
        let (mut latest, mut receiver) = latest_value_channel();
        assert_eq!(latest.publish(1), Ok(true));
        assert_eq!(latest.publish(2), Ok(true));
        latest.close();

        let context = MainContext::new();
        let (value, closed) =
            context.block_on(async move { (receiver.recv().await, receiver.recv().await) });
        assert_eq!(value, Some(2));
        assert_eq!(closed, None);
    }

    #[test]
    fn duplicate_latest_value_does_not_publish_another_change() {
        let (mut latest, mut receiver) = latest_value_channel();
        assert_eq!(latest.publish(7), Ok(true));

        let context = MainContext::new();
        assert_eq!(context.block_on(receiver.recv()), Some(7));
        assert_eq!(latest.publish(7), Ok(false));
        latest.close();
        assert_eq!(context.block_on(receiver.recv()), None);
    }

    #[test]
    fn latest_value_publish_rejects_a_disconnected_receiver() {
        let (mut latest, receiver) = latest_value_channel();
        drop(receiver);

        assert_eq!(latest.publish(1), Err(LatestValueSendError::Closed));
    }

    #[test]
    fn feedback_preserves_acceptance_order() {
        let mailbox = publisher_channel();
        for seq in 1..=3 {
            mailbox
                .publisher
                .publish(drag(seq))
                .expect("test feedback stays within channel capacity");
        }

        assert_eq!(
            mailbox.drain(GTK_FEEDBACK_CAPACITY),
            vec![drag(1), drag(2), drag(3)]
        );
        assert!(matches!(
            mailbox.failures.try_recv(),
            Err(TryRecvError::Empty | TryRecvError::Disconnected)
        ));
    }

    #[test]
    fn full_mailbox_coalesces_moves_within_the_current_segment() {
        let mailbox = publisher_channel();
        for seq in 1..=GTK_FEEDBACK_CAPACITY as u64 {
            mailbox
                .publisher
                .publish(drag(seq))
                .expect("test fills mailbox with coalescible moves");
        }
        mailbox
            .publisher
            .publish(drag(100))
            .expect("latest compatible move replaces an unread move");

        let drained = mailbox.drain(GTK_FEEDBACK_CAPACITY);
        assert_eq!(drained.len(), GTK_FEEDBACK_CAPACITY);
        assert_eq!(drained.last(), Some(&drag(100)));
        assert_eq!(mailbox.failures.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn coalescing_does_not_move_feedback_across_an_ordered_boundary() {
        let mailbox = publisher_channel();
        for seq in 0..(GTK_FEEDBACK_CAPACITY - 2) as u64 {
            mailbox
                .publisher
                .publish(drag(seq))
                .expect("test fills the pre-boundary segment");
        }
        mailbox
            .publisher
            .publish(event())
            .expect("test publishes its ordered event boundary");
        let side_move = drag_with(GtkToolbarKind::Side, GtkToolbarDragPhase::Move, 1);
        mailbox
            .publisher
            .publish(side_move.clone())
            .expect("test publishes a second-kind move after the boundary");
        mailbox
            .publisher
            .publish(drag(999))
            .expect("latest top move reclaims an older pre-boundary move");

        let drained = mailbox.drain(GTK_FEEDBACK_CAPACITY);
        assert_eq!(drained[GTK_FEEDBACK_CAPACITY - 3], event());
        assert_eq!(drained[GTK_FEEDBACK_CAPACITY - 2], side_move);
        assert_eq!(drained.last(), Some(&drag(999)));
        assert_eq!(mailbox.failures.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn feedback_is_committed_before_its_runtime_wake() {
        let mailbox = publisher_channel();
        mailbox
            .publisher
            .publish(event())
            .expect("test feedback fits in the empty channel");

        assert!(wake_is_readable(&mailbox.wake));
        assert_eq!(mailbox.drain(1), vec![event()]);
    }

    #[test]
    fn bounded_overload_is_typed_and_closes_future_admission() {
        let mailbox = publisher_channel();
        for _ in 0..GTK_FEEDBACK_CAPACITY {
            mailbox
                .publisher
                .publish(event())
                .expect("test fills exactly the configured channel capacity");
        }
        mailbox
            .wake
            .drain()
            .expect("test drains feedback publication wakeups");

        assert_eq!(
            mailbox.publisher.publish(event()),
            Err(FeedbackPublishError::Failed)
        );
        assert_eq!(
            mailbox.publisher.publish(event()),
            Err(FeedbackPublishError::Failed)
        );
        assert!(wake_is_readable(&mailbox.wake));
        assert!(
            mailbox
                .failures
                .try_recv()
                .expect("bounded overload publishes a terminal reason")
                .contains("ordered feedback")
        );
        assert_eq!(
            mailbox.drain(GTK_FEEDBACK_CAPACITY).len(),
            GTK_FEEDBACK_CAPACITY
        );
    }

    #[test]
    fn disconnected_feedback_receiver_closes_admission_without_failure() {
        let mut mailbox = publisher_channel();
        mailbox.stop();

        assert_eq!(
            mailbox.publisher.publish(event()),
            Err(FeedbackPublishError::Closed)
        );
        assert_eq!(
            mailbox.publisher.publish(event()),
            Err(FeedbackPublishError::Closed)
        );
        assert!(matches!(
            mailbox.failures.try_recv(),
            Err(TryRecvError::Empty | TryRecvError::Disconnected)
        ));
    }

    #[test]
    fn failed_bridge_drains_every_accepted_tail_value() {
        let wake = RuntimeWakeSource::new().expect("test creates its runtime wake source");
        let mailbox_wake = wake
            .try_sender()
            .expect("test duplicates its mailbox wake descriptor");
        let (updates, _update_rx) = latest_value_channel();
        let (failure_tx, failure_rx) = channel();
        let (feedback_mailbox, mailbox_thread) =
            spawn_feedback_mailbox(failure_tx.clone(), mailbox_wake)
                .expect("test starts its feedback mailbox owner");
        let publisher = FeedbackPublisher::new(feedback_mailbox.clone());
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let gtk_thread = std::thread::spawn(move || {
            let _ = shutdown_rx.blocking_recv();
        });
        for seq in 1..=GTK_FEEDBACK_CAPACITY as u64 {
            publisher
                .publish(drag(seq))
                .expect("test fills the accepted feedback tail exactly");
        }
        failure_tx
            .send("intentional transport failure".into())
            .expect("test publishes its terminal transport state");

        let mut bridge = GtkToolbarBridge {
            updates,
            feedback_mailbox,
            failures: failure_rx,
            shutdown: Some(shutdown_tx),
            state: BridgeState::Active,
            gtk_thread: Some(gtk_thread),
            mailbox_thread: Some(mailbox_thread),
        };
        let (drained, failed) = bridge.drain_feedback();

        assert!(failed);
        assert_eq!(drained.len(), GTK_FEEDBACK_CAPACITY);
        assert_eq!(drained.first(), Some(&drag(1)));
        assert_eq!(drained.last(), Some(&drag(GTK_FEEDBACK_CAPACITY as u64)));
    }

    #[test]
    fn bridge_drop_signals_shutdown_before_joining_thread() {
        let wake = RuntimeWakeSource::new().expect("test creates its runtime wake source");
        let mailbox_wake = wake
            .try_sender()
            .expect("test duplicates its mailbox wake descriptor");
        let (updates, _update_rx) = latest_value_channel();
        let (failure_tx, failure_rx) = channel();
        let (feedback_mailbox, mailbox_thread) = spawn_feedback_mailbox(failure_tx, mailbox_wake)
            .expect("test starts its feedback mailbox owner");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (observed_tx, observed_rx) = channel();
        let gtk_thread = std::thread::spawn(move || {
            let observed = shutdown_rx.blocking_recv().is_ok();
            let _ = observed_tx.send(observed);
        });
        let bridge = GtkToolbarBridge {
            updates,
            feedback_mailbox,
            failures: failure_rx,
            shutdown: Some(shutdown_tx),
            state: BridgeState::Active,
            gtk_thread: Some(gtk_thread),
            mailbox_thread: Some(mailbox_thread),
        };

        drop(bridge);

        assert_eq!(observed_rx.try_recv(), Ok(true));
    }

    #[test]
    fn joined_thread_is_consumed() {
        let mut thread = Some(std::thread::spawn(|| {}));
        assert_eq!(finish_thread(&mut thread), ThreadShutdownOutcome::Joined);
        assert!(thread.is_none());
    }

    #[test]
    fn panicked_thread_is_consumed_with_typed_outcome() {
        let mut thread = Some(std::thread::spawn(|| {
            panic!("intentional GTK bridge shutdown test panic");
        }));
        assert_eq!(finish_thread(&mut thread), ThreadShutdownOutcome::Panicked);
        assert!(thread.is_none());
    }

    #[test]
    fn finish_thread_waits_for_cooperative_release_instead_of_detaching() {
        let (release_tx, release_rx) = channel();
        let (joined_tx, joined_rx) = channel();
        let worker = std::thread::spawn(move || {
            let mut thread = Some(std::thread::spawn(move || {
                let _ = release_rx.recv();
            }));
            let outcome = finish_thread(&mut thread);
            let _ = joined_tx.send((outcome, thread.is_none()));
        });

        assert_eq!(
            joined_rx.recv_timeout(Duration::from_millis(20)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        );
        release_tx
            .send(())
            .expect("test releases its cooperatively stopped thread");
        let deadline = Instant::now() + Duration::from_secs(1);
        let joined = loop {
            match joined_rx.try_recv() {
                Ok(result) => break Ok(result),
                Err(TryRecvError::Empty) if Instant::now() < deadline => {
                    std::thread::yield_now();
                }
                Err(error) => break Err(error),
            }
        };
        let (outcome, consumed) = joined
            .expect("the test released the thread and allowed its observer to report completion");
        worker
            .join()
            .expect("test joins its finish-thread observer");
        assert_eq!(outcome, ThreadShutdownOutcome::Joined);
        assert!(consumed);
    }
}
