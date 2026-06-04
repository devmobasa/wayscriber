pub(super) fn base64_encoded_len(bytes: u64) -> u64 {
    bytes.div_ceil(3).saturating_mul(4)
}

pub(super) fn escaped_json_string_len(value: &str) -> u64 {
    // Include quotes and conservatively account for JSON escaping.
    value
        .chars()
        .fold(2u64, |len, ch| len.saturating_add(json_char_len(ch)))
}

fn json_char_len(ch: char) -> u64 {
    match ch {
        '"' | '\\' => 2,
        '\u{08}' | '\u{0C}' | '\n' | '\r' | '\t' => 2,
        ch if ch <= '\u{1F}' => 6,
        ch => ch.len_utf8() as u64,
    }
}

pub(super) fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

pub(super) fn format_session_limit(bytes: u64) -> String {
    let mib = bytes as f64 / 1024.0 / 1024.0;
    if mib >= 10.0 {
        format!("{mib:.0} MiB")
    } else {
        format!("{mib:.1} MiB")
    }
}
