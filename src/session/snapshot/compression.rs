use anyhow::{Context, Result};
use flate2::{Compression, bufread::GzDecoder, write::GzEncoder};
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub(super) const DEFAULT_MAX_EXPANDED_SESSION_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug)]
pub(super) struct ExpandedSessionTooLarge {
    pub expanded_size: u64,
    pub max_expanded_size: u64,
}

impl fmt::Display for ExpandedSessionTooLarge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "session expands to at least {} bytes which exceeds the safety limit of {} bytes",
            self.expanded_size, self.max_expanded_size
        )
    }
}

impl std::error::Error for ExpandedSessionTooLarge {}

pub(super) fn compress_bytes(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .context("failed to compress session payload")?;
    encoder
        .finish()
        .context("failed to finalise compressed session payload")
}

pub(super) fn maybe_decompress_with_limit(
    bytes: Vec<u8>,
    max_expanded_size: u64,
) -> Result<(Vec<u8>, bool)> {
    if !is_gzip(&bytes) {
        return Ok((bytes, false));
    }

    let mut decoder = GzDecoder::new(&bytes[..]);
    let mut out = Vec::new();
    decoder
        .by_ref()
        .take(max_expanded_size.saturating_add(1))
        .read_to_end(&mut out)
        .context("failed to decompress session file")?;
    if out.len() as u64 > max_expanded_size {
        return Err(ExpandedSessionTooLarge {
            expanded_size: out.len() as u64,
            max_expanded_size,
        }
        .into());
    }
    Ok((out, true))
}

pub(super) fn is_gzip(bytes: &[u8]) -> bool {
    bytes.len() > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

pub(super) fn temp_path(target: &Path) -> Result<PathBuf> {
    let mut candidate = target.with_extension("json.tmp");
    let mut counter = 0u32;
    while candidate.exists() {
        counter += 1;
        candidate = target.with_extension(format!("json.tmp{}", counter));
    }
    Ok(candidate)
}
