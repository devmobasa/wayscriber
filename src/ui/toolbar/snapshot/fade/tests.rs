use super::*;
use crate::ui::anim::override_motion_for_test;

fn secs(value: u64) -> Duration {
    Duration::from_secs(value)
}

/// Idle inputs: `idle_secs` since the last stroke, nothing holding the strip.
fn inputs(idle_secs: u64) -> TopStripFadeInputs {
    TopStripFadeInputs {
        idle_for: secs(idle_secs),
        pointer_near: false,
        menus_open: false,
        reduced_chrome: false,
        idle_fade_enabled: true,
    }
}

fn near(idle_secs: u64) -> TopStripFadeInputs {
    TopStripFadeInputs {
        pointer_near: true,
        ..inputs(idle_secs)
    }
}

/// Drive a fresh engine to the settled hidden state at `start`.
fn hidden_fade(start: Instant) -> TopStripFade {
    let mut fade = TopStripFade::new();
    fade.update(&inputs(10), start);
    fade.update(&inputs(10), start + TOP_STRIP_FADE_OUT);
    assert_eq!(fade.value(), TOP_STRIP_HIDDEN_LEVEL);
    assert!(!fade.animating());
    fade
}

#[test]
fn hidden_level_is_the_input_passthrough_threshold() {
    assert!(top_strip_hidden(TOP_STRIP_HIDDEN_LEVEL));
    assert!(!top_strip_hidden(0.01));
    assert!(!top_strip_hidden(TOP_STRIP_SHOWN_LEVEL));
}

#[test]
fn strip_hides_after_the_idle_delay_and_reveals_on_pointer_approach() {
    let _motion = override_motion_for_test(true);
    let mut fade = TopStripFade::new();
    let start = Instant::now();

    // Not yet idle long enough: shown, with a wakeup at the 4s mark.
    assert_eq!(fade.update(&inputs(1), start), TOP_STRIP_SHOWN_LEVEL);
    assert!(!fade.animating());
    assert_eq!(
        fade.wake_after(&inputs(1), start),
        Some(TOP_STRIP_IDLE_DELAY - secs(1))
    );

    // Past the idle delay: the fade-out starts (still shown for zero
    // elapsed time) and requests animation ticks.
    let hide_start = start + Duration::from_millis(1);
    assert_eq!(fade.update(&inputs(5), hide_start), TOP_STRIP_SHOWN_LEVEL);
    assert!(fade.animating());
    assert_eq!(
        fade.wake_after(&inputs(5), hide_start),
        Some(TOP_STRIP_FADE_TICK)
    );

    // Halfway through the fade-out: an intermediate value.
    let mid = fade.update(&inputs(5), hide_start + TOP_STRIP_FADE_OUT / 2);
    assert!(mid > TOP_STRIP_HIDDEN_LEVEL && mid < TOP_STRIP_SHOWN_LEVEL);

    // The transition settles fully hidden and stops ticking; a hidden strip
    // waits for input instead of polling.
    let hidden_at = hide_start + TOP_STRIP_FADE_OUT;
    assert_eq!(fade.update(&inputs(5), hidden_at), TOP_STRIP_HIDDEN_LEVEL);
    assert!(top_strip_hidden(fade.value()));
    assert!(!fade.animating());
    assert_eq!(fade.wake_after(&inputs(5), hidden_at), None);

    // The pointer entering the reveal zone brings it back (faster envelope).
    assert_eq!(fade.wake_after(&near(9), hidden_at), Some(Duration::ZERO));
    assert_eq!(fade.update(&near(9), hidden_at), TOP_STRIP_HIDDEN_LEVEL);
    assert!(fade.animating(), "reveal transition in flight");
    let rising = fade.update(&near(9), hidden_at + TOP_STRIP_RESTORE / 2);
    assert!(rising > TOP_STRIP_HIDDEN_LEVEL && rising < TOP_STRIP_SHOWN_LEVEL);
    assert!(!top_strip_hidden(rising), "a revealing strip takes input");
    let shown_at = hidden_at + TOP_STRIP_RESTORE;
    assert_eq!(fade.update(&near(9), shown_at), TOP_STRIP_SHOWN_LEVEL);
    assert!(!fade.animating());
    // Held by the pointer: nothing to wake for.
    assert_eq!(fade.wake_after(&near(9), shown_at), None);
}

