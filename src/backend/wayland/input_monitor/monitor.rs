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

/// Non-blocking, close-on-exec pipe used only to interrupt the reader's poll.
fn stop_pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as RawFd; 2];
    // SAFETY: `fds` is a writable two-element array, the shape pipe2(2) expects.
    let result = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: both descriptors were just returned by pipe2 and are unwrapped.
    let read = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    // SAFETY: as above for the write end.
    let write = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    Ok((read, write))
}

/// Open evdev nodes for libinput. Unprivileged `open(2)`: it succeeds when the
/// user is in the `input` group, which is exactly the permission model the
/// probe reports and `docs/CONFIG.md` documents.
struct DirectInputDevices;

impl LibinputInterface for DirectInputDevices {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        OpenOptions::new()
            .custom_flags(flags)
            .read((flags & libc::O_WRONLY) == 0)
            .write((flags & libc::O_RDWR) != 0 || (flags & libc::O_WRONLY) != 0)
            .open(path)
            .map(OwnedFd::from)
            .map_err(|err| err.raw_os_error().unwrap_or(libc::EIO))
    }

    fn close_restricted(&mut self, fd: OwnedFd) {
        drop(File::from(fd));
    }
}

/// Reader thread body. Every exit path is a typed event or a clean return; a
/// panic must never cross the libinput FFI boundary.
fn run(stop_read: OwnedFd, sender: Sender<SystemInputEvent>, wake: RuntimeWakeHandle) {
    let seat = super::probe::current_seat();
    let mut libinput = Libinput::new_with_udev(DirectInputDevices);
    if libinput.udev_assign_seat(&seat).is_err() {
        report_failure(&sender, &wake, SystemInputFailure::SeatUnavailable { seat });
        return;
    }
    // `udev_assign_seat` succeeds for a seat with no devices, and libinput
    // silently skips nodes `open_restricted` could not open. Without this
    // check the HUD would sit in system mode reporting nothing at all, which
    // reads as a broken feature rather than a permission problem.
    match usable_device_count(&mut libinput) {
        Ok(0) => {
            // Nodes this process cannot open look exactly like an empty seat
            // from libinput's side, so ask udev which it was: only the
            // permission case is worth pointing at the `input` group, and an
            // unclassifiable seat gets no invented cause at all.
            let failure = match super::probe::event_node_access() {
                super::probe::EventNodeAccess::Unreadable => {
                    SystemInputFailure::DevicesUnreadable { seat }
                }
                super::probe::EventNodeAccess::None => SystemInputFailure::NoUsableDevices { seat },
                super::probe::EventNodeAccess::Unknown
                | super::probe::EventNodeAccess::Readable => {
                    SystemInputFailure::Unavailable { seat }
                }
            };
            report_failure(&sender, &wake, failure);
            return;
        }
        Ok(_) => {}
        Err(err) => {
            report_failure(
                &sender,
                &wake,
                SystemInputFailure::ReadFailed(format!("libinput dispatch failed: {err}")),
            );
            return;
        }
    }

    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    // Empty RMLVO names mean "the environment's defaults", so XKB_DEFAULT_*
    // is honored. This can differ from the compositor's live layout; the
    // limitation is documented rather than guessed at.
    let Some(keymap) =
        xkb::Keymap::new_from_names(&context, "", "", "", "", None, xkb::KEYMAP_COMPILE_NO_FLAGS)
    else {
        report_failure(&sender, &wake, SystemInputFailure::KeymapUnavailable);
        return;
    };
    let mut xkb_state = xkb::State::new(&keymap);
    let mut repeat = RepeatState::new();
    let mut keys = KeyboardKeys::default();

    // Everything that can fail deterministically has now succeeded; hand the
    // Wayland thread the go-ahead so it can switch sources.
    if sender.send(SystemInputEvent::Ready).is_err() {
        return;
    }
    if let Err(err) = wake.wake() {
        log::warn!("Failed to wake the runtime for system input events: {err}");
        return;
    }

    loop {
        match wait_for_input(&libinput, &stop_read, repeat.timeout(Instant::now())) {
            Ok(PollOutcome::Stop) => return,
            Ok(PollOutcome::Ready) => {}
            Ok(PollOutcome::Timeout) => {
                // No device activity: the held key's repeat came due.
                if let Some(chip) = repeat.due(Instant::now()) {
                    if sender.send(chip).is_err() {
                        return;
                    }
                    if let Err(err) = wake.wake() {
                        log::warn!("Failed to wake the runtime for system input events: {err}");
                        return;
                    }
                }
                continue;
            }
            Err(err) => {
                report_failure(
                    &sender,
                    &wake,
                    SystemInputFailure::ReadFailed(format!("input poll failed: {err}")),
                );
                return;
            }
        }

        if let Err(err) = libinput.dispatch() {
            report_failure(
                &sender,
                &wake,
                SystemInputFailure::ReadFailed(format!("libinput dispatch failed: {err}")),
            );
            return;
        }

        let mut sent_any = false;
        for event in &mut libinput {
            let Some(chip) = translate_event(event, &mut xkb_state, &mut repeat, &mut keys) else {
                continue;
            };
            if sender.send(chip).is_err() {
                // The Wayland thread dropped the receiver; nothing left to do.
                return;
            }
            sent_any = true;
        }
        if sent_any && let Err(err) = wake.wake() {
            log::warn!("Failed to wake the runtime for system input events: {err}");
            return;
        }
    }
}

