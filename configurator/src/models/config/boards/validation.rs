use wayscriber::config::BoardBackgroundConfig;

use crate::models::color::ColorTripletInput;
use crate::models::error::FormError;

pub(super) fn default_pen_fallback(background: &BoardBackgroundConfig) -> [f64; 3] {
    match background {
        BoardBackgroundConfig::Transparent(_) => [0.0, 0.0, 0.0],
        BoardBackgroundConfig::Color(color) => {
            let rgb = color.rgb();
            let avg = (rgb[0] + rgb[1] + rgb[2]) / 3.0;
            if avg >= 0.5 {
                [0.0, 0.0, 0.0]
            } else {
                [1.0, 1.0, 1.0]
            }
        }
    }
}

pub(super) fn parse_triplet(
    input: &ColorTripletInput,
    field_prefix: &str,
    errors: &mut Vec<FormError>,
) -> Option<[f64; 3]> {
    match input.to_array(field_prefix) {
        Ok(values) => Some(values),
        Err(err) => {
            errors.push(err);
            None
        }
    }
}

pub(super) fn parse_usize<F>(
    value: &str,
    field: &'static str,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(usize),
{
    match value.trim().parse::<usize>() {
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}
