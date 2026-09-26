use std::time::{Duration, Instant};

use super::*;

const HOLD: Duration = Duration::from_millis(1200);
const FADE: Duration = Duration::from_millis(500);

fn ink() -> LaserInk {
    LaserInk::new(LaserSettings {
        style: LaserStyle {
            color: Color {
                r: 1.0,
                g: 0.1,
                b: 0.1,
                a: 1.0,
            },
            width: 6.0,
        },
        hold: HOLD,
        fade: FADE,
    })
}

fn stroke(x: i32) -> Vec<(i32, i32)> {
    vec![(x, 100), (x + 40, 120)]
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

#[test]
fn settings_follow_the_laser_config() {
    let config = LaserConfig {
        color: [0.2, 0.4, 0.6, 0.8],
        width: 9.0,
        hold_ms: 700,
        fade_ms: 300,
    };

    let settings = LaserSettings::from(&config);

    assert_eq!(settings.style.color.g, 0.4);
    assert_eq!(settings.style.color.a, 0.8);
    assert_eq!(settings.style.width, 9.0);
    assert_eq!(settings.hold, ms(700));
    assert_eq!(settings.fade, ms(300));
}

#[test]
fn committed_ink_stays_fully_visible_through_the_hold() {
    let mut ink = ink();
    let mut tracker = DirtyTracker::new();
    let start = Instant::now();
    ink.commit(stroke(0), start, &mut tracker);
    let _ = tracker.take_regions(1000, 1000);

    let animating = ink.advance(start + ms(1199), false, true, &mut tracker);

    assert!(!animating, "a hold is still, so it must not tick the loop");
    assert_eq!(ink.painted_opacity(), 1.0);
    assert!(
        tracker.take_regions(1000, 1000).is_empty(),
        "nothing changed on screen, so nothing is damaged"
    );
    assert_eq!(ink.wake_after(start + ms(1000), false, true), Some(ms(200)));
}

#[test]
fn ink_fades_after_the_hold_then_disappears() {
    let mut ink = ink();
    let mut tracker = DirtyTracker::new();
    let start = Instant::now();
    ink.commit(stroke(0), start, &mut tracker);

    assert!(ink.advance(start + HOLD + ms(250), false, true, &mut tracker));
    assert!((ink.painted_opacity() - 0.5).abs() < 1e-9);
    assert!(!tracker.take_regions(1000, 1000).is_empty());
    assert_eq!(
        ink.wake_after(start + HOLD + ms(250), false, true),
        None,
        "the animation tick drives a fade already in progress"
    );

    assert!(!ink.advance(start + HOLD + FADE, false, true, &mut tracker));
    assert!(!ink.has_ink());
    assert!(
        !tracker.take_regions(1000, 1000).is_empty(),
        "removing the ink must repaint where it was"
    );
    assert_eq!(ink.wake_after(start + HOLD + FADE, false, true), None);
}

#[test]
fn the_end_of_the_hold_wakes_the_loop_even_if_no_frame_ran_at_that_moment() {
    let mut ink = ink();
    let mut tracker = DirtyTracker::new();
    let start = Instant::now();
    ink.commit(stroke(0), start, &mut tracker);

    assert_eq!(
        ink.wake_after(start + HOLD + ms(10), false, true),
        Some(Duration::ZERO)
    );
    assert!(ink.advance(start + HOLD + ms(10), false, true, &mut tracker));
    assert_eq!(ink.wake_after(start + HOLD + ms(20), false, true), None);
}

#[test]
fn a_new_stroke_during_the_hold_keeps_the_whole_group_together() {
    let mut ink = ink();
    let mut tracker = DirtyTracker::new();
    let start = Instant::now();
    ink.commit(stroke(0), start, &mut tracker);

    ink.commit(stroke(200), start + ms(1000), &mut tracker);

    assert!(!ink.advance(start + HOLD + ms(100), false, true, &mut tracker));
    assert_eq!(ink.stroke_count(), 2);
    assert_eq!(ink.painted_opacity(), 1.0);
    assert!(!ink.advance(start + ms(1000) + HOLD + FADE, false, true, &mut tracker));
    assert_eq!(ink.stroke_count(), 0, "both strokes leave together");
}

#[test]
fn drawing_another_stroke_holds_the_group_and_revives_a_fade() {
    let mut ink = ink();
    let mut tracker = DirtyTracker::new();
    let start = Instant::now();
    ink.commit(stroke(0), start, &mut tracker);
    assert!(ink.advance(start + HOLD + ms(250), false, true, &mut tracker));
    let _ = tracker.take_regions(1000, 1000);

    let pen_down = start + HOLD + ms(300);
    assert!(!ink.advance(pen_down, true, true, &mut tracker));

    assert_eq!(ink.painted_opacity(), 1.0);
    assert!(
        !tracker.take_regions(1000, 1000).is_empty(),
        "returning to full opacity repaints the group"
    );
    assert_eq!(ink.wake_after(pen_down + ms(5000), true, true), None);
    assert!(!ink.advance(pen_down + ms(5000), true, true, &mut tracker));
    assert!(ink.has_ink(), "ink never expires while a stroke is drawn");
}

#[test]
fn reduced_motion_holds_still_and_then_removes_the_ink_in_one_step() {
    let mut ink = ink();
    let mut tracker = DirtyTracker::new();
    let start = Instant::now();
    ink.commit(stroke(0), start, &mut tracker);

    assert_eq!(ink.wake_after(start, false, false), Some(HOLD + FADE));
    assert!(!ink.advance(start + HOLD + ms(250), false, false, &mut tracker));
    assert_eq!(ink.painted_opacity(), 1.0);
    assert_eq!(
        ink.wake_after(start + HOLD + ms(250), false, false),
        Some(ms(250))
    );

    assert!(!ink.advance(start + HOLD + FADE, false, false, &mut tracker));
    assert!(!ink.has_ink());
}

#[test]
fn zero_fade_removes_the_ink_when_the_hold_ends() {
    let mut ink = ink();
    ink.settings.fade = Duration::ZERO;
    let mut tracker = DirtyTracker::new();
    let start = Instant::now();
    ink.commit(stroke(0), start, &mut tracker);

    assert_eq!(
        ink.wake_after(start + HOLD, false, true),
        Some(Duration::ZERO)
    );
    assert!(!ink.advance(start + HOLD, false, true, &mut tracker));
    assert!(!ink.has_ink());
}

#[test]
fn an_idle_ink_owner_needs_no_frames_and_no_wakeups() {
    let mut ink = ink();
    let mut tracker = DirtyTracker::new();

    assert!(!ink.advance(Instant::now(), false, true, &mut tracker));
    assert_eq!(ink.wake_after(Instant::now(), false, true), None);
    assert!(tracker.take_regions(1000, 1000).is_empty());
}

#[test]
fn the_group_is_capped_by_dropping_its_oldest_stroke() {
    let mut ink = ink();
    let mut tracker = DirtyTracker::new();
    let start = Instant::now();

    for index in 0..(MAX_LASER_STROKES as i32 + 3) {
        ink.commit(stroke(index * 10), start, &mut tracker);
    }

    assert_eq!(ink.stroke_count(), MAX_LASER_STROKES);
    assert_eq!(ink.strokes.front().map(|s| s.points[0]), Some((30, 100)));
}

#[test]
fn damage_covers_the_committed_stroke_and_clear_repaints_it() {
    let mut ink = ink();
    let mut tracker = DirtyTracker::new();
    ink.commit(stroke(0), Instant::now(), &mut tracker);

    let damage = tracker.take_regions(1000, 1000);
    assert!(damage.iter().any(|rect| rect.contains(20, 110)));

    ink.clear(&mut tracker);
    let cleared = tracker.take_regions(1000, 1000);
    assert!(cleared.iter().any(|rect| rect.contains(20, 110)));
    assert!(!ink.has_ink());
}

#[test]
fn an_empty_stroke_is_ignored() {
    let mut ink = ink();
    let mut tracker = DirtyTracker::new();

    ink.commit(Vec::new(), Instant::now(), &mut tracker);

    assert!(!ink.has_ink());
}