#[test]
fn strip_lingers_for_the_idle_delay_after_the_pointer_leaves() {
    let _motion = override_motion_for_test(true);
    let start = Instant::now();
    let mut fade = hidden_fade(start);

    // Reveal it with the pointer long after the last stroke.
    let revealed = start + secs(1);
    fade.update(&near(30), revealed);
    fade.update(&near(30), revealed + TOP_STRIP_RESTORE);
    assert_eq!(fade.value(), TOP_STRIP_SHOWN_LEVEL);

    // The pointer leaves: the strip stays up for the full idle delay even
    // though the last stroke is much older, and the loop sleeps until then.
    let left = revealed + secs(2);
    fade.update(&near(32), left);
    assert_eq!(fade.update(&inputs(32), left), TOP_STRIP_SHOWN_LEVEL);
    assert_eq!(
        fade.wake_after(&inputs(32), left),
        Some(TOP_STRIP_IDLE_DELAY)
    );

    let almost = left + TOP_STRIP_IDLE_DELAY - Duration::from_millis(1);
    assert_eq!(fade.update(&inputs(36), almost), TOP_STRIP_SHOWN_LEVEL);
    assert!(!fade.animating());

    let deadline = left + TOP_STRIP_IDLE_DELAY;
    fade.update(&inputs(36), deadline);
    assert!(fade.animating(), "the hide starts at the deadline");
    assert_eq!(
        fade.update(&inputs(36), deadline + TOP_STRIP_FADE_OUT),
        TOP_STRIP_HIDDEN_LEVEL
    );
}

#[test]
fn drawing_keeps_a_shown_strip_up_but_never_reveals_a_hidden_one() {
    let _motion = override_motion_for_test(true);
    let start = Instant::now();
    let mut shown = TopStripFade::new();

    // Strokes keep resetting the idle clock: the shown strip stays up.
    for offset in [0, 3, 6, 9] {
        let at = start + secs(offset);
        assert_eq!(shown.update(&inputs(0), at), TOP_STRIP_SHOWN_LEVEL);
        assert!(!shown.animating());
    }

    // Once hidden, a new stroke leaves it hidden, with no wakeup scheduled.
    let mut hidden = hidden_fade(start);
    let stroke = start + secs(20);
    assert_eq!(hidden.update(&inputs(0), stroke), TOP_STRIP_HIDDEN_LEVEL);
    assert!(!hidden.animating());
    assert_eq!(hidden.wake_after(&inputs(0), stroke), None);
}

#[test]
fn reveal_pulse_shows_a_hidden_strip_briefly() {
    let _motion = override_motion_for_test(true);
    let start = Instant::now();
    let mut fade = hidden_fade(start);

    // A keyboard tool change requests a reveal: the loop wakes at once.
    let key = start + secs(1);
    fade.reveal_briefly(key);
    assert_eq!(fade.wake_after(&inputs(30), key), Some(Duration::ZERO));
    fade.update(&inputs(30), key);
    assert!(fade.animating());
    let shown_at = key + TOP_STRIP_RESTORE;
    assert_eq!(fade.update(&inputs(30), shown_at), TOP_STRIP_SHOWN_LEVEL);

    // It stays up only for the pulse, not a full idle delay.
    assert_eq!(
        fade.wake_after(&inputs(30), shown_at),
        Some(TOP_STRIP_REVEAL_PULSE - TOP_STRIP_RESTORE)
    );
    let before_end = key + TOP_STRIP_REVEAL_PULSE - Duration::from_millis(1);
    assert_eq!(fade.update(&inputs(31), before_end), TOP_STRIP_SHOWN_LEVEL);
    let pulse_end = key + TOP_STRIP_REVEAL_PULSE;
    fade.update(&inputs(31), pulse_end);
    assert!(fade.animating(), "hides once the pulse ends");
    assert_eq!(
        fade.update(&inputs(31), pulse_end + TOP_STRIP_FADE_OUT),
        TOP_STRIP_HIDDEN_LEVEL
    );
}

#[test]
fn reveal_pulse_outlasts_an_idle_deadline_that_expires_first() {
    let _motion = override_motion_for_test(true);
    let mut fade = TopStripFade::new();
    let start = Instant::now();

    // Shown with 3.5s of idle time: the pulse (1.5s) outlasts the 0.5s left.
    fade.update(&inputs(3), start);
    fade.reveal_briefly(start);
    let idle = TopStripFadeInputs {
        idle_for: Duration::from_millis(3500),
        ..inputs(0)
    };
    fade.update(&idle, start);
    assert_eq!(fade.wake_after(&idle, start), Some(TOP_STRIP_REVEAL_PULSE));
}

