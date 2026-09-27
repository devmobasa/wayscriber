//! Drawing limits shared by validation, input, and presentation.
pub const MIN_STROKE_THICKNESS: f64 = 1.0;
pub const MAX_STROKE_THICKNESS: f64 = 50.0;

/// Steps a stroke width by `delta` whole pixels, clamped to the stroke limits.
///
/// A fractional width, such as one drawn before widths snapped or loaded from
/// an older session, lands on the nearest whole pixel in the step's direction
/// first: 30.8 steps up to 31 and down to 30, never skipping a value.
pub fn step_stroke_thickness(thickness: f64, delta: f64) -> f64 {
    let stepped = thickness + delta;
    let whole = if delta > 0.0 {
        stepped.floor()
    } else {
        stepped.ceil()
    };

    whole.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS)
}

#[cfg(test)]
mod tests {
    use super::step_stroke_thickness;

    #[test]
    fn fractional_widths_step_to_the_next_whole_pixel_in_each_direction() {
        assert_eq!(step_stroke_thickness(30.8, 1.0), 31.0);
        assert_eq!(step_stroke_thickness(30.8, -1.0), 30.0);
        assert_eq!(step_stroke_thickness(3.2, 1.0), 4.0);
        assert_eq!(step_stroke_thickness(3.2, -1.0), 3.0);
        assert_eq!(step_stroke_thickness(30.8, 5.0), 35.0);
        assert_eq!(step_stroke_thickness(30.8, -5.0), 26.0);
    }

    #[test]
    fn whole_widths_step_by_the_delta_within_the_limits() {
        assert_eq!(step_stroke_thickness(30.0, 1.0), 31.0);
        assert_eq!(step_stroke_thickness(30.0, -1.0), 29.0);
        assert_eq!(step_stroke_thickness(49.0, 10.0), 50.0);
        assert_eq!(step_stroke_thickness(1.4, -1.0), 1.0);
    }
}
