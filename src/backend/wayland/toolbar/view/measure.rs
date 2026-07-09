//! Text measurement abstraction so tree builders stay pure.
//!
//! Builders need text widths to place labels; production code measures with
//! Pango/Cairo while tests use a deterministic fake. Keeping this behind a
//! trait means golden dumps and layout invariants run without a rendering
//! stack.

// Consumed starting with the top-strip port; the allow is removed then.
#![allow(dead_code)]
/// Width measurement for a single line of toolbar text.
pub trait TextMeasure {
    /// Advance width in logical pixels for `text` at `size`, bold or not.
    fn text_width(&self, text: &str, size: f64, bold: bool) -> f64;
}

/// Deterministic measurement for tests and golden dumps: every character
/// advances a fixed fraction of the font size (bold slightly wider).
#[derive(Debug, Clone, Copy)]
pub struct FixedMeasure {
    /// Advance per character as a fraction of the font size.
    pub per_char: f64,
}

impl Default for FixedMeasure {
    fn default() -> Self {
        Self { per_char: 0.6 }
    }
}

impl TextMeasure for FixedMeasure {
    fn text_width(&self, text: &str, size: f64, bold: bool) -> f64 {
        let bold_factor = if bold { 1.08 } else { 1.0 };
        text.chars().count() as f64 * size * self.per_char * bold_factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_measure_is_deterministic_and_monotonic() {
        let measure = FixedMeasure::default();
        let short = measure.text_width("Undo", 13.0, false);
        let long = measure.text_width("Undo all", 13.0, false);
        let bold = measure.text_width("Undo", 13.0, true);

        assert!(long > short);
        assert!(bold > short);
        assert_eq!(short, measure.text_width("Undo", 13.0, false));
    }
}
