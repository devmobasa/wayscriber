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
    match parse_f64(value.trim()) {
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
    match value.trim().parse::<usize>() {
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
    match value.trim().parse::<u64>() {
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}

pub(super) fn parse_u32_field<F>(
    value: &str,
    field: &'static str,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(u32),
{
    match value.trim().parse::<u32>() {
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}
