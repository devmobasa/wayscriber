use crate::backend::wayland::RuntimeWakeSender;
use crate::tray_action::TrayAction;
use crate::update_check::AvailableUpdate;
use std::ffi::OsString;
use std::io;
#[cfg(any(feature = "tray", test))]
use std::sync::mpsc::Sender;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
#[cfg(feature = "tray")]
use std::time::Instant;

const MAX_DAEMON_CONTROL_EVENTS: usize = 64;
const MAX_VISIBILITY_INTENTS: usize = 64;
const MAX_PENDING_QUIT_EVENTS: usize = 1;
#[cfg(any(feature = "tray", test))]
pub(super) const MAX_OVERLAY_ACTION_INTENTS: usize = 64;

#[derive(Debug)]
pub(super) enum DaemonPublishError {
    QueueFull,
    Disconnected,
    #[cfg(any(feature = "tray", test))]
    InvalidCapacity,
    Wake(io::Error),
}

impl std::fmt::Display for DaemonPublishError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull => formatter.write_str("daemon event queue is full"),
            Self::Disconnected => formatter.write_str("daemon event owner disconnected"),
            #[cfg(any(feature = "tray", test))]
            Self::InvalidCapacity => {
                formatter.write_str("daemon action capacity release exceeded its fixed bound")
            }
            Self::Wake(error) => write!(
                formatter,
                "daemon event was queued but wake failed: {error}"
            ),
        }
    }
}

impl std::error::Error for DaemonPublishError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Wake(error) => Some(error),
            Self::QueueFull | Self::Disconnected => None,
            #[cfg(any(feature = "tray", test))]
            Self::InvalidCapacity => None,
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct VisibilityIntent {
    pub(super) activation_token: Option<String>,
    pub(super) signal_requested: bool,
}

#[derive(Debug)]
pub(super) enum DaemonControlMessage {
    Quit,
    Visibility(VisibilityIntent),
    UpdateAvailable(Option<AvailableUpdate>),
    UpdateNotificationAuthorization(UpdateNotificationAuthorizationRequest),
    #[cfg(feature = "tray")]
    TrayWatcherOnline,
    #[cfg(feature = "tray")]
    TrayWatcherOffline(String),
}

#[derive(Debug)]
pub(super) struct UpdateNotificationAuthorizationRequest {
    pub(super) request_id: u64,
    pub(super) update: AvailableUpdate,
}

#[derive(Debug, Default)]
pub(super) struct DaemonEventBatch {
    pub(super) controls: Vec<DaemonControlMessage>,
    pub(super) overlay_actions: Vec<TrayAction>,
}

pub(super) struct DaemonEventInbox {
    quit: Receiver<()>,
    visibility: Receiver<VisibilityIntent>,
    control: Receiver<DaemonControlMessage>,
    #[cfg(any(feature = "tray", test))]
    overlay_actions: Receiver<TrayAction>,
    #[cfg(any(feature = "tray", test))]
    overlay_action_releases: Sender<()>,
}

impl DaemonEventInbox {
    pub(super) fn drain(&self) -> DaemonEventBatch {
        let mut controls = Vec::new();
        if self.quit.try_recv().is_ok() {
            controls.push(DaemonControlMessage::Quit);
        }
        controls.extend(
            self.visibility
                .try_iter()
                .map(DaemonControlMessage::Visibility),
        );
        controls.extend(self.control.try_iter());
        #[cfg(any(feature = "tray", test))]
        let overlay_actions = self.overlay_actions.try_iter().collect();
        #[cfg(not(any(feature = "tray", test)))]
        let overlay_actions = Vec::new();
        DaemonEventBatch {
            controls,
            overlay_actions,
        }
    }

    #[cfg(any(feature = "tray", test))]
    pub(super) fn release_overlay_action_slots(
        &self,
        count: usize,
    ) -> Result<(), DaemonPublishError> {
        for _ in 0..count {
            self.overlay_action_releases
                .send(())
                .map_err(|_| DaemonPublishError::Disconnected)?;
        }
        Ok(())
    }

