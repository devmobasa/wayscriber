use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalDateTime {
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

/// Local time with UTC fallback if the local offset cannot be determined.
pub fn now_local() -> LocalDateTime {
    system_time_to_unix_seconds(SystemTime::now())
        .and_then(local_or_utc_datetime_from_unix)
        .unwrap_or(UNIX_EPOCH_UTC)
}

/// Format a [`LocalDateTime`] using a small strftime-like subset.
///
/// Supported directives: `%Y`, `%m`, `%d`, `%H`, `%M`, `%S`, and `%%`.
/// Unknown directives are emitted literally (e.g., `%q` becomes `%q`).
pub fn format_with_template(dt: LocalDateTime, template: &str) -> String {
    let mut out = String::with_capacity(template.len() + 8);
    let mut chars = template.chars();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }

        match chars.next() {
            Some('%') => out.push('%'),
            Some('Y') => push_padded_i32(&mut out, dt.year, 4),
            Some('m') => push_padded_u8(&mut out, dt.month),
            Some('d') => push_padded_u8(&mut out, dt.day),
            Some('H') => push_padded_u8(&mut out, dt.hour),
            Some('M') => push_padded_u8(&mut out, dt.minute),
            Some('S') => push_padded_u8(&mut out, dt.second),
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }

    out
}

/// Format a [`SystemTime`] using the strftime-like subset.
pub fn format_system_time(time: SystemTime, template: &str) -> Option<String> {
    let dt = system_time_to_unix_seconds(time).and_then(local_or_utc_datetime_from_unix)?;
    Some(format_with_template(dt, template))
}

/// Format a UNIX timestamp in milliseconds using the strftime-like subset.
pub fn format_unix_millis(ms: u64, template: &str) -> Option<String> {
    const MAX_FORMAT_UNIX_MILLIS: u64 = 253_402_300_799_999; // 9999-12-31T23:59:59Z
    if ms > MAX_FORMAT_UNIX_MILLIS {
        return None;
    }

    let secs = i64::try_from(ms / 1_000).ok()?;
    let dt = local_or_utc_datetime_from_unix(secs)?;
    Some(format_with_template(dt, template))
}

/// RFC3339 string of the current UTC time.
pub fn now_rfc3339() -> String {
    system_time_to_unix_seconds(SystemTime::now())
        .and_then(utc_datetime_from_unix)
        .map(format_rfc3339_utc)
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

fn push_padded_u8(out: &mut String, value: u8) {
    out.push(char::from(b'0' + (value / 10)));
    out.push(char::from(b'0' + (value % 10)));
}

fn push_padded_i32(out: &mut String, value: i32, width: usize) {
    let text = value.to_string();
    for _ in text.len()..width {
        out.push('0');
    }
    out.push_str(&text);
}

fn format_rfc3339_utc(dt: LocalDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second
    )
}

fn system_time_to_unix_seconds(time: SystemTime) -> Option<i64> {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_secs()).ok(),
        Err(err) => {
            let duration = err.duration();
            let secs = i64::try_from(duration.as_secs()).ok()?;
            if duration.subsec_nanos() == 0 {
                secs.checked_neg()
            } else {
                secs.checked_add(1)?.checked_neg()
            }
        }
    }
}

const UNIX_EPOCH_UTC: LocalDateTime = LocalDateTime {
    year: 1970,
    month: 1,
    day: 1,
    hour: 0,
    minute: 0,
    second: 0,
};

fn local_or_utc_datetime_from_unix(secs: i64) -> Option<LocalDateTime> {
    local_or_utc_datetime_from_unix_with(secs, local_datetime_from_unix, utc_datetime_from_unix)
}

fn local_or_utc_datetime_from_unix_with(
    secs: i64,
    local: impl FnOnce(i64) -> Option<LocalDateTime>,
    utc: impl FnOnce(i64) -> Option<LocalDateTime>,
) -> Option<LocalDateTime> {
    local(secs).or_else(|| utc(secs))
}

#[cfg(unix)]
fn local_datetime_from_unix(secs: i64) -> Option<LocalDateTime> {
    libc_datetime_from_unix(secs, true)
}

#[cfg(not(unix))]
fn local_datetime_from_unix(secs: i64) -> Option<LocalDateTime> {
    utc_datetime_from_unix(secs)
}