#[test]
fn menus_and_reduced_chrome_hold_and_reveal_the_strip() {
    let _motion = override_motion_for_test(true);
    let start = Instant::now();

    let mut menu_open = inputs(30);
    menu_open.menus_open = true;
    let mut fade = TopStripFade::new();
    assert_eq!(fade.update(&menu_open, start), TOP_STRIP_SHOWN_LEVEL);
    assert_eq!(fade.wake_after(&menu_open, start), None, "no hide pending");

    // An open menu also brings a hidden strip back.
    let mut hidden = hidden_fade(start);
    let opened = start + secs(1);
    hidden.update(&menu_open, opened);
    assert_eq!(
        hidden.update(&menu_open, opened + TOP_STRIP_RESTORE),
        TOP_STRIP_SHOWN_LEVEL
    );

    let mut minimal = inputs(30);
    minimal.reduced_chrome = true;
    let mut fade = TopStripFade::new();
    assert_eq!(fade.update(&minimal, start), TOP_STRIP_SHOWN_LEVEL);
    assert_eq!(fade.wake_after(&minimal, start), None);
}

#[test]
fn stalled_pending_transitions_request_an_immediate_wake() {
    let _motion = override_motion_for_test(true);
    let mut fade = TopStripFade::new();
    let start = Instant::now();

    // The last update ran before the idle deadline, so the latch still says
    // shown and the envelope is settled...
    assert_eq!(fade.update(&inputs(3), start), TOP_STRIP_SHOWN_LEVEL);
    assert!(!fade.animating());
    // ...and by the time the loop computes its timeout the deadline has
    // passed. The stalled hide must request an immediate wake instead of
    // letting dispatch block until an arbitrary event.
    assert_eq!(fade.wake_after(&inputs(5), start), Some(Duration::ZERO));

    // Mirror case: hidden and settled, then a hold appears before any
    // update has cleared the latch.
    let fade = hidden_fade(start);
    assert_eq!(fade.wake_after(&near(10), start), Some(Duration::ZERO));
}

#[test]
fn reduced_motion_snaps_between_shown_and_hidden_without_ticking() {
    let _motion = override_motion_for_test(false);
    let mut fade = TopStripFade::new();
    let start = Instant::now();

    assert_eq!(fade.update(&inputs(1), start), TOP_STRIP_SHOWN_LEVEL);
    // The 4s idle trigger still needs its wakeup under reduced motion.
    assert_eq!(
        fade.wake_after(&inputs(1), start),
        Some(TOP_STRIP_IDLE_DELAY - secs(1))
    );

    // Hard snap: no intermediate values, no animation ticking.
    assert_eq!(fade.update(&inputs(5), start), TOP_STRIP_HIDDEN_LEVEL);
    assert!(!fade.animating());
    assert_eq!(fade.wake_after(&inputs(5), start), None);

    assert_eq!(fade.update(&near(9), start), TOP_STRIP_SHOWN_LEVEL);
    assert!(!fade.animating());
}

#[test]
fn disabled_idle_fade_stays_shown_and_does_not_schedule_a_wakeup() {
    let _motion = override_motion_for_test(true);
    let mut fade = TopStripFade::new();
    let start = Instant::now();
    let mut disabled = inputs(30);
    disabled.idle_fade_enabled = false;

    assert_eq!(fade.update(&disabled, start), TOP_STRIP_SHOWN_LEVEL);
    assert!(!fade.animating());
    assert_eq!(fade.wake_after(&disabled, start), None);

    // A reveal pulse on an always-visible strip schedules nothing either.
    fade.reveal_briefly(start);
    fade.update(&disabled, start);
    assert_eq!(fade.wake_after(&disabled, start), None);
}

#[test]
fn disabling_idle_fade_restores_a_hidden_strip() {
    let _motion = override_motion_for_test(true);
    let start = Instant::now();
    let mut fade = hidden_fade(start);

    let mut disabled = inputs(10);
    disabled.idle_fade_enabled = false;
    let toggled = start + TOP_STRIP_FADE_OUT;
    fade.update(&disabled, toggled);
    assert_eq!(
        fade.update(&disabled, toggled + TOP_STRIP_RESTORE),
        TOP_STRIP_SHOWN_LEVEL
    );
    assert_eq!(
        fade.wake_after(&disabled, toggled + TOP_STRIP_RESTORE),
        None
    );
}
