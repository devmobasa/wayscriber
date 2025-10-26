use std::{fs, thread, time::Duration};

use crate::capture::types::CaptureError;

/// Read image data from a file:// URI.
///
/// This properly decodes percent-encoded URIs (spaces, non-ASCII characters, etc.)
/// and cleans up the temporary file after reading.
pub fn read_image_from_uri(uri: &str) -> Result<Vec<u8>, CaptureError> {
    // Parse URL to handle percent-encoding (spaces → %20, unicode, etc.)
    let url = url::Url::parse(uri)
        .map_err(|e| CaptureError::InvalidResponse(format!("Invalid file URI '{}': {}", uri, e)))?;

    // Convert to file path (handles percent-decoding automatically)
    let path = url.to_file_path().map_err(|_| {
        CaptureError::InvalidResponse(format!("Cannot convert URI to path: {}", uri))
    })?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn reads_and_removes_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("capture file.png");
        std::fs::write(&file_path, b"portal-bytes").unwrap();
        let uri = url::Url::from_file_path(&file_path).unwrap().to_string();

        let data = read_image_from_uri(&uri).expect("read succeeds");
        assert_eq!(data, b"portal-bytes");
        assert!(
            !file_path.exists(),
            "read_image_from_uri should delete the portal temp file"
        );
    }
}