    #[cfg(not(any(feature = "tray", test)))]
    pub(super) fn release_overlay_action_slots(
        &self,
        _count: usize,
    ) -> Result<(), DaemonPublishError> {
        Ok(())
    }
}

pub(super) struct DaemonEventSenders {
    quit: SyncSender<()>,
    visibility: SyncSender<VisibilityIntent>,
    control: SyncSender<DaemonControlMessage>,
    #[cfg(any(feature = "tray", test))]
    overlay_actions: SyncSender<TrayAction>,
    #[cfg(any(feature = "tray", test))]
    overlay_action_releases: Option<Receiver<()>>,
}

pub(super) fn daemon_event_channel() -> (DaemonEventInbox, DaemonEventSenders) {
    let (quit_sender, quit_receiver) = mpsc::sync_channel(MAX_PENDING_QUIT_EVENTS);
    let (visibility_sender, visibility_receiver) = mpsc::sync_channel(MAX_VISIBILITY_INTENTS);
    let (control_sender, control_receiver) = mpsc::sync_channel(MAX_DAEMON_CONTROL_EVENTS);
    #[cfg(any(feature = "tray", test))]
    let (action_sender, action_receiver) = mpsc::sync_channel(MAX_OVERLAY_ACTION_INTENTS);
    #[cfg(any(feature = "tray", test))]
    let (action_release_sender, action_release_receiver) = mpsc::channel();
    (
        DaemonEventInbox {
            quit: quit_receiver,
            visibility: visibility_receiver,
            control: control_receiver,
            #[cfg(any(feature = "tray", test))]
            overlay_actions: action_receiver,
            #[cfg(any(feature = "tray", test))]
            overlay_action_releases: action_release_sender,
        },
        DaemonEventSenders {
            quit: quit_sender,
            visibility: visibility_sender,
            control: control_sender,
            #[cfg(any(feature = "tray", test))]
            overlay_actions: action_sender,
            #[cfg(any(feature = "tray", test))]
            overlay_action_releases: Some(action_release_receiver),
        },
    )
}

struct EventPublisher<T> {
    sender: SyncSender<T>,
    wake: RuntimeWakeSender,
}

impl<T> EventPublisher<T> {
    fn try_duplicate(&self) -> io::Result<Self> {
        Ok(Self {
            sender: self.sender.clone(),
            wake: self.wake.try_duplicate()?,
        })
    }

    fn publish(&self, message: T, source: &str) -> Result<(), DaemonPublishError> {
        publish_and_wake(&self.sender, message, &self.wake, source)
    }
}

fn publish_and_wake<T>(
    sender: &SyncSender<T>,
    message: T,
    wake: &RuntimeWakeSender,
    source: &str,
) -> Result<(), DaemonPublishError> {
    match sender.try_send(message) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => return Err(DaemonPublishError::QueueFull),
        Err(TrySendError::Disconnected(_)) => return Err(DaemonPublishError::Disconnected),
    }
    wake.wake().map_err(|error| {
        DaemonPublishError::Wake(io::Error::new(error.kind(), format!("{source}: {error}")))
    })
}

/// A shutdown publisher. The daemon loop remains the only owner of the quit
/// state; producers can only enqueue a typed request and wake that owner.
pub(super) struct DaemonControlEvent {
    publisher: EventPublisher<()>,
}

impl DaemonControlEvent {
    pub(super) fn try_duplicate(&self) -> io::Result<Self> {
        Ok(Self {
            publisher: self.publisher.try_duplicate()?,
        })
    }

    #[cfg(any(feature = "tray", test))]
    pub(super) fn raise(&self, source: &str) -> Result<(), DaemonPublishError> {
        match self.publisher.sender.try_send(()) {
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => return Err(DaemonPublishError::Disconnected),
        }
        self.publisher.wake.wake().map_err(|error| {
            DaemonPublishError::Wake(io::Error::new(error.kind(), format!("{source}: {error}")))
        })
    }
}

