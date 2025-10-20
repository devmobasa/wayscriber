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
