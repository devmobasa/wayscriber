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

    let start = SystemInputFailure::StartFailed("too many open files".to_string()).user_message();
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