pub(super) struct VisibilityPublisher {
    publisher: EventPublisher<VisibilityIntent>,
}

impl VisibilityPublisher {
    pub(super) fn try_duplicate(&self) -> io::Result<Self> {
        Ok(Self {
            publisher: self.publisher.try_duplicate()?,
        })
    }

    #[cfg(any(feature = "portal", feature = "tray", test))]
    pub(super) fn publish(
        &self,
        activation_token: Option<String>,
        signal_requested: bool,
        source: &str,
    ) -> Result<(), DaemonPublishError> {
        self.publisher.publish(
            VisibilityIntent {
                activation_token,
                signal_requested,
            },
            source,
        )
    }
}

pub(super) struct UpdateWatchPublisher {
    publisher: EventPublisher<DaemonControlMessage>,
}

impl UpdateWatchPublisher {
    pub(super) fn publish_available(
        &self,
        update: Option<AvailableUpdate>,
    ) -> Result<(), DaemonPublishError> {
        self.publisher.publish(
            DaemonControlMessage::UpdateAvailable(update),
            "update availability publication",
        )
    }

    pub(super) fn request_notification(
        &self,
        request_id: u64,
        update: AvailableUpdate,
    ) -> Result<(), DaemonPublishError> {
        self.publisher.publish(
            DaemonControlMessage::UpdateNotificationAuthorization(
                UpdateNotificationAuthorizationRequest { request_id, update },
            ),
            "update notification authorization publication",
        )
    }
}

#[cfg(feature = "tray")]
pub(super) struct TrayStatusPublisher {
    publisher: EventPublisher<DaemonControlMessage>,
}

#[cfg(feature = "tray")]
impl TrayStatusPublisher {
    pub(super) fn watcher_online(&self) -> Result<(), DaemonPublishError> {
        self.publisher.publish(
            DaemonControlMessage::TrayWatcherOnline,
            "tray watcher-online publication",
        )
    }

    pub(super) fn watcher_offline(&self, reason: String) -> Result<(), DaemonPublishError> {
        self.publisher.publish(
            DaemonControlMessage::TrayWatcherOffline(reason),
            "tray watcher-offline publication",
        )
    }
}

pub(super) struct OverlayActionPublisher {
    #[cfg(any(feature = "tray", test))]
    sender: SyncSender<TrayAction>,
    #[cfg(any(feature = "tray", test))]
    wake: RuntimeWakeSender,
    #[cfg(any(feature = "tray", test))]
    releases: Receiver<()>,
    #[cfg(any(feature = "tray", test))]
    available_slots: usize,
}

#[cfg(any(feature = "tray", test))]
pub(super) type OverlayActionPublishError = DaemonPublishError;

#[cfg(any(feature = "tray", test))]
impl OverlayActionPublisher {
    fn refresh_available_slots(&mut self) -> Result<(), OverlayActionPublishError> {
        for () in self.releases.try_iter() {
            if self.available_slots >= MAX_OVERLAY_ACTION_INTENTS {
                return Err(DaemonPublishError::InvalidCapacity);
            }
            self.available_slots += 1;
        }
        Ok(())
    }

    pub(super) fn publish(&mut self, action: TrayAction) -> Result<(), OverlayActionPublishError> {
        self.refresh_available_slots()?;
        if self.available_slots == 0 {
            return Err(DaemonPublishError::QueueFull);
        }
        self.available_slots -= 1;
        match self.sender.try_send(action) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.available_slots += 1;
                return Err(DaemonPublishError::QueueFull);
            }
            Err(TrySendError::Disconnected(_)) => {
                self.available_slots += 1;
                return Err(DaemonPublishError::Disconnected);
            }
        }
        self.wake.wake().map_err(|error| {
            DaemonPublishError::Wake(io::Error::new(
                error.kind(),
                format!("tray action: {error}"),
            ))
        })
    }
}

