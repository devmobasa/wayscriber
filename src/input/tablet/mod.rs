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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        )
    }

    #[test]
    fn apply_pressure_to_state_ignores_disabled_tablet_settings() {
        let mut state = make_state();
        state.current_thickness = 3.0;
        state.needs_redraw = false;

        apply_pressure_to_state(0.8, &mut state, TabletSettings::default());

        assert_eq!(state.current_thickness, 3.0);
        assert!(!state.needs_redraw);
    }

    #[test]
    fn apply_pressure_to_state_ignores_disabled_pressure_mapping() {
        let mut state = make_state();
        state.needs_redraw = false;
        let settings = TabletSettings {
            enabled: true,
            pressure_enabled: false,
            min_thickness: 1.0,
            max_thickness: 8.0,
        };

        apply_pressure_to_state(0.8, &mut state, settings);

        assert_eq!(state.current_thickness, 4.0);
        assert!(!state.needs_redraw);
    }

    #[test]
    fn apply_pressure_to_state_clamps_pressure_to_configured_range() {
        let mut state = make_state();
        state.needs_redraw = false;
        let settings = TabletSettings {
            enabled: true,
            pressure_enabled: true,
            min_thickness: 2.0,
            max_thickness: 6.0,
        };

        apply_pressure_to_state(-1.0, &mut state, settings);
        assert_eq!(state.current_thickness, 2.0);
        assert!(state.needs_redraw);

        state.needs_redraw = false;
        apply_pressure_to_state(2.0, &mut state, settings);
        assert_eq!(state.current_thickness, 6.0);
        assert!(state.needs_redraw);
    }

    #[test]
    fn apply_pressure_to_state_clamps_to_global_max_thickness() {
        let mut state = make_state();
        let settings = TabletSettings {
            enabled: true,
            pressure_enabled: true,
            min_thickness: 1.0,
            max_thickness: MAX_STROKE_THICKNESS + 50.0,
        };

        apply_pressure_to_state(1.0, &mut state, settings);

        assert_eq!(state.current_thickness, MAX_STROKE_THICKNESS);
        assert!(state.needs_redraw);
    }
}
