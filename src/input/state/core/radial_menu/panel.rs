use std::time::{Duration, Instant};

use super::layout::CENTER_RADIUS;
use super::{RADIAL_PAINT_DELAY, RadialMenuLayout, RadialMenuState};
use crate::config::RadialMenuMouseBinding;

/// Lifecycle, layout, and configured pointer trigger for the radial menu.
#[derive(Debug)]
pub struct RadialMenuPanel {
    pub(crate) state: RadialMenuState,
    pub(crate) layout: Option<RadialMenuLayout>,
    pub(crate) mouse_binding: RadialMenuMouseBinding,
}

impl RadialMenuPanel {
    pub fn is_open(&self) -> bool {
        matches!(self.state, RadialMenuState::Open { .. })
    }

    pub(crate) fn open(&mut self, x: f64, y: f64, now: Instant) {
        self.state = RadialMenuState::Open {
            center_x: x,
            center_y: y,
            hover: None,
            expanded_sub_ring: None,
            opened_at: now,
            painted: false,
            flick_armed: false,
            size_dragging: false,
        };
    }

    pub(crate) fn close(&mut self) -> bool {
        if !self.is_open() {
            return false;
        }
        self.state = RadialMenuState::Hidden;
        self.layout = None;
        true
    }

    pub fn paint_timeout(&self, now: Instant, redraw_pending: bool) -> Option<Duration> {
        match &self.state {
            RadialMenuState::Open {
                opened_at,
                painted: false,
                ..
            } => {
                let deadline = *opened_at + RADIAL_PAINT_DELAY;
                if now >= deadline && redraw_pending {
                    None
                } else {
                    Some(deadline.saturating_duration_since(now))
                }
            }
            _ => None,
        }
    }

    pub(crate) fn paint_due(&self, now: Instant) -> bool {
        matches!(
            self.state,
            RadialMenuState::Open {
                opened_at,
                painted: false,
                ..
            } if now >= opened_at + RADIAL_PAINT_DELAY
        )
    }

    pub(crate) fn mark_painted_if_due(&mut self, now: Instant) -> bool {
        let RadialMenuState::Open {
            opened_at, painted, ..
        } = &mut self.state
        else {
            return false;
        };
        if *painted {
            return true;
        }
        if now < *opened_at + RADIAL_PAINT_DELAY {
            return false;
        }
        *painted = true;
        true
    }

    pub fn has_painted(&self) -> bool {
        matches!(self.state, RadialMenuState::Open { painted: true, .. })
    }

    pub(crate) fn sample_flick(&mut self, x: f64, y: f64) {
        if let RadialMenuState::Open {
            center_x,
            center_y,
            flick_armed,
            size_dragging: false,
            ..
        } = &mut self.state
        {
            let dx = x - *center_x;
            let dy = y - *center_y;
            if (dx * dx + dy * dy).sqrt() > CENTER_RADIUS {
                *flick_armed = true;
            }
        }
    }

    pub fn is_size_dragging(&self) -> bool {
        matches!(
            self.state,
            RadialMenuState::Open {
                size_dragging: true,
                ..
            }
        )
    }

    pub(crate) fn set_size_dragging(&mut self, dragging: bool) {
        if let RadialMenuState::Open { size_dragging, .. } = &mut self.state {
            *size_dragging = dragging;
        }
    }
}

impl Default for RadialMenuPanel {
    fn default() -> Self {
        Self {
            state: RadialMenuState::Hidden,
            layout: None,
            mouse_binding: RadialMenuMouseBinding::Middle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_gate_transitions_once_after_the_delay() {
        let opened_at = Instant::now();
        let mut panel = RadialMenuPanel::default();
        panel.open(20.0, 30.0, opened_at);

        assert_eq!(
            panel.paint_timeout(opened_at, false),
            Some(RADIAL_PAINT_DELAY)
        );
        assert!(!panel.mark_painted_if_due(opened_at));
        assert!(panel.mark_painted_if_due(opened_at + RADIAL_PAINT_DELAY));
        assert!(panel.has_painted());
        assert_eq!(panel.paint_timeout(opened_at, false), None);
    }

    #[test]
    fn flick_arms_only_after_leaving_the_raw_center_deadzone() {
        let mut panel = RadialMenuPanel::default();
        panel.open(100.0, 100.0, Instant::now());
        panel.sample_flick(100.0 + CENTER_RADIUS, 100.0);
        assert!(matches!(
            panel.state,
            RadialMenuState::Open {
                flick_armed: false,
                ..
            }
        ));
        panel.sample_flick(101.0 + CENTER_RADIUS, 100.0);
        assert!(matches!(
            panel.state,
            RadialMenuState::Open {
                flick_armed: true,
                ..
            }
        ));
    }
}
