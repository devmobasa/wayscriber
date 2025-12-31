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