impl DaemonEventSenders {
    fn control_publisher(&self, wake: RuntimeWakeSender) -> EventPublisher<DaemonControlMessage> {
        EventPublisher {
            sender: self.control.clone(),
            wake,
        }
    }

    pub(super) fn quit(&self, wake: RuntimeWakeSender) -> DaemonControlEvent {
        DaemonControlEvent {
            publisher: EventPublisher {
                sender: self.quit.clone(),
                wake,
            },
        }
    }

    pub(super) fn visibility(&self, wake: RuntimeWakeSender) -> VisibilityPublisher {
        VisibilityPublisher {
            publisher: EventPublisher {
                sender: self.visibility.clone(),
                wake,
            },
        }
    }

    pub(super) fn update_watch(&self, wake: RuntimeWakeSender) -> UpdateWatchPublisher {
        UpdateWatchPublisher {
            publisher: self.control_publisher(wake),
        }
    }

    #[cfg(feature = "tray")]
    pub(super) fn tray_status(&self, wake: RuntimeWakeSender) -> TrayStatusPublisher {
        TrayStatusPublisher {
            publisher: self.control_publisher(wake),
        }
    }

    pub(super) fn overlay_actions(
        &mut self,
        wake: RuntimeWakeSender,
    ) -> io::Result<OverlayActionPublisher> {
        #[cfg(not(any(feature = "tray", test)))]
        let _ = wake;
        Ok(OverlayActionPublisher {
            #[cfg(any(feature = "tray", test))]
            sender: self.overlay_actions.clone(),
            #[cfg(any(feature = "tray", test))]
            wake,
            #[cfg(any(feature = "tray", test))]
            releases: self.overlay_action_releases.take().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "daemon overlay-action publisher was already claimed",
                )
            })?,
            #[cfg(any(feature = "tray", test))]
            available_slots: MAX_OVERLAY_ACTION_INTENTS,
        })
    }
}

/// Overlay state for daemon mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Hidden,
    Visible,
}

#[cfg(feature = "tray")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OverlaySpawnErrorInfo {
    pub(crate) message: String,
    pub(crate) next_retry_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub(crate) struct OverlaySpawnCandidate {
    pub(crate) program: OsString,
    pub(crate) source: &'static str,
}

#[cfg(feature = "tray")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AvailableUpdateNotice {
    pub(crate) version: String,
    pub(crate) update_url: String,
}

#[cfg(feature = "tray")]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct TrayStatus {
    pub(crate) overlay_error: Option<OverlaySpawnErrorInfo>,
    pub(crate) watcher_offline: bool,
    pub(crate) watcher_reason: Option<String>,
    pub(crate) available_update: Option<AvailableUpdateNotice>,
}

#[cfg(feature = "tray")]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct TraySnapshot {
    pub(crate) overlay_active: bool,
    pub(crate) status: TrayStatus,
}

#[derive(Debug)]
pub struct AlreadyRunningError;

impl std::fmt::Display for AlreadyRunningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("wayscriber daemon is already running")
    }
}

impl std::error::Error for AlreadyRunningError {}