#[cfg(unix)]
fn utc_datetime_from_unix(secs: i64) -> Option<LocalDateTime> {
    libc_datetime_from_unix(secs, false)
}

#[cfg(not(unix))]
fn utc_datetime_from_unix(secs: i64) -> Option<LocalDateTime> {
    civil_datetime_from_unix(secs)
}

#[cfg(unix)]
fn libc_datetime_from_unix(secs: i64, local: bool) -> Option<LocalDateTime> {
    let raw = unix_seconds_to_time_t(secs)?;
    let mut out = std::mem::MaybeUninit::<libc::tm>::uninit();
    let ptr = unsafe {
        if local {
            libc::localtime_r(&raw, out.as_mut_ptr())
        } else {
            libc::gmtime_r(&raw, out.as_mut_ptr())
        }
    };
    if ptr.is_null() {
        return None;
    }

    let tm = unsafe { out.assume_init() };
    tm_to_datetime(tm)
}

#[cfg(all(unix, target_pointer_width = "64"))]
fn unix_seconds_to_time_t(secs: i64) -> Option<libc::time_t> {
    Some(secs)
}

#[cfg(all(unix, not(target_pointer_width = "64")))]
#[allow(clippy::useless_conversion)]
fn unix_seconds_to_time_t(secs: i64) -> Option<libc::time_t> {
    secs.try_into().ok()
}

#[cfg(unix)]
fn tm_to_datetime(tm: libc::tm) -> Option<LocalDateTime> {
    Some(LocalDateTime {
        year: tm.tm_year.checked_add(1900)?,
        month: u8::try_from(tm.tm_mon.checked_add(1)?).ok()?,
        day: u8::try_from(tm.tm_mday).ok()?,
        hour: u8::try_from(tm.tm_hour).ok()?,
        minute: u8::try_from(tm.tm_min).ok()?,
        second: u8::try_from(tm.tm_sec).ok()?,
    })
}

#[cfg(not(unix))]
fn civil_datetime_from_unix(secs: i64) -> Option<LocalDateTime> {
    const SECS_PER_DAY: i64 = 86_400;
    let days = secs.div_euclid(SECS_PER_DAY);
    let day_secs = secs.rem_euclid(SECS_PER_DAY);
    let (year, month, day) = civil_from_days(days)?;
    Some(LocalDateTime {
        year,
        month,
        day,
        hour: u8::try_from(day_secs / 3_600).ok()?,
        minute: u8::try_from((day_secs % 3_600) / 60).ok()?,
        second: u8::try_from(day_secs % 60).ok()?,
    })
}

#[cfg(not(unix))]
fn civil_from_days(days: i64) -> Option<(i32, u8, u8)> {
    let z = days.checked_add(719_468)?;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    Some((
        i32::try_from(year).ok()?,
        u8::try_from(month).ok()?,
        u8::try_from(day).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_with_template_basic_components() {
        // 1_600_000_000 -> 2020-09-13 12:26:40 UTC
        let dt = utc_datetime_from_unix(1_600_000_000).unwrap();
        let formatted = format_with_template(dt, "%Y-%m-%d %H:%M:%S");
        assert_eq!(formatted, "2020-09-13 12:26:40");
    }

    #[test]
    fn format_with_template_preserves_unknown_directives() {
        let dt = utc_datetime_from_unix(1_600_000_000).unwrap();
        let formatted = format_with_template(dt, "%Y-%q-%m");
        assert_eq!(formatted, "2020-%q-09");
    }

    #[test]
    fn format_with_template_handles_escaped_percent() {
        let dt = utc_datetime_from_unix(1_600_000_000).unwrap();
        let formatted = format_with_template(dt, "%% %Y");
        assert_eq!(formatted, "% 2020");
    }

    #[test]
    fn format_unix_millis_out_of_range_returns_none() {
        let result = format_unix_millis(u64::MAX, "%Y");
        assert!(result.is_none());
    }

    #[test]
    fn local_or_utc_datetime_from_unix_falls_back_to_utc() {
        let fallback = LocalDateTime {
            year: 2026,
            month: 5,
            day: 14,
            hour: 12,
            minute: 30,
            second: 45,
        };

        let result =
            local_or_utc_datetime_from_unix_with(1_777_000_000, |_| None, |_| Some(fallback));

        assert_eq!(result, Some(fallback));
    }
}
