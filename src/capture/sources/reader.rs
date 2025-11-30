#![cfg(feature = "portal")]
use std::path::PathBuf;
use std::{fs, thread, time::Duration};

use crate::capture::types::CaptureError;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

/// Read image data from a file:// URI.
///
/// This properly decodes percent-encoded URIs (spaces, non-ASCII characters, etc.)
/// and cleans up the temporary file after reading.
pub fn read_image_from_uri(uri: &str) -> Result<Vec<u8>, CaptureError> {
    let path = decode_file_uri(uri)?;

    log::debug!("Reading screenshot from: {}", path.display());

    // Wait briefly for portal to flush the file to disk (some portals write asynchronously)
    const MAX_ATTEMPTS: usize = 60; // up to 3 seconds total
    const ATTEMPT_DELAY_MS: u64 = 50;

    let mut data = Vec::new();
    for attempt in 0..MAX_ATTEMPTS {
        match fs::read(&path) {
            Ok(bytes) if !bytes.is_empty() => {
                data = bytes;
                break;
            }
            Ok(_) => {
                log::trace!(
                    "Portal screenshot file {} still empty (attempt {}/{})",
                    path.display(),
                    attempt + 1,
                    MAX_ATTEMPTS
                );
            }
            Err(e) => {
                log::trace!(
                    "Portal screenshot file {} not ready yet (attempt {}/{}): {}",
                    path.display(),
                    attempt + 1,
                    MAX_ATTEMPTS,
                    e
                );
            }
        }

        if attempt + 1 == MAX_ATTEMPTS {
            return Err(CaptureError::ImageError(format!(
                "Portal screenshot file {} not ready after {} attempts",
                path.display(),
                MAX_ATTEMPTS
            )));
        }

        thread::sleep(Duration::from_millis(ATTEMPT_DELAY_MS));
    }

    log::info!(
        "Successfully read {} bytes from portal screenshot",
        data.len()
    );

    // Clean up portal temp file to prevent accumulation
    if let Err(e) = fs::remove_file(&path) {
        log::warn!(
            "Failed to remove portal temp file {}: {}",
            path.display(),
            e
        );
    } else {
        log::debug!("Removed portal temp file: {}", path.display());
    }

    Ok(data)
}

fn decode_file_uri(uri: &str) -> Result<PathBuf, CaptureError> {
    let raw = uri
        .strip_prefix("file://")
        .ok_or_else(|| CaptureError::InvalidResponse(format!("Invalid file URI '{}'", uri)))?;

    // Allow optional host (empty or localhost)
    let path_part = if raw.starts_with("localhost/") {
        &raw["localhost".len()..]
    } else if raw.starts_with('/') {
        raw
    } else {
        return Err(CaptureError::InvalidResponse(format!(
            "Unsupported file URI host in '{}'",
            uri
        )));
    };

    let decoded = percent_decode(path_part).map_err(|e| {
        CaptureError::InvalidResponse(format!("Invalid percent-encoding in '{}': {}", uri, e))
    })?;

    #[cfg(unix)]
    {
        use std::ffi::OsString;
        Ok(PathBuf::from(OsString::from_vec(decoded)))
    }

    #[cfg(not(unix))]
    {
        let path_str = String::from_utf8(decoded).map_err(|e| {
            CaptureError::InvalidResponse(format!("Non-UTF8 path in URI '{}': {}", uri, e))
        })?;
        Ok(PathBuf::from(path_str))
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
    use tempfile::TempDir;

    #[test]
    fn reads_and_removes_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("capture file.png");
        std::fs::write(&file_path, b"portal-bytes").unwrap();
        let uri = format!(
            "file://{}",
            file_path
                .to_string_lossy()
                .replace('%', "%25")
                .replace(' ', "%20")
        );

        let data = read_image_from_uri(&uri).expect("read succeeds");
        assert_eq!(data, b"portal-bytes");
        assert!(
            !file_path.exists(),
            "read_image_from_uri should delete the portal temp file"
        );
    }
}