/// Test seam for running the overlay inline without spawning another process.
pub type BackendRunner = dyn FnMut(Option<String>, Option<bool>) -> anyhow::Result<()> + Send;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;
    use std::time::Duration;

    fn test_wake() -> (RuntimeWakeSource, RuntimeWakeSender) {
        let wake = RuntimeWakeSource::new().expect("test creates a daemon runtime eventfd");
        let sender = wake
            .try_sender()
            .expect("test duplicates its daemon runtime eventfd");
        (wake, sender)
    }

    #[test]
    fn independent_action_owners_preserve_fifo_order() {
        let (first_inbox, mut first_senders) = daemon_event_channel();
        let (second_inbox, mut second_senders) = daemon_event_channel();
        let (first_wake, first_sender) = test_wake();
        let (second_wake, second_sender) = test_wake();
        let mut first = first_senders
            .overlay_actions(first_sender)
            .expect("first fixture claims its only action publisher");
        let mut second = second_senders
            .overlay_actions(second_sender)
            .expect("second fixture claims its only action publisher");

        first
            .publish(TrayAction::ToggleFreeze)
            .expect("first owner publishes into an open fixture queue");
        first
            .publish(TrayAction::ToggleHelp)
            .expect("first owner preserves its second FIFO action");
        second
            .publish(TrayAction::CaptureRegion)
            .expect("second owner publishes into its isolated fixture queue");

        assert!(
            first_wake
                .wait_readable(Some(Duration::ZERO))
                .expect("fixture wake remains readable")
        );
        assert!(
            second_wake
                .wait_readable(Some(Duration::ZERO))
                .expect("fixture wake remains readable")
        );
        assert_eq!(
            first_inbox.drain().overlay_actions,
            [TrayAction::ToggleFreeze, TrayAction::ToggleHelp]
        );
        assert_eq!(
            second_inbox.drain().overlay_actions,
            [TrayAction::CaptureRegion]
        );
    }

    #[test]
    fn bounded_action_queue_reports_backpressure_without_reordering() {
        let (inbox, mut senders) = daemon_event_channel();
        let (_wake, wake_sender) = test_wake();
        let mut publisher = senders
            .overlay_actions(wake_sender)
            .expect("fixture claims its only action publisher");

        for _ in 0..MAX_OVERLAY_ACTION_INTENTS {
            publisher
                .publish(TrayAction::ToggleHelp)
                .expect("fixture queue has its documented remaining capacity");
        }
        assert!(matches!(
            publisher.publish(TrayAction::ToggleFreeze),
            Err(DaemonPublishError::QueueFull)
        ));
        let batch = inbox.drain();
        assert_eq!(batch.overlay_actions.len(), MAX_OVERLAY_ACTION_INTENTS);
        assert!(matches!(
            publisher.publish(TrayAction::ToggleFreeze),
            Err(DaemonPublishError::QueueFull)
        ));

        inbox
            .release_overlay_action_slots(1)
            .expect("owner returns one completed action slot");
        publisher
            .publish(TrayAction::ToggleFreeze)
            .expect("returned capacity admits exactly one newer action");
        assert_eq!(inbox.drain().overlay_actions, [TrayAction::ToggleFreeze]);
    }

    #[test]
    fn dropped_owner_reports_disconnection() {
        let (inbox, senders) = daemon_event_channel();
        let (_wake, wake_sender) = test_wake();
        let publisher = senders.visibility(wake_sender);
        drop(inbox);

        assert!(matches!(
            publisher.publish(None, true, "fixture publication"),
            Err(DaemonPublishError::Disconnected)
        ));
    }

    #[test]
    fn shutdown_publication_is_independent_and_idempotent_when_control_is_full() {
        let (inbox, senders) = daemon_event_channel();
        let (_control_wake, control_sender) = test_wake();
        let (_quit_wake, quit_sender) = test_wake();
        let updates = senders.update_watch(control_sender);
        let quit = senders.quit(quit_sender);

        for _ in 0..MAX_DAEMON_CONTROL_EVENTS {
            updates
                .publish_available(None)
                .expect("fixture fills the ordinary control queue");
        }
        quit.raise("first fixture shutdown")
            .expect("shutdown has an independent queue");
        quit.raise("duplicate fixture shutdown")
            .expect("a duplicate shutdown coalesces with the pending request");

        let batch = inbox.drain();
        assert!(matches!(
            batch.controls.first(),
            Some(DaemonControlMessage::Quit)
        ));
        assert_eq!(batch.controls.len(), MAX_DAEMON_CONTROL_EVENTS + 1);
    }
}
