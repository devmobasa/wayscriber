use super::Generation;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Read};

/// Marker record format. Change a field's meaning only with a new number.
pub(in crate::session::snapshot) const MARKER_FORMAT: u32 = 1;
/// Largest marker this build reads. A record is about 80 bytes.
const MAX_MARKER_BYTES: u64 = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(in crate::session::snapshot) enum MarkerKind {
    Cleared,
    BackupRecoverable,
    RecoveryRecoverable,
}

#[derive(Debug, Serialize, Deserialize)]
struct MarkerRecord {
    format: u32,
    /// Must match the path the record was read from.
    kind: MarkerKind,
    generation: u64,
    /// RFC 3339 wall-clock time, for people only. Never used for ordering.
    #[serde(default)]
    written: String,
}

/// One line of JSON: {"format":1,"kind":"cleared","generation":42,"written":"..."}\n
pub(in crate::session::snapshot) fn marker_record_bytes(
    kind: MarkerKind,
    generation: u64,
    written: String,
) -> Result<Vec<u8>> {
    let record = MarkerRecord {
        format: MARKER_FORMAT,
        kind,
        generation,
        written,
    };
    let mut bytes = serde_json::to_vec(&record).context("failed to serialise session marker")?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Legacy timestamp content, another format, a kind mismatch, a parse error,
/// or more than 1 KiB all read as `Unknown`. Unknown extra fields are ignored,
/// so format 1 can grow. Only the read can fail.
pub(in crate::session::snapshot) fn read_marker_generation(
    file: &File,
    expected: MarkerKind,
) -> io::Result<Generation> {
    let mut bytes = Vec::new();
    file.take(MAX_MARKER_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MARKER_BYTES {
        return Ok(Generation::Unknown);
    }

    let Ok(record) = serde_json::from_slice::<MarkerRecord>(&bytes) else {
        return Ok(Generation::Unknown);
    };
    if record.format != MARKER_FORMAT || record.kind != expected {
        return Ok(Generation::Unknown);
    }
    Ok(Generation::from_raw(record.generation))
}
