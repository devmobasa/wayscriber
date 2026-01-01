use anyhow::{Context, Result};
use flate2::{Compression, bufread::GzDecoder, write::GzEncoder};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub(super) fn compress_bytes(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .context("failed to compress session payload")?;
    encoder
        .finish()
        .context("failed to finalise compressed session payload")
}

pub(super) fn maybe_decompress(bytes: Vec<u8>) -> Result<(Vec<u8>, bool)> {
    if !is_gzip(&bytes) {
        return Ok((bytes, false));
    }

    let mut decoder = GzDecoder::new(&bytes[..]);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .context("failed to decompress session file")?;
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