/// Count devices on the seat the HUD can actually report from.
///
/// The first dispatch after `udev_assign_seat` enumerates the seat, so every
/// existing device arrives here as an `Added` event. Touch and switch devices
/// are excluded because nothing downstream translates them into chips.
fn usable_device_count(libinput: &mut Libinput) -> Result<usize, io::Error> {
    libinput.dispatch()?;
    let mut usable = 0;
    for event in libinput {
        // Startup enumeration produces device events only, so dropping the
        // rest here cannot swallow a real keystroke.
        if let Event::Device(DeviceEvent::Added(added)) = event {
            let device = added.device();
            if [
                DeviceCapability::Keyboard,
                DeviceCapability::Pointer,
                DeviceCapability::TabletTool,
            ]
            .iter()
            .any(|capability| device.has_capability(*capability))
            {
                usable += 1;
            }
        }
    }
    Ok(usable)
}

/// Client-side typematic for the system source.
///
/// libinput reports logical key state transitions only — compositors and
/// toolkits synthesize their own repeats — so a held Backspace would freeze
/// the HUD's ×N counter without this. The policy mirrors the overlay path
/// exactly: any fresh press ends the previous repeat, a repeatable press
/// re-arms it, and releasing the held key stops it. The label is captured at
/// press time; a chord whose modifiers change mid-hold keeps its original
/// label, which matches how the coalesced chip already reads.
struct RepeatState {
    held: Option<HeldKey>,
}

struct HeldKey {
    /// Raw evdev keycode, used to match the eventual release.
    keycode: u32,
    /// Device the held key belongs to, so unplugging a *different* keyboard
    /// leaves this repeat running.
    device: String,
    label: String,
    next_at: Instant,
}

/// Which keys each keyboard currently holds down.
///
/// libinput sends no releases for a device that disappears, so the reader
/// keeps its own per-device ledger: on removal it drops that device's entry
/// and replays what the survivors still hold. Tracking it per device is what
/// makes unplugging an external keyboard leave a modifier held on the
/// built-in one alone.
#[derive(Default)]
struct KeyboardKeys {
    held: HashMap<String, HashSet<u32>>,
}

impl KeyboardKeys {
    /// Record a press. Returns whether this is the *first* device to hold the
    /// keycode, i.e. whether xkb should be told the key went down.
    ///
    /// xkb counts transitions, not holders: two `Down`s followed by one `Up`
    /// leaves the modifier latched forever (verified against libxkbcommon, and
    /// pinned by `xkb_down_and_up_must_be_balanced_per_keycode`). Reporting
    /// only the first press keeps `Down`/`Up` exactly balanced no matter how
    /// many keyboards hold the same key.
    fn press(&mut self, device: &str, keycode: u32) -> bool {
        let first = !self.is_held_anywhere(keycode);
        self.held
            .entry(device.to_string())
            .or_default()
            .insert(keycode);
        first
    }

