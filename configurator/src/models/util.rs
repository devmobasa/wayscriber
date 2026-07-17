pub fn parse_f64(input: &str) -> Result<f64, String> {
    input
        .parse::<f64>()
        .map_err(|_| "Expected a numeric value".to_string())
}

pub fn format_float(value: f64) -> String {
    value.to_string()
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
    fn format_float_uses_shortest_lossless_representation() {
        assert_eq!(format_float(12.0), "12");
        assert_eq!(format_float(12.340), "12.34");
        assert_eq!(format_float(12.300), "12.3");
        assert_eq!(format_float(12.3456), "12.3456");

        let precise = 1.234_567_890_123_45;
        assert_eq!(parse_f64(&format_float(precise)).unwrap(), precise);
    }
}
