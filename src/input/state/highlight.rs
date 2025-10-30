use std::time::{Duration, Instant};

use crate::config::ClickHighlightConfig;
use crate::draw::{Color, DirtyTracker};
use crate::util::Rect;

const MAX_ACTIVE_HIGHLIGHTS: usize = 4;

/// Runtime settings for click highlight rendering.
#[derive(Clone)]
pub struct ClickHighlightSettings {
    pub enabled: bool,
    pub radius: f64,
    pub outline_thickness: f64,
    pub duration: Duration,
    pub fill_color: Color,
    pub outline_color: Color,
    pub base_fill_color: Color,
    pub base_outline_color: Color,
    pub use_pen_color: bool,
}

impl ClickHighlightSettings {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn disabled() -> Self {
        let base_fill = Color {
            r: 1.0,
            g: 0.8,
            b: 0.0,
            a: 0.35,
        };
        let base_outline = Color {
            r: 1.0,
            g: 0.6,
            b: 0.0,
            a: 0.9,
        };
        Self {
            enabled: false,
            radius: 24.0,
            outline_thickness: 4.0,
            duration: Duration::from_millis(750),
            fill_color: base_fill,
            outline_color: base_outline,
            base_fill_color: base_fill,
            base_outline_color: base_outline,
            use_pen_color: true,
        }
    }
}

impl From<&ClickHighlightConfig> for ClickHighlightSettings {
    fn from(cfg: &ClickHighlightConfig) -> Self {
        let fill = Color {
            r: cfg.fill_color[0],
            g: cfg.fill_color[1],
            b: cfg.fill_color[2],
            a: cfg.fill_color[3],
        };
        let outline = Color {
            r: cfg.outline_color[0],
            g: cfg.outline_color[1],
            b: cfg.outline_color[2],
            a: cfg.outline_color[3],
        };
        ClickHighlightSettings {
            enabled: cfg.enabled,
            radius: cfg.radius,
            outline_thickness: cfg.outline_thickness,
            duration: Duration::from_millis(cfg.duration_ms),
            fill_color: fill,
            outline_color: outline,
            base_fill_color: fill,
            base_outline_color: outline,
            use_pen_color: cfg.use_pen_color,
        }
    }
}

pub struct ClickHighlightState {
    settings: ClickHighlightSettings,
    enabled: bool,
    highlights: Vec<ActiveHighlight>,
}

struct ActiveHighlight {
    x: i32,
    y: i32,
    started_at: Instant,
    last_bounds: Option<Rect>,
}

impl ActiveHighlight {
    fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            started_at: Instant::now(),
            last_bounds: None,
        }
    }
}

impl ClickHighlightState {
    pub fn new(settings: ClickHighlightSettings) -> Self {
        let enabled = settings.enabled;
        Self {
            settings,
            enabled,
            highlights: Vec::new(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn uses_pen_color(&self) -> bool {
        self.settings.use_pen_color
    }

    pub fn toggle(&mut self, tracker: &mut DirtyTracker) -> bool {
        self.enabled = !self.enabled;
        if !self.enabled {
            self.clear_all(tracker);
        }
        tracker.mark_full();
        self.enabled
    }

    pub fn clear_all(&mut self, tracker: &mut DirtyTracker) {
        for highlight in &self.highlights {
            if let Some(bounds) = highlight.last_bounds {
                tracker.mark_rect(bounds);
            }
        }
        self.highlights.clear();
    }

    pub fn spawn(&mut self, x: i32, y: i32, tracker: &mut DirtyTracker) -> bool {
        if !self.enabled {
            return false;
        }

        if self.highlights.len() >= MAX_ACTIVE_HIGHLIGHTS {
            if let Some(removed) = self.highlights.first() {
                if let Some(bounds) = removed.last_bounds {
                    tracker.mark_rect(bounds);
                }
            }
            self.highlights.remove(0);
        }

        let mut highlight = ActiveHighlight::new(x, y);
        highlight.last_bounds = Self::bounds_for(&self.settings, x, y);
        if let Some(bounds) = highlight.last_bounds {
            tracker.mark_rect(bounds);
        }
        self.highlights.push(highlight);
        true
    }

    pub fn has_active(&self) -> bool {
        !self.highlights.is_empty()
    }

    pub fn advance(&mut self, now: Instant, tracker: &mut DirtyTracker) -> bool {
        if self.highlights.is_empty() {
            return false;
        }

        let mut has_alive = false;
        let duration = self.settings.duration;
        let settings = self.settings.clone();

        self.highlights.retain_mut(|highlight| {
            let elapsed = now.saturating_duration_since(highlight.started_at);
            let alive = elapsed < duration;

            let bounds = Self::bounds_for(&settings, highlight.x, highlight.y);
            if let Some(bounds) = bounds {
                tracker.mark_rect(bounds);
                highlight.last_bounds = Some(bounds);
            }

            if alive {
                has_alive = true;
            } else if let Some(prev) = highlight.last_bounds {
                tracker.mark_rect(prev);
            }

            alive
        });

        has_alive
    }

    pub fn render(&self, ctx: &cairo::Context, now: Instant) {
        if self.highlights.is_empty() {
            return;
        }

        let total = self.settings.duration.as_secs_f64();
        for highlight in &self.highlights {
            let elapsed = now.saturating_duration_since(highlight.started_at);
            let progress = (elapsed.as_secs_f64() / total).clamp(0.0, 1.0);
            let fade = (1.0 - progress).clamp(0.0, 1.0);

            crate::draw::render_click_highlight(
                ctx,
                highlight.x as f64,
                highlight.y as f64,
                self.settings.radius,
                self.settings.outline_thickness,
                self.settings.fill_color,
                self.settings.outline_color,
                fade,
            );
        }
    }

    fn bounds_for(settings: &ClickHighlightSettings, x: i32, y: i32) -> Option<Rect> {
        let radius = settings.radius + settings.outline_thickness;
        let extent = radius.ceil() as i32 + 2; // small padding for anti-aliased edges
        let size = extent * 2;
        Rect::new(x - extent, y - extent, size, size)
    }

    pub fn apply_pen_color(&mut self, pen: Color) -> bool {
        if !self.settings.use_pen_color {
            return false;
        }

        let new_fill = Color {
            r: pen.r,
            g: pen.g,
            b: pen.b,
            a: self.settings.base_fill_color.a,
        };
        let new_outline = Color {
            r: pen.r,
            g: pen.g,
            b: pen.b,
            a: self.settings.base_outline_color.a,
        };

        if self.settings.fill_color == new_fill && self.settings.outline_color == new_outline {
            return false;
        }

        self.settings.fill_color = new_fill;
        self.settings.outline_color = new_outline;
        true
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(
            state.settings.fill_color.a,
            state.settings.base_fill_color.a
        );
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
}
