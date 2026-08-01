use super::*;

mod repeat;

use repeat::*;

/// Non-blocking, close-on-exec pipe used only to interrupt the reader's poll.
pub(super) fn stop_pipe() -> io::Result<(OwnedFd, OwnedFd)> {
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
pub(super) fn run(stop_read: OwnedFd, sender: Sender<SystemInputEvent>, wake: RuntimeWakeHandle) {
    let seat = super::super::probe::current_seat();
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
            let failure = match super::super::probe::event_node_access() {
                super::super::probe::EventNodeAccess::Unreadable => {
                    SystemInputFailure::DevicesUnreadable { seat }
                }
                super::super::probe::EventNodeAccess::None => {
                    SystemInputFailure::NoUsableDevices { seat }
                }
                super::super::probe::EventNodeAccess::Unknown
                | super::super::probe::EventNodeAccess::Readable => {
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
mod tests;
