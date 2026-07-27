use super::Config;

mod arrow;
mod board;
mod boards;
mod drawing;
mod export;
mod fonts;
mod history;
mod keybindings;
mod performance;
mod presets;
mod render_profiles;
mod session;
mod spotlight;
#[cfg(feature = "tablet-input")]
mod tablet;
mod ui;
mod updates;

pub use keybindings::KeybindingConflictResolution;

/// What loading had to change before a configuration could be used.
///
/// Everything recorded here is session-only — the file keeps its authored text
/// — so callers are expected to show it rather than let it disappear into the
/// log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigValidationReport {
    /// Duplicate shortcuts resolved per binding while loading.
    pub keybinding_conflicts: Vec<KeybindingConflictResolution>,
}

impl ConfigValidationReport {
    /// Whether validation changed nothing the user needs to know about.
    pub fn is_empty(&self) -> bool {
        self.keybinding_conflicts.is_empty()
    }
}

impl Config {
    /// Validates and clamps all configuration values to acceptable ranges.
    ///
    /// This method ensures that user-provided config values won't cause undefined behavior
    /// or rendering issues. Invalid values are clamped to the nearest valid value and a
    /// warning is logged.
    ///
    /// Validated ranges:
    /// - `default_thickness`: 1.0 - 50.0
    /// - `default_font_size`: 8.0 - 72.0
    /// - `arrow.length`: 5.0 - 50.0
    /// - `arrow.angle_degrees`: 15.0 - 60.0
    /// - `spotlight.dim_opacity`: 0.1 - 0.95
    /// - `spotlight.feather`: 0.0 - 0.9
    /// - `buffer_count`: 2 - 4
    ///
    /// Returns what the user should be told about: a clamp is a silent
    /// correction, but a resolved keybinding conflict changes which shortcuts
    /// work and is never written back, so it has to be surfaced.
    pub fn validate_and_clamp(&mut self) -> ConfigValidationReport {
        self.validate_drawing();
        self.validate_presets();
        #[cfg(feature = "tablet-input")]
        self.validate_tablet();
        self.validate_history();
        self.validate_arrow();
        self.validate_spotlight();
        self.validate_performance();
        self.validate_fonts();
        self.validate_boards();
        self.validate_board();
        self.validate_ui();
        self.validate_render_profiles();
        self.validate_export();
        let keybinding_conflicts = self.validate_keybindings();
        self.validate_session();
        self.validate_updates();
        ConfigValidationReport {
            keybinding_conflicts,
        }
    }
}
