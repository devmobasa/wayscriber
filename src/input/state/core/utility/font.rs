use super::super::base::InputState;

impl InputState {
    /// Adjusts the current font size by a delta, clamping to valid range.
    ///
    /// Font size is clamped to 8.0-72.0px range (same as config validation).
    pub fn adjust_font_size(&mut self, delta: f64) {
        if !self.set_font_size(self.style.current_font_size + delta) {
            return;
        }

        log::debug!(
            "Font size adjusted to {:.1}px",
            self.style.current_font_size
        );
    }
}