    /// Record a release. Returns whether the key is now up everywhere, i.e.
    /// whether xkb should be told the key was released.
    fn release(&mut self, device: &str, keycode: u32) -> bool {
        if let Some(keys) = self.held.get_mut(device) {
            keys.remove(&keycode);
            if keys.is_empty() {
                self.held.remove(device);
            }
        }
        !self.is_held_anywhere(keycode)
    }

    /// Whether any device still holds `keycode`.
    fn is_held_anywhere(&self, keycode: u32) -> bool {
        self.held.values().any(|keys| keys.contains(&keycode))
    }

    /// Forget a vanished device and release, in xkb, exactly the keys nothing
    /// else is holding.
    ///
    /// Deliberately *not* a rebuild from a fresh `xkb::State`: that would
    /// discard locked and latched modifiers (Caps Lock above all) and the
    /// selected layout group, desynchronizing every later label from the app
    /// the user is actually typing into. Releasing the orphaned keys one by
    /// one leaves all of that untouched, and stays balanced because a key
    /// another keyboard still holds is skipped.
    fn forget_device(&mut self, device: &str, xkb_state: &mut xkb::State) {
        let Some(orphans) = self.held.remove(device) else {
            return;
        };
        for keycode in orphans {
            if self.is_held_anywhere(keycode) {
                continue;
            }
            xkb_state.update_key(
                xkb::Keycode::new(keycode + EVDEV_KEYCODE_OFFSET),
                xkb::KeyDirection::Up,
            );
        }
    }
}

/// What a press means for the repeat timer: the keymap's repeat flag, plus
/// the chip label when the HUD has one.
enum RepeatPress<'a> {
    /// The keymap marks this key as non-repeating (modifiers, and anything
    /// else the layout excludes from typematic).
    NonRepeating,
    /// The keymap repeats this key; `None` when the HUD has no label for it.
    Repeating(Option<&'a str>),
}

impl RepeatState {
    fn new() -> Self {
        Self { held: None }
    }

    /// Record a press.
    ///
    /// Only a key the keymap says repeats takes over the timer, because that
    /// is what the kernel's typematic does: it repeats the most recent
    /// repeating key. A non-repeating key — a modifier above all — leaves the
    /// held key's repeat running, so pressing Shift while holding A keeps the
    /// counter ticking instead of freezing it. A repeating key the HUD has no
    /// label for still takes over, and cancels: the old key is no longer the
    /// one repeating, and inventing chips for an unlabeled keysym would be
    /// worse than showing none.
    fn on_press(&mut self, keycode: u32, device: &str, press: RepeatPress<'_>, now: Instant) {
        match press {
            RepeatPress::NonRepeating => {}
            RepeatPress::Repeating(label) => {
                self.held = label.map(|label| HeldKey {
                    keycode,
                    device: device.to_string(),
                    label: label.to_string(),
                    next_at: now + KEY_REPEAT_INITIAL_DELAY,
                });
            }
        }
    }

    /// Releasing the held key stops its repeat; other releases are unrelated.
    ///
    /// Matched on device *and* keycode: the same physical key on a second
    /// keyboard is a different key press, and letting it stop this repeat
    /// would freeze the counter on a key that is still held.
    fn on_release(&mut self, keycode: u32, device: &str) {
        if self
            .held
            .as_ref()
            .is_some_and(|held| held.keycode == keycode && held.device == device)
        {
            self.held = None;
        }
    }

    /// Drop the pending repeat only if the vanished device owns it. A held
    /// key on a keyboard that is still plugged in keeps repeating.
    fn cancel_device(&mut self, device: &str) {
        if self.held.as_ref().is_some_and(|held| held.device == device) {
            self.held = None;
        }
    }

    /// Poll timeout until the next repeat fires, or `None` to block.
    fn timeout(&self, now: Instant) -> Option<Duration> {
        self.held
            .as_ref()
            .map(|held| held.next_at.saturating_duration_since(now))
    }

