/// Normalize before comparing or clamping: neither operation repairs NaN.
/// Callers supply their own finite default; finite inputs retain the owner's policy.
pub(super) fn finite_or_default(value: f64, fallback: f64, field: &str) -> f64 {
    if value.is_finite() {
        value
    } else {
        log::warn!("Non-finite {field}: {value}; resetting to {fallback}");
        fallback
    }
}
