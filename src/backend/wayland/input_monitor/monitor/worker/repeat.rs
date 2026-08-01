use super::*;

pub(super) struct RepeatState {
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
pub(super) struct KeyboardKeys {
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
    pub(super) fn press(&mut self, device: &str, keycode: u32) -> bool {
        let first = !self.is_held_anywhere(keycode);
        self.held
            .entry(device.to_string())
            .or_default()
            .insert(keycode);
        first
    }

    /// Record a release. Returns whether the key is now up everywhere, i.e.
    /// whether xkb should be told the key was released.
    pub(super) fn release(&mut self, device: &str, keycode: u32) -> bool {
        if let Some(keys) = self.held.get_mut(device) {
            keys.remove(&keycode);
            if keys.is_empty() {
                self.held.remove(device);
            }
        }
        !self.is_held_anywhere(keycode)
    }

    /// Whether any device still holds `keycode`.
    pub(super) fn is_held_anywhere(&self, keycode: u32) -> bool {
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
    pub(super) fn forget_device(&mut self, device: &str, xkb_state: &mut xkb::State) {
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
pub(super) enum RepeatPress<'a> {
    /// The keymap marks this key as non-repeating (modifiers, and anything
    /// else the layout excludes from typematic).
    NonRepeating,
    /// The keymap repeats this key; `None` when the HUD has no label for it.
    Repeating(Option<&'a str>),
}

impl RepeatState {
    pub(super) fn new() -> Self {
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
    pub(super) fn on_press(
        &mut self,
        keycode: u32,
        device: &str,
        press: RepeatPress<'_>,
        now: Instant,
    ) {
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
    pub(super) fn on_release(&mut self, keycode: u32, device: &str) {
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
    pub(super) fn cancel_device(&mut self, device: &str) {
        if self.held.as_ref().is_some_and(|held| held.device == device) {
            self.held = None;
        }
    }

    /// Poll timeout until the next repeat fires, or `None` to block.
    pub(super) fn timeout(&self, now: Instant) -> Option<Duration> {
        self.held
            .as_ref()
            .map(|held| held.next_at.saturating_duration_since(now))
    }

    /// Fire a repeat if one is due, rescheduling from `now` so a long block
    /// does not burst-catch-up (the overlay's tick has the same policy).
    pub(super) fn due(&mut self, now: Instant) -> Option<SystemInputEvent> {
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
