use crate::models::util::format_float;

pub(in crate::app::view) fn validate_f64_range(value: &str, min: f64, max: f64) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a numeric value".to_string());
    }

    match trimmed.parse::<f64>() {
        Ok(value) => {
            if value < min || value > max {
                Some(format!(
                    "Range: {}-{}",
                    format_float(min),
                    format_float(max)
                ))
            } else {
                None
            }
        }
        Err(_) => Some("Expected a numeric value".to_string()),
    }
}

pub(in crate::app::view) fn validate_u32_range(value: &str, min: u32, max: u32) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a whole number".to_string());
    }

    match trimmed.parse::<u32>() {
        Ok(value) => {
            if value < min || value > max {
                Some(format!("Range: {min}-{max}"))
            } else {
                None
            }
        }
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

pub(in crate::app::view) fn validate_u64_range(value: &str, min: u64, max: u64) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a whole number".to_string());
    }

    match trimmed.parse::<u64>() {
        Ok(value) => {
            if value < min || value > max {
                Some(format!("Range: {min}-{max}"))
            } else {
                None
            }
        }
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

pub(in crate::app::view) fn validate_u64_min(value: &str, min: u64) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a whole number".to_string());
    }

    match trimmed.parse::<u64>() {
        Ok(value) => {
            if value < min {
                Some(format!("Minimum: {min}"))
            } else {
                None
            }
        }
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

pub(in crate::app::view) fn validate_usize_range(
    value: &str,
    min: usize,
    max: usize,
) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a whole number".to_string());
    }

    match trimmed.parse::<usize>() {
        Ok(value) => {
            if value < min || value > max {
                Some(format!("Range: {min}-{max}"))
            } else {
                None
            }
        }
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

pub(in crate::app::view) fn validate_usize_min(value: &str, min: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a whole number".to_string());
    }

    match trimmed.parse::<usize>() {
        Ok(value) => {
            if value < min {
                Some(format!("Minimum: {min}"))
            } else {
                None
            }
        }
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_f64_range_accepts_bounds_and_rejects_outside() {
        assert_eq!(validate_f64_range("0.5", 0.5, 2.0), None);
        assert_eq!(validate_f64_range("2.0", 0.5, 2.0), None);
        assert_eq!(
            validate_f64_range("2.1", 0.5, 2.0),
            Some("Range: 0.5-2".to_string())
        );
        assert_eq!(
            validate_f64_range("", 0.5, 2.0),
            Some("Expected a numeric value".to_string())
        );
    }

    #[test]
    fn validate_u32_range_reports_expected_errors() {
        assert_eq!(validate_u32_range("10", 1, 100), None);
        assert_eq!(
            validate_u32_range("0", 1, 100),
            Some("Range: 1-100".to_string())
        );
        assert_eq!(
            validate_u32_range("abc", 1, 100),
            Some("Expected a whole number".to_string())
        );
    }

    #[test]
    fn validate_u64_helpers_enforce_range_and_minimum() {
        assert_eq!(validate_u64_range("5", 1, 10), None);
        assert_eq!(
            validate_u64_range("11", 1, 10),
            Some("Range: 1-10".to_string())
        );
        assert_eq!(validate_u64_min("10", 10), None);
        assert_eq!(validate_u64_min("9", 10), Some("Minimum: 10".to_string()));
    }

    #[test]
    fn validate_usize_helpers_enforce_range_and_minimum() {
        assert_eq!(validate_usize_range("3", 1, 5), None);
        assert_eq!(
            validate_usize_range("6", 1, 5),
            Some("Range: 1-5".to_string())
        );
        assert_eq!(validate_usize_min("3", 3), None);
        assert_eq!(validate_usize_min("2", 3), Some("Minimum: 3".to_string()));
    }
}
