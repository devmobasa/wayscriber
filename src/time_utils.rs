use std::time::SystemTime;

use time::format_description::{self, well_known::Rfc3339};
use time::{OffsetDateTime, UtcOffset};

/// Local time with UTC fallback if the local offset cannot be determined.
pub fn now_local() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

/// Format an [`OffsetDateTime`] using a small strftime-like subset.
///
/// Supported directives: `%Y`, `%m`, `%d`, `%H`, `%M`, `%S`, and `%%`.
/// Unknown directives are emitted literally (e.g., `%q` becomes `%q`).
pub fn format_with_template(dt: OffsetDateTime, template: &str) -> String {
    let desc = convert_strftime_to_time_fmt(template);
    match format_description::parse(&desc) {
        Ok(parsed) => dt.format(&parsed).unwrap_or_else(|_| fallback_rfc3339(dt)),
        Err(_) => fallback_rfc3339(dt),
    }
}

/// Format a [`SystemTime`] using the strftime-like subset.
pub fn format_system_time(time: SystemTime, template: &str) -> Option<String> {
    let dt: OffsetDateTime = time.into();
    Some(format_with_template(to_local(dt), template))
}

/// Format a UNIX timestamp in milliseconds using the strftime-like subset.
pub fn format_unix_millis(ms: u64, template: &str) -> Option<String> {
    let nanos = (ms as i128).saturating_mul(1_000_000);
    let dt = OffsetDateTime::from_unix_timestamp_nanos(nanos).ok()?;
    Some(format_with_template(to_local(dt), template))
}

/// RFC3339 string of the current UTC time.
pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn fallback_rfc3339(dt: OffsetDateTime) -> String {
    dt.format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn to_local(dt: OffsetDateTime) -> OffsetDateTime {
    match UtcOffset::local_offset_at(dt) {
        Ok(offset) => dt.to_offset(offset),
        Err(_) => dt,
    }
}

fn convert_strftime_to_time_fmt(template: &str) -> String {
    let mut out = String::with_capacity(template.len() * 2);
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            match chars.next() {
                Some('%') => out.push('%'),
                Some('Y') => out.push_str("[year repr:full padding:zero]"),
                Some('m') => out.push_str("[month repr:numerical padding:zero]"),
                Some('d') => out.push_str("[day padding:zero]"),
                Some('H') => out.push_str("[hour repr:24 padding:zero]"),
                Some('M') => out.push_str("[minute padding:zero]"),
                Some('S') => out.push_str("[second padding:zero]"),
                Some(other) => {
                    out.push('%');
                    escape_literal(other, &mut out);
                }
                None => out.push('%'),
            }
        } else {
            escape_literal(ch, &mut out);
        }
    }
    out
}

fn escape_literal(ch: char, out: &mut String) {
    match ch {
        '[' => out.push_str("[["),
        ']' => out.push_str("]]"),
        _ => out.push(ch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_with_template_basic_components() {
        // 1_600_000_000 -> 2020-09-13 12:26:40 UTC
        let dt = OffsetDateTime::from_unix_timestamp(1_600_000_000).unwrap();
        let formatted = format_with_template(dt, "%Y-%m-%d %H:%M:%S");
        assert_eq!(formatted, "2020-09-13 12:26:40");
    }

    #[test]
    fn format_with_template_preserves_unknown_directives() {
        let dt = OffsetDateTime::from_unix_timestamp(1_600_000_000).unwrap();
        let formatted = format_with_template(dt, "%Y-%q-%m");
        assert_eq!(formatted, "2020-%q-09");
    }

    #[test]
    fn format_with_template_handles_escaped_percent() {
        let dt = OffsetDateTime::from_unix_timestamp(1_600_000_000).unwrap();
        let formatted = format_with_template(dt, "%% %Y");
        assert_eq!(formatted, "% 2020");
    }

    #[test]
    fn format_unix_millis_out_of_range_returns_none() {
        let result = format_unix_millis(u64::MAX, "%Y");
        assert!(result.is_none());
    }
}
