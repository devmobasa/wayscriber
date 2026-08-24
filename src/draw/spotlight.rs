//! Shared Spotlight magnification values.

/// Historical and configured default: an ordinary, unmagnified Spotlight.
pub const DEFAULT_SPOTLIGHT_MAGNIFICATION: f64 = 1.0;
pub const MIN_SPOTLIGHT_MAGNIFICATION: f64 = 1.0;
pub const MAX_SPOTLIGHT_MAGNIFICATION: f64 = 4.0;
pub const SPOTLIGHT_MAGNIFICATION_STEP: f64 = 0.25;

/// Serde default for Spotlight shapes written before magnification existed.
pub const fn default_spotlight_magnification() -> f64 {
    DEFAULT_SPOTLIGHT_MAGNIFICATION
}

pub fn normalize_spotlight_magnification(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(MIN_SPOTLIGHT_MAGNIFICATION, MAX_SPOTLIGHT_MAGNIFICATION)
    } else {
        DEFAULT_SPOTLIGHT_MAGNIFICATION
    }
}

pub fn spotlight_magnification_is_active(value: f64) -> bool {
    normalize_spotlight_magnification(value) > MIN_SPOTLIGHT_MAGNIFICATION + f64::EPSILON
}

pub fn format_spotlight_magnification(value: f64) -> String {
    let value = normalize_spotlight_magnification(value);
    let mut number = format!("{value:.2}");
    while number.ends_with('0') {
        number.pop();
    }
    if number.ends_with('.') {
        number.pop();
    }
    format!("{number}x")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnification_normalization_defaults_non_finite_and_clamps_the_supported_range() {
        assert_eq!(normalize_spotlight_magnification(f64::NAN), 1.0);
        assert_eq!(normalize_spotlight_magnification(0.5), 1.0);
        assert_eq!(normalize_spotlight_magnification(2.25), 2.25);
        assert_eq!(normalize_spotlight_magnification(8.0), 4.0);
    }

    #[test]
    fn magnification_formatting_keeps_quarter_steps_without_noise() {
        assert_eq!(format_spotlight_magnification(1.0), "1x");
        assert_eq!(format_spotlight_magnification(1.5), "1.5x");
        assert_eq!(format_spotlight_magnification(2.25), "2.25x");
    }

    #[test]
    fn magnification_is_active_only_above_the_unmagnified_default() {
        assert!(!spotlight_magnification_is_active(f64::NAN));
        assert!(!spotlight_magnification_is_active(1.0));
        assert!(spotlight_magnification_is_active(1.01));
    }
}
