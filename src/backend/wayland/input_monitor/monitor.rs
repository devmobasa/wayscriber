//! Reader thread that turns libinput events into input HUD chips.
//!
//! The thread owns the entire capture stack — the libinput context, its device
//! file descriptors, and the xkb state — because none of it is `Send`-shareable
//! and none of it belongs to the Wayland thread's model. Nothing is shared:
//! chips travel over an `mpsc` channel whose receiver stays pinned to the
//! Wayland thread, and the thread pokes the existing `RuntimeWakeHandle` so the
//! event loop drains them in `route_woken_sources`.
//!
//! Shutdown is a pipe: the Wayland thread writes one byte to the write end and
//! the reader's `poll(2)` returns immediately, so a blocked read never delays
//! a disable, a mode change, or overlay exit.
//!
//! Labels never reach a log. The thread formats them and sends them straight
//! into render-only state; failures are reported as typed `Failed` events so a
//! device or keymap problem can never panic across the libinput FFI edge.

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use input::event::device::DeviceEvent;
use input::event::keyboard::{KeyState, KeyboardEventTrait};
use input::event::pointer::{Axis, ButtonState, PointerScrollEvent};
use input::event::tablet_tool::{TabletToolEvent, TipState};
use input::event::{Event, EventTrait, KeyboardEvent, PointerEvent};
use input::{DeviceCapability, Libinput, LibinputInterface};
use xkbcommon::xkb;

use super::super::RuntimeWakeHandle;
use super::super::handlers::keyboard::{KEY_REPEAT_INITIAL_DELAY, KEY_REPEAT_INTERVAL};
use super::translate::{
    EVDEV_KEYCODE_OFFSET, button_label, key_label, modifiers_from_xkb, pen_label, scroll_label,
};

mod worker;

use worker::{run, stop_pipe};

/// Appended to failures that group membership would plausibly fix. Kept in
/// step with the equivalent text in `docs/CONFIG.md`.
const GROUP_HINT: &str = "add your user to the 'input' group \
(sudo usermod -aG input $USER, then re-login).";
/// How long the Wayland thread waits for the reader to notice the stop byte
/// before giving up on the join and letting the thread finish detached.
const STOP_JOIN_TIMEOUT: Duration = Duration::from_millis(500);
/// Poll interval while waiting for the reader thread to finish.
const STOP_JOIN_POLL: Duration = Duration::from_millis(5);

/// One translated chip, or a lifecycle event from the reader thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) enum SystemInputEvent {
    /// The seat is open, at least one usable device was enumerated, and the
    /// keymap compiled: capture is live. The HUD switches to the system
    /// source only on this event, never on thread spawn, so a seat that turns
    /// out to be empty never suppresses overlay reporting in the meantime.
    Ready,
    Key {
        label: String,
        bare_modifier: bool,
    },
    Mouse {
        label: String,
    },
    Scroll {
        label: String,
    },
    /// The reader could not start or could not continue; the Wayland thread
    /// tears the monitor down and falls back to overlay mode.
    Failed(SystemInputFailure),
}

/// Why system capture is not available.
///
/// Typed rather than a formatted string so the Wayland thread can tell a
/// permission problem (fixable by joining the `input` group) from an empty
/// seat, a broken layout, or a runtime read error, and say the right thing
/// instead of always reciting the group hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) enum SystemInputFailure {
    /// The reader could not be started at all (pipe or thread creation).
    /// An OS resource problem, never a permission one.
    StartFailed(String),
    /// libinput could not open the seat at all.
    SeatUnavailable { seat: String },
    /// Device nodes exist but this process cannot read them — the one case
    /// `input` group membership actually fixes.
    DevicesUnreadable { seat: String },
    /// The seat is genuinely empty, or carries nothing the HUD reports from.
    /// Distinct from [`DevicesUnreadable`]: no permission change helps.
    ///
    /// [`DevicesUnreadable`]: SystemInputFailure::DevicesUnreadable
    NoUsableDevices { seat: String },
    /// Capture is unavailable and the cause could not be established (udev
    /// could not enumerate the seat). Says so plainly rather than guessing at
    /// a fix that may not apply.
    Unavailable { seat: String },
    /// No keyboard layout could be compiled for translation.
    KeymapUnavailable,
    /// A read or dispatch failed once capture was already running.
    ReadFailed(String),
    /// The reader ended without reporting why.
    ReaderStopped,
}

