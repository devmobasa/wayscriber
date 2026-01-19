use super::Config;

mod arrow;
mod board;
mod boards;
mod drawing;
mod fonts;
mod history;
mod keybindings;
mod performance;
mod presets;
mod session;
#[cfg(tablet)]
mod tablet;
mod ui;

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
    /// - `buffer_count`: 2 - 4
    pub fn validate_and_clamp(&mut self) {
        self.validate_drawing();
        self.validate_presets();
        #[cfg(tablet)]
        self.validate_tablet();
        self.validate_history();
        self.validate_arrow();
        self.validate_performance();
        self.validate_fonts();
        self.validate_boards();
        self.validate_board();
        self.validate_ui();
        self.validate_keybindings();
        self.validate_session();
    }
}
