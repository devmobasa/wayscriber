use std::time::Duration;

use super::*;

fn default_state() -> ClickHighlightState {
    ClickHighlightState::new(ClickHighlightSettings::disabled())
}

#[test]
fn spawn_returns_false_when_disabled() {
    let mut state = default_state();
    let mut tracker = DirtyTracker::new();
    assert!(!state.spawn(100, 100, &mut tracker));
}

#[test]
fn toggle_enables_highlight() {
    let mut state = default_state();
    let mut tracker = DirtyTracker::new();
    assert!(state.toggle(&mut tracker));
    assert!(state.enabled());
}

#[test]
fn apply_pen_color_overrides_settings_when_enabled() {
    let mut settings = ClickHighlightSettings::disabled();
    settings.use_pen_color = true;
    let mut state = ClickHighlightState::new(settings);
    let pen = Color {
        r: 0.2,
        g: 0.4,
        b: 0.6,
        a: 1.0,
    };

    assert!(state.apply_pen_color(pen));
    assert_eq!(state.settings.fill_color.r, pen.r);
    assert_eq!(state.settings.fill_color.g, pen.g);
    assert_eq!(state.settings.fill_color.b, pen.b);
    assert_eq!(state.settings.fill_color.a, state.settings.base_fill_color.a);
}

#[test]
fn apply_pen_color_noop_when_disabled() {
    let mut settings = ClickHighlightSettings::disabled();
    settings.use_pen_color = false;
    let mut state = ClickHighlightState::new(settings.clone());
    let pen = Color {
        r: 0.6,
        g: 0.4,
        b: 0.2,
        a: 1.0,
    };

    assert!(!state.apply_pen_color(pen));
    assert_eq!(state.settings.fill_color, settings.base_fill_color);
    assert_eq!(state.settings.outline_color, settings.base_outline_color);
}

#[test]
fn apply_pen_color_idempotent() {
    let mut settings = ClickHighlightSettings::disabled();
    settings.use_pen_color = true;
    let mut state = ClickHighlightState::new(settings);
    let pen = Color {
        r: 0.4,
        g: 0.6,
        b: 0.8,
        a: 1.0,
    };

    assert!(state.apply_pen_color(pen));
    assert!(!state.apply_pen_color(pen));
}

#[test]
fn advance_drops_expired_highlights() {
    let mut settings = ClickHighlightSettings::disabled();
    settings.enabled = true;
    settings.duration = Duration::from_millis(10);
    let mut state = ClickHighlightState::new(settings);
    let mut tracker = DirtyTracker::new();
    assert!(state.spawn(0, 0, &mut tracker));
    if let Some(first) = state.highlights.first_mut() {
        first.started_at = Instant::now() - Duration::from_millis(20);
    }
    let alive = state.advance(Instant::now(), &mut tracker);
    assert!(!alive);
    assert!(!state.has_active());
}
