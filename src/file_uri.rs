use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

pub(crate) fn decode_file_uri(uri: &str) -> Result<PathBuf, String> {
    let raw = uri
        .strip_prefix("file://")
        .ok_or_else(|| format!("Invalid file URI '{uri}'"))?;

    let path_part = if raw.starts_with("localhost/") {
        &raw["localhost".len()..]
    } else if raw.starts_with('/') {
        raw
    } else {
        return Err(format!("Unsupported file URI host in '{uri}'"));
    };

    let decoded = percent_decode(path_part)
        .map_err(|err| format!("Invalid percent-encoding in '{uri}': {err}"))?;

    #[cfg(unix)]
    {
        use std::ffi::OsString;
        Ok(PathBuf::from(OsString::from_vec(decoded)))
    }

    #[cfg(not(unix))]
    {
        let path = String::from_utf8(decoded)
            .map_err(|err| format!("Non-UTF8 path in URI '{uri}': {err}"))?;
        Ok(PathBuf::from(path))
    }
}

fn percent_decode(input: &str) -> Result<Vec<u8>, &'static str> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err("truncated percent escape");
                }
                let hi = hex_value(bytes[i + 1]).ok_or("invalid hex digit")?;
                let lo = hex_value(bytes[i + 2]).ok_or("invalid hex digit")?;
                out.push((hi << 4) | lo);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    Ok(out)
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(10 + b - b'a'),
        b'A'..=b'F' => Some(10 + b - b'A'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_file_uri_rejects_non_file_schemes() {
        let err = decode_file_uri("http://example.com/file.png").expect_err("expected error");
        assert!(err.contains("Invalid file URI"));
    }

    #[test]
    fn decode_file_uri_rejects_unsupported_hosts() {
        let err = decode_file_uri("file://example.com/path.png").expect_err("expected error");
        assert!(err.contains("Unsupported file URI host"));
    }

    #[test]
    fn percent_decode_rejects_truncated_escape() {
        let err = percent_decode("%").expect_err("expected error");
        assert_eq!(err, "truncated percent escape");
    }

    #[test]
    fn percent_decode_rejects_invalid_hex() {
        let err = percent_decode("%ZZ").expect_err("expected error");
        assert_eq!(err, "invalid hex digit");
    }

    #[test]
    #[cfg(unix)]
    fn decode_file_uri_accepts_localhost() {
        let path = decode_file_uri("file://localhost/tmp/test.png")
            .expect("decode_file_uri should accept localhost");
        assert_eq!(path, PathBuf::from("/tmp/test.png"));
    }
}
