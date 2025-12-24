pub fn parse_f64(input: &str) -> Result<f64, String> {
    input
        .parse::<f64>()
        .map_err(|_| "Expected a numeric value".to_string())
}

pub fn format_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{:.0}", value)
    } else {
        format!("{:.3}", value)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_f64_rejects_non_numeric() {
        let err = parse_f64("not-a-number").expect_err("expected parse error");
        assert_eq!(err, "Expected a numeric value");
    }

    #[test]
    fn format_float_trims_trailing_zeroes() {
        assert_eq!(format_float(12.0), "12");
        assert_eq!(format_float(12.340), "12.34");
        assert_eq!(format_float(12.300), "12.3");
        assert_eq!(format_float(12.3456), "12.346");
    }
}