    /// Fire a repeat if one is due, rescheduling from `now` so a long block
    /// does not burst-catch-up (the overlay's tick has the same policy).
    fn due(&mut self, now: Instant) -> Option<SystemInputEvent> {
        let held = self.held.as_mut()?;
        if now < held.next_at {
            return None;
        }
        held.next_at = now + KEY_REPEAT_INTERVAL;
        Some(SystemInputEvent::Key {
            label: held.label.clone(),
            bare_modifier: false,
        })
    }
}

enum PollOutcome {
    Ready,
    Stop,
    Timeout,
}

/// Block until libinput has events, the stop pipe is written, or `timeout`
/// (the pending key-repeat deadline) expires.
fn wait_for_input(
    libinput: &Libinput,
    stop_read: &OwnedFd,
    timeout: Option<Duration>,
) -> io::Result<PollOutcome> {
    // A repeat deadline in the past polls with 0 and returns Timeout at once.
    let timeout_ms = timeout.map_or(-1, |timeout| {
        timeout.as_millis().min(i32::MAX as u128) as i32
    });
    loop {
        let mut fds = [
            libc::pollfd {
                fd: libinput.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: stop_read.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: `fds` is a valid two-element array and both descriptors are
        // owned (or borrowed from an owner) for the duration of the call. A
        // negative timeout blocks until one becomes ready.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout_ms) };
        if ready < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }
        if ready == 0 {
            return Ok(PollOutcome::Timeout);
        }
        if fds[1].revents != 0 {
            return Ok(PollOutcome::Stop);
        }
        if fds[0].revents & (libc::POLLERR | libc::POLLNVAL | libc::POLLHUP) != 0 {
            return Err(io::Error::other(format!(
                "libinput poll descriptor failed with readiness {:#x}",
                fds[0].revents
            )));
        }
        if fds[0].revents & libc::POLLIN != 0 {
            return Ok(PollOutcome::Ready);
        }
    }
}

/// Turn one libinput event into a chip, updating the xkb state and the
/// repeat timer as it goes. Releases update both but produce no chip: the
/// HUD reports presses, not key-up transitions.
fn translate_event(
    event: Event,
    xkb_state: &mut xkb::State,
    repeat: &mut RepeatState,
    keys: &mut KeyboardKeys,
) -> Option<SystemInputEvent> {
    match event {
        Event::Keyboard(KeyboardEvent::Key(key_event)) => {
            let raw_keycode = key_event.key();
            let device = key_event.device().sysname().to_string();
            let keycode = xkb::Keycode::new(raw_keycode + EVDEV_KEYCODE_OFFSET);
            let pressed = key_event.key_state() == KeyState::Pressed;
            // Read the chord modifiers *before* folding this key into the
            // state, so pressing Z while Ctrl is held reads as "Ctrl+Z" and a
            // bare Shift press does not prefix itself.
            let modifiers = modifiers_from_xkb(xkb_state);
            if !pressed {
                repeat.on_release(raw_keycode, &device);
                // Only the last holder's release lifts the key: with Ctrl down
                // on two keyboards, letting go of one must not clear the
                // modifier the other still holds.
                if keys.release(&device, raw_keycode) {
                    xkb_state.update_key(keycode, xkb::KeyDirection::Up);
                }
                return None;
            }
            // ...and symmetrically, only the first holder presses it, so the
            // Down/Up transitions xkb sees stay balanced.
            if keys.press(&device, raw_keycode) {
                xkb_state.update_key(keycode, xkb::KeyDirection::Down);
            }
            let keysym = xkb_state.key_get_one_sym(keycode);
            let label_pair = key_label(keysym, modifiers);
            // Repeatability comes from the keymap, not the overlay's action
            // allowlist: that list exists to stop a held Return or Tab from
            // re-firing a one-shot *action*, while here the HUD only mirrors
            // what the focused app receives — and typematic does repeat those
            // keys.
            let press = if xkb_state.get_keymap().key_repeats(keycode) {
                RepeatPress::Repeating(label_pair.as_ref().map(|(label, _)| label.as_str()))
            } else {
                RepeatPress::NonRepeating
            };
            repeat.on_press(raw_keycode, &device, press, Instant::now());
            let (label, bare_modifier) = label_pair?;
            Some(SystemInputEvent::Key {
                label,
                bare_modifier,
            })
        }
        Event::Pointer(PointerEvent::Button(button_event)) => {
            if button_event.button_state() != ButtonState::Pressed {
                return None;
            }
            Some(SystemInputEvent::Mouse {
                label: button_label(button_event.button(), modifiers_from_xkb(xkb_state)),
            })
        }
        Event::Pointer(PointerEvent::ScrollWheel(scroll)) => {
            scroll_vertical(scroll.scroll_value(Axis::Vertical), xkb_state)
        }
        Event::Pointer(PointerEvent::ScrollFinger(scroll)) => {
            scroll_vertical(scroll.scroll_value(Axis::Vertical), xkb_state)
        }
        Event::Pointer(PointerEvent::ScrollContinuous(scroll)) => {
            scroll_vertical(scroll.scroll_value(Axis::Vertical), xkb_state)
        }
        // A stylus tip-down is the tablet equivalent of a click; the overlay's
        // own tablet hook is suppressed while the system source is active, so
        // dropping these here would lose Pen chips in system mode.
        // A keyboard can vanish (unplugged, or a VT/suspend teardown) while a
        // key or modifier is down, and no release for it will ever arrive.
        // Left alone that pins the repeat timer on forever and leaves xkb
        // modifiers latched, so every later chip would carry a phantom
        // "Ctrl+". Retire only what the vanished device held: another
        // keyboard may legitimately still be holding a key or modifier.
        Event::Device(DeviceEvent::Removed(removed)) => {
            let device = removed.device();
            if device.has_capability(DeviceCapability::Keyboard) {
                let sysname = device.sysname();
                repeat.cancel_device(sysname);
                keys.forget_device(sysname, xkb_state);
            }
            None
        }
        Event::Tablet(TabletToolEvent::Tip(tip)) => {
            if tip.tip_state() != TipState::Down {
                return None;
            }
            Some(SystemInputEvent::Mouse {
                label: pen_label(modifiers_from_xkb(xkb_state)),
            })
        }
        _ => None,
    }
}

fn scroll_vertical(value: f64, xkb_state: &xkb::State) -> Option<SystemInputEvent> {
    scroll_label(value, modifiers_from_xkb(xkb_state))
        .map(|label| SystemInputEvent::Scroll { label })
}

/// Send a terminal failure and wake the runtime so it tears the monitor down
/// promptly instead of on the next unrelated event.
fn report_failure(
    sender: &Sender<SystemInputEvent>,
    wake: &RuntimeWakeHandle,
    reason: SystemInputFailure,
) {
    if sender.send(SystemInputEvent::Failed(reason)).is_ok()
        && let Err(err) = wake.wake()
    {
        log::warn!("Failed to wake the runtime for a system input failure: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYBOARD: &str = "event0";

    /// A keymap-repeating press, labeled as the HUD would label it.
    fn press(repeat: &mut RepeatState, keycode: u32, label: Option<&str>, now: Instant) {
        repeat.on_press(keycode, KEYBOARD, RepeatPress::Repeating(label), now);
    }

    /// A press the keymap marks as non-repeating (modifiers and friends).
    fn press_non_repeating(repeat: &mut RepeatState, keycode: u32, now: Instant) {
        repeat.on_press(keycode, KEYBOARD, RepeatPress::NonRepeating, now);
    }

    #[test]
    fn a_repeatable_press_arms_the_initial_delay_then_ticks_at_the_interval() {
        let mut repeat = RepeatState::new();
        let start = Instant::now();
        press(&mut repeat, 14, Some("Backspace"), start);

        assert_eq!(repeat.timeout(start), Some(KEY_REPEAT_INITIAL_DELAY));
        assert!(
            repeat.due(start).is_none(),
            "nothing fires before the delay"
        );

        let first = start + KEY_REPEAT_INITIAL_DELAY;
        let chip = repeat.due(first).expect("repeat due after the delay");
        assert_eq!(
            chip,
            SystemInputEvent::Key {
                label: "Backspace".to_string(),
                bare_modifier: false,
            }
        );
        // Rescheduled from the fire time, at the steady interval.
        assert_eq!(repeat.timeout(first), Some(KEY_REPEAT_INTERVAL));
        assert!(repeat.due(first + KEY_REPEAT_INTERVAL).is_some());
    }

    #[test]
    fn a_repeating_press_takes_over_the_timer() {
        let mut repeat = RepeatState::new();
        let start = Instant::now();
        press(&mut repeat, 14, Some("Backspace"), start);

        // A repeating key the HUD cannot label still takes over, and cancels:
        // the old key is no longer the one the kernel repeats.
        press(&mut repeat, 99, None, start);
        assert_eq!(repeat.timeout(start), None);
        assert!(repeat.due(start + KEY_REPEAT_INITIAL_DELAY).is_none());

        // A labeled repeating press re-arms for the new key.
        press(&mut repeat, 30, Some("A"), start);
        let chip = repeat.due(start + KEY_REPEAT_INITIAL_DELAY);
        assert!(matches!(
            chip,
            Some(SystemInputEvent::Key { ref label, .. }) if label == "A"
        ));
    }

    /// Pressing a modifier while a key is held must not freeze the counter:
    /// typematic keeps running on the held key, so Shift during a held A
    /// leaves the repeat alone.
    #[test]
    fn a_non_repeating_press_leaves_the_held_key_repeating() {
        let mut repeat = RepeatState::new();
        let start = Instant::now();
        press(&mut repeat, 30, Some("A"), start);

        press_non_repeating(&mut repeat, 42, start);
        assert_eq!(repeat.timeout(start), Some(KEY_REPEAT_INITIAL_DELAY));
        let chip = repeat.due(start + KEY_REPEAT_INITIAL_DELAY);
        assert!(matches!(
            chip,
            Some(SystemInputEvent::Key { ref label, .. }) if label == "A"
        ));

        // Releasing the modifier is likewise unrelated to the held key.
        repeat.on_release(42, KEYBOARD);
        assert!(repeat.timeout(start).is_some());
    }

    /// The same physical key on a second keyboard is a different press, so
    /// its release must not stop this keyboard's repeat.
    #[test]
    fn another_keyboards_release_of_the_same_key_leaves_the_repeat_running() {
        let mut repeat = RepeatState::new();
        let start = Instant::now();
        press(&mut repeat, 14, Some("Backspace"), start);

        repeat.on_release(14, "event9");
        assert!(
            repeat.timeout(start).is_some(),
            "only the owning device's release ends the repeat"
        );

        repeat.on_release(14, KEYBOARD);
        assert_eq!(repeat.timeout(start), None);
    }

    /// A device can vanish with keys still down, so no release will arrive —
    /// but only *its own* repeat may be cancelled.
    #[test]
    fn only_the_vanished_devices_repeat_is_cancelled() {
        let mut repeat = RepeatState::new();
        let start = Instant::now();
        press(&mut repeat, 14, Some("Backspace"), start);

        repeat.cancel_device("event9");
        assert!(
            repeat.timeout(start).is_some(),
            "unplugging another keyboard must not stop this repeat"
        );

        repeat.cancel_device(KEYBOARD);
        assert_eq!(repeat.timeout(start), None);
        assert!(repeat.due(start + KEY_REPEAT_INITIAL_DELAY).is_none());
    }

    fn default_keymap() -> Option<xkb::Keymap> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        xkb::Keymap::new_from_names(&context, "", "", "", "", None, xkb::KEYMAP_COMPILE_NO_FLAGS)
    }

    /// evdev KEY_LEFTCTRL.
    const CTRL_KEYCODE: u32 = 29;

    /// A press/release pair is reported to xkb once per *keycode*, not once
    /// per device. libxkbcommon counts transitions: two Downs followed by one
    /// Up leave the modifier latched forever, which is exactly what a key held
    /// on two keyboards would produce without this gating.
    #[test]
    fn xkb_down_and_up_must_be_balanced_per_keycode() {
        let Some(keymap) = default_keymap() else {
            eprintln!("no default xkb keymap available; skipping");
            return;
        };
        let mut state = xkb::State::new(&keymap);
        let ctrl = xkb::Keycode::new(CTRL_KEYCODE + EVDEV_KEYCODE_OFFSET);

        // The hazard this gating exists for, demonstrated directly.
        state.update_key(ctrl, xkb::KeyDirection::Down);
        state.update_key(ctrl, xkb::KeyDirection::Down);
        state.update_key(ctrl, xkb::KeyDirection::Up);
        assert!(
            modifiers_from_xkb(&state).ctrl,
            "unbalanced transitions leave the modifier stuck"
        );

        // The ledger reports only the first press and the last release, so the
        // same two-keyboard sequence stays balanced.
        let mut state = xkb::State::new(&keymap);
        let mut keys = KeyboardKeys::default();
        assert!(keys.press("event0", CTRL_KEYCODE), "first holder presses");
        state.update_key(ctrl, xkb::KeyDirection::Down);
        assert!(
            !keys.press("event9", CTRL_KEYCODE),
            "a second holder must not press again"
        );

        assert!(
            !keys.release("event9", CTRL_KEYCODE),
            "the other keyboard still holds it"
        );
        assert!(modifiers_from_xkb(&state).ctrl);

        assert!(keys.release("event0", CTRL_KEYCODE), "last holder releases");
        state.update_key(ctrl, xkb::KeyDirection::Up);
        assert!(!modifiers_from_xkb(&state).ctrl);
    }

    /// Unplugging a keyboard releases exactly what it held: a modifier still
    /// down on another keyboard survives, so later chords are not stripped of
    /// their "Ctrl+".
    #[test]
    fn removing_one_keyboard_keeps_another_keyboards_modifier_held() {
        let Some(keymap) = default_keymap() else {
            eprintln!("no default xkb keymap available; skipping");
            return;
        };
        let mut state = xkb::State::new(&keymap);
        let mut keys = KeyboardKeys::default();

        // Ctrl held on the built-in keyboard, another key on an external one.
        for (device, keycode) in [("event0", CTRL_KEYCODE), ("event9", 30)] {
            assert!(keys.press(device, keycode));
            state.update_key(
                xkb::Keycode::new(keycode + EVDEV_KEYCODE_OFFSET),
                xkb::KeyDirection::Down,
            );
        }
        assert!(modifiers_from_xkb(&state).ctrl);

        keys.forget_device("event9", &mut state);
        assert!(
            modifiers_from_xkb(&state).ctrl,
            "the surviving keyboard still holds Ctrl"
        );

        // Removing the keyboard that actually holds Ctrl does clear it.
        keys.forget_device("event0", &mut state);
        assert!(!modifiers_from_xkb(&state).ctrl);
    }

    /// The same keycode held on two keyboards survives losing one: the
    /// orphan sweep skips keys another device still holds.
    #[test]
    fn a_modifier_held_on_two_keyboards_survives_losing_one() {
        let Some(keymap) = default_keymap() else {
            eprintln!("no default xkb keymap available; skipping");
            return;
        };
        let mut state = xkb::State::new(&keymap);
        let mut keys = KeyboardKeys::default();

        assert!(keys.press("event0", CTRL_KEYCODE));
        state.update_key(
            xkb::Keycode::new(CTRL_KEYCODE + EVDEV_KEYCODE_OFFSET),
            xkb::KeyDirection::Down,
        );
        assert!(!keys.press("event9", CTRL_KEYCODE));

        keys.forget_device("event9", &mut state);
        assert!(modifiers_from_xkb(&state).ctrl);

        keys.forget_device("event0", &mut state);
        assert!(!modifiers_from_xkb(&state).ctrl);
    }

    /// Device removal must not reset the whole keyboard state: locked
    /// modifiers (Caps Lock) and the selected layout group belong to the
    /// session, not to the vanished device.
    #[test]
    fn removing_a_keyboard_preserves_locked_modifiers() {
        let Some(keymap) = default_keymap() else {
            eprintln!("no default xkb keymap available; skipping");
            return;
        };
        let mut state = xkb::State::new(&keymap);
        let mut keys = KeyboardKeys::default();

        // evdev KEY_CAPSLOCK is 58: press and release to latch the lock.
        let caps = xkb::Keycode::new(58 + EVDEV_KEYCODE_OFFSET);
        state.update_key(caps, xkb::KeyDirection::Down);
        state.update_key(caps, xkb::KeyDirection::Up);
        let locked_before = state.mod_name_is_active(xkb::MOD_NAME_CAPS, xkb::STATE_MODS_LOCKED);
        if !locked_before {
            eprintln!("keymap does not lock Caps Lock; skipping");
            return;
        }

        assert!(keys.press("event9", 30));
        state.update_key(
            xkb::Keycode::new(30 + EVDEV_KEYCODE_OFFSET),
            xkb::KeyDirection::Down,
        );
        keys.forget_device("event9", &mut state);

        assert!(
            state.mod_name_is_active(xkb::MOD_NAME_CAPS, xkb::STATE_MODS_LOCKED),
            "Caps Lock survives unplugging an unrelated keyboard"
        );
    }

    /// Removing a device that holds nothing touches no xkb state at all, so
    /// an idle unplug cannot disturb the modifiers in flight.
    #[test]
    fn forgetting_an_idle_device_leaves_xkb_untouched() {
        let Some(keymap) = default_keymap() else {
            eprintln!("no default xkb keymap available; skipping");
            return;
        };
        let mut state = xkb::State::new(&keymap);
        let mut keys = KeyboardKeys::default();

        // Ctrl is held on a keyboard that stays plugged in.
        assert!(keys.press("event0", CTRL_KEYCODE));
        state.update_key(
            xkb::Keycode::new(CTRL_KEYCODE + EVDEV_KEYCODE_OFFSET),
            xkb::KeyDirection::Down,
        );

        // An unknown device, and one whose keys were all released.
        keys.forget_device("event7", &mut state);
        assert!(keys.press("event9", 30));
        assert!(keys.release("event9", 30));
        keys.forget_device("event9", &mut state);

        assert!(modifiers_from_xkb(&state).ctrl);
    }

    /// Explicit `system` mode gets the real reason; the group hint appears
    /// only where group membership is a plausible cause.
    #[test]
    fn failure_messages_name_the_actual_cause() {
        let unreadable = SystemInputFailure::DevicesUnreadable {
            seat: "seat0".to_string(),
        }
        .user_message();
        assert!(unreadable.contains("seat0"));
        assert!(
            unreadable.contains("usermod"),
            "unreadable nodes are what the group fixes"
        );

        // An empty seat is a different problem: no group change helps.
        let empty = SystemInputFailure::NoUsableDevices {
            seat: "seat0".to_string(),
        }
        .user_message();
        assert!(empty.contains("seat0"));
        assert!(!empty.contains("usermod"));

        let start =
            SystemInputFailure::StartFailed("too many open files".to_string()).user_message();
        assert!(start.contains("too many open files"));
        assert!(
            !start.contains("usermod"),
            "resource exhaustion is not a permission problem"
        );

        let keymap = SystemInputFailure::KeymapUnavailable.user_message();
        assert!(keymap.contains("layout"));
        assert!(
            !keymap.contains("usermod"),
            "a broken layout is not fixed by joining a group"
        );

        let read = SystemInputFailure::ReadFailed("poll failed".to_string()).user_message();
        assert!(read.contains("poll failed"));
        assert!(!read.contains("usermod"));

        let seat = SystemInputFailure::SeatUnavailable {
            seat: "seat1".to_string(),
        }
        .user_message();
        assert!(seat.contains("seat1"));
    }

    #[test]
    fn only_the_held_keys_release_stops_its_repeat() {
        let mut repeat = RepeatState::new();
        let start = Instant::now();
        press(&mut repeat, 14, Some("Backspace"), start);

        // Releasing an unrelated key (e.g. one that was down before the hold
        // began) leaves the repeat armed.
        repeat.on_release(42, KEYBOARD);
        assert!(repeat.timeout(start).is_some());

        repeat.on_release(14, KEYBOARD);
        assert_eq!(repeat.timeout(start), None);
        assert!(repeat.due(start + KEY_REPEAT_INITIAL_DELAY).is_none());
    }
}