impl SystemInputFailure {
    /// User-facing explanation, with the `input` group guidance attached only
    /// where group membership is plausibly the cause.
    pub(in crate::backend::wayland) fn user_message(&self) -> String {
        match self {
            Self::StartFailed(reason) => {
                format!("System-wide capture could not be started: {reason}.")
            }
            Self::SeatUnavailable { seat } => {
                format!("System-wide capture could not open the '{seat}' input seat.")
            }
            Self::DevicesUnreadable { seat } => format!(
                "System-wide capture cannot read the input devices on seat '{seat}' - {GROUP_HINT}"
            ),
            Self::NoUsableDevices { seat } => format!(
                "System-wide capture found no keyboard, pointer, or tablet devices on seat '{seat}'."
            ),
            Self::Unavailable { seat } => {
                format!("System-wide capture is unavailable on seat '{seat}'.")
            }
            Self::KeymapUnavailable => "System-wide capture could not compile a keyboard layout; \
check XKB_DEFAULT_LAYOUT and xkeyboard-config."
                .to_string(),
            Self::ReadFailed(reason) => {
                format!("System-wide capture stopped reading input: {reason}.")
            }
            Self::ReaderStopped => "System-wide capture stopped unexpectedly.".to_string(),
        }
    }
}

/// Handle to a running reader thread, owned by `WaylandState`.
pub(in crate::backend::wayland) struct InputMonitor {
    join: Option<JoinHandle<()>>,
    /// Write end of the stop pipe; one byte ends the reader's poll.
    stop: OwnedFd,
    events: Receiver<SystemInputEvent>,
    /// Set when the reader's `Ready` event has been drained. Until then the
    /// HUD stays on the overlay source: the thread may still turn out to have
    /// an empty seat or an uncompilable keymap.
    ready: bool,
}

impl InputMonitor {
    /// Spawn the reader thread. Returns an error only for failures the Wayland
    /// thread caused (pipe creation); everything the reader itself hits arrives
    /// later as `SystemInputEvent::Failed`.
    pub(in crate::backend::wayland) fn start(wake: RuntimeWakeHandle) -> io::Result<Self> {
        let (stop_read, stop_write) = stop_pipe()?;
        let (sender, events) = channel();
        let join = thread::Builder::new()
            .name("wayscriber-input-monitor".to_string())
            .spawn(move || run(stop_read, sender, wake))?;
        Ok(Self {
            join: Some(join),
            stop: stop_write,
            events,
            ready: false,
        })
    }

    /// Whether the reader has reported that capture is actually live.
    pub(in crate::backend::wayland) fn is_ready(&self) -> bool {
        self.ready
    }

    /// Record the reader's `Ready` event.
    pub(in crate::backend::wayland) fn mark_ready(&mut self) {
        self.ready = true;
    }

    /// Take every chip the reader has produced since the last drain.
    pub(in crate::backend::wayland) fn drain(&mut self) -> Vec<SystemInputEvent> {
        let mut drained = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(event) => drained.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // The reader exited without a Failed event (a panic-free
                    // early return or a dropped sender); report it so the
                    // caller falls back rather than waiting forever.
                    drained.push(SystemInputEvent::Failed(SystemInputFailure::ReaderStopped));
                    break;
                }
            }
        }
        drained
    }

    /// Ask the reader to stop and wait a bounded time for it.
    ///
    /// A reader wedged inside libinput is left to finish on its own rather than
    /// blocking overlay shutdown; its descriptors close with the process.
    fn stop(&mut self) {
        let byte = [1_u8];
        // SAFETY: the write end is owned and open for the duration of the call,
        // and `byte` is a valid one-byte buffer.
        let written =
            unsafe { libc::write(self.stop.as_raw_fd(), byte.as_ptr().cast(), byte.len()) };
        if written < 0 {
            log::warn!(
                "Failed to signal the system input reader: {}",
                io::Error::last_os_error()
            );
        }
        let Some(join) = self.join.take() else {
            return;
        };
        let deadline = Instant::now() + STOP_JOIN_TIMEOUT;
        while !join.is_finished() {
            if Instant::now() >= deadline {
                log::warn!("System input reader did not stop in time; leaving it detached");
                return;
            }
            thread::sleep(STOP_JOIN_POLL);
        }
        if join.join().is_err() {
            log::warn!("System input reader thread ended abnormally");
        }
    }
}

impl Drop for InputMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}
