//! Tablet/stylus input helpers (Wayland tablet-unstable-v2).
//!
//! These helpers map tablet tool events (position, pressure) into the existing
//! `InputState` without changing the drawing logic.

use crate::input::{InputState, state::MAX_STROKE_THICKNESS};

/// User-configurable settings for tablet input.
#[derive(Debug, Clone, Copy)]
pub struct TabletSettings {
    /// Enable tablet handling at runtime (feature must be compiled in too).
    pub enabled: bool,
    /// Whether to map pressure to thickness.
    pub pressure_enabled: bool,
    /// Minimum thickness when pressure is near 0.
    pub min_thickness: f64,
    /// Maximum thickness when pressure is 1.0.
    pub max_thickness: f64,
}

impl Default for TabletSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            pressure_enabled: true,
            min_thickness: 1.0,
            max_thickness: 8.0,
        }
    }
}

/// Apply a normalized pressure value [0.0, 1.0] to the current thickness.
pub fn apply_pressure_to_state(pressure01: f64, state: &mut InputState, settings: TabletSettings) {
    if !settings.enabled || !settings.pressure_enabled {
        return;
    }

    let p = pressure01.clamp(0.0, 1.0);
    let thick = settings.min_thickness + (settings.max_thickness - settings.min_thickness) * p;
    let new_thickness = thick.clamp(1.0, MAX_STROKE_THICKNESS);

    if (new_thickness - state.current_thickness).abs() > 0.1 {
        log::debug!(
            "Pressure {} → thickness {:.1}px (range: {:.1}-{:.1})",
            p,
            new_thickness,
            settings.min_thickness,
            settings.max_thickness
        );
    }

    state.current_thickness = new_thickness;
    state.needs_redraw = true;
}
