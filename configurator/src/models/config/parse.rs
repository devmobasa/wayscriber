use super::super::error::FormError;
use super::super::util::parse_f64;

pub(super) fn parse_field<F>(
    value: &str,
    field: &'static str,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(f64),
{
    parse_field_in_range(
        value,
        field,
        f64::NEG_INFINITY,
        f64::INFINITY,
        errors,
        apply,
    );
}

pub(super) fn parse_field_in_range<F>(
    value: &str,
    field: &'static str,
    min: f64,
    max: f64,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(f64),
{
    match parse_f64(value.trim()) {
        Ok(parsed) if !parsed.is_finite() => {
            errors.push(FormError::new(field, "Expected a finite numeric value"));
        }
        Ok(parsed) if parsed < min || parsed > max => {
            errors.push(FormError::new(field, format!("Expected {min}-{max}")));
        }
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err)),
    }
}

pub(super) fn parse_usize_field<F>(
    value: &str,
    field: &'static str,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(usize),
{
    parse_usize_in_range(value, field, 0, usize::MAX, errors, apply);
}

pub(super) fn parse_usize_in_range<F>(
    value: &str,
    field: &'static str,
    min: usize,
    max: usize,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(usize),
{
    match value.trim().parse::<usize>() {
        Ok(parsed) if parsed < min || parsed > max => {
            errors.push(FormError::new(field, format!("Expected {min}-{max}")));
        }
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}

pub(super) fn parse_usize_at_least<F>(
    value: &str,
    field: &'static str,
    min: usize,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(usize),
{
    match value.trim().parse::<usize>() {
        Ok(parsed) if parsed < min => {
            errors.push(FormError::new(field, format!("Expected at least {min}")));
        }
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}

pub(super) fn parse_u8_in_range<F>(
    value: &str,
    field: &'static str,
    min: u8,
    max: u8,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(u8),
{
    match value.trim().parse::<u8>() {
        Ok(parsed) if parsed < min || parsed > max => {
            errors.push(FormError::new(field, format!("Expected {min}-{max}")));
        }
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}

pub(super) fn parse_optional_usize_field<F>(
    value: &str,
    field: &'static str,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(Option<usize>),
{
    let trimmed = value.trim();
    if trimmed.is_empty() {
        apply(None);
        return;
    }
    match trimmed.parse::<usize>() {
        Ok(parsed) => apply(Some(parsed)),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}

pub(super) fn parse_required_f64<F>(
    value: &str,
    field: F,
    errors: &mut Vec<FormError>,
) -> Option<f64>
where
    F: FnOnce() -> String,
{
    let trimmed = value.trim();
    if trimmed.is_empty() {
        errors.push(FormError::new(field(), "Value is required"));
        return None;
    }
    match parse_f64(trimmed) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            errors.push(FormError::new(field(), err));
            None
        }
    }
}

pub(super) fn parse_optional_f64<F>(
    value: &str,
    field: F,
    errors: &mut Vec<FormError>,
) -> Option<f64>
where
    F: FnOnce() -> String,
{
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    match parse_f64(trimmed) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            errors.push(FormError::new(field(), err));
            None
        }
    }
}

pub(super) fn parse_u64_field<F>(
    value: &str,
    field: &'static str,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(u64),
{
    parse_u64_in_range(value, field, 0, u64::MAX, errors, apply);
}

pub(super) fn parse_u64_in_range<F>(
    value: &str,
    field: &'static str,
    min: u64,
    max: u64,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(u64),
{
    match value.trim().parse::<u64>() {
        Ok(parsed) if parsed < min || parsed > max => {
            errors.push(FormError::new(field, format!("Expected {min}-{max}")));
        }
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}
