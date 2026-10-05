use super::Generation;
use crate::session::snapshot::compression::is_gzip;
use flate2::bufread::GzDecoder;
use std::fs::File;
use std::io::{self, Read};
use std::os::unix::fs::FileExt;

/// Bytes read from the start of a payload, whatever its size.
pub(super) const PROBE_READ_BYTES: usize = 16 * 1024;
/// Decoded bytes scanned for the header. The header is about 60 bytes.
pub(super) const PROBE_SCAN_BYTES: usize = 4 * 1024;

const MAX_HEADER_MEMBERS: usize = 4;
const MAX_KEY_BYTES: usize = 64;
const MAX_SKIPPED_STRING_BYTES: usize = 256;
const MAX_INTEGER_DIGITS: usize = 16;

/// Reads the generation of a payload artifact. Only the read can fail; a
/// payload without a readable header returns `Unknown`. Uses `pread`, so the
/// file position stays at 0 for a later full load.
pub(super) fn probe_payload_header(file: &File) -> io::Result<Generation> {
    let window = read_probe_window(file)?;
    Ok(payload_prefix_generation(&window))
}

pub(super) fn read_probe_window(file: &File) -> io::Result<Vec<u8>> {
    let mut window = vec![0; PROBE_READ_BYTES];
    let mut filled = 0;
    while filled < window.len() {
        match file.read_at(&mut window[filled..], filled as u64) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        }
    }
    window.truncate(filled);
    Ok(window)
}

/// Generation named by the first bytes of a plain or gzip payload.
pub(in crate::session::snapshot) fn payload_prefix_generation(bytes: &[u8]) -> Generation {
    let window = &bytes[..bytes.len().min(PROBE_READ_BYTES)];
    let decoded;
    let json = if is_gzip(window) {
        decoded = inflate_probe_prefix(window);
        &decoded[..]
    } else {
        &window[..window.len().min(PROBE_SCAN_BYTES)]
    };
    scan_header(json).map_or(Generation::Unknown, Generation::from_raw)
}

pub(super) fn inflate_probe_prefix(window: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(PROBE_SCAN_BYTES);
    // A stream cut at the window end ends in an error. read_to_end keeps the
    // bytes decoded before it, and those are all the header scan needs.
    let _ = GzDecoder::new(window)
        .take(PROBE_SCAN_BYTES as u64)
        .read_to_end(&mut decoded);
    decoded
}

/// A deliberately small JSON header grammar. Never scans drawing data for a key.
fn scan_header(bytes: &[u8]) -> Option<u64> {
    let mut cursor = HeaderCursor { bytes, offset: 0 };
    cursor.expect(b'{')?;
    let mut found = None;
    for _ in 0..MAX_HEADER_MEMBERS {
        cursor.space();
        if cursor.peek() == Some(b'}') {
            return found;
        }
        let key = match cursor.string(MAX_KEY_BYTES, true) {
            Some(key) => key,
            None => return found,
        };
        if cursor.expect(b':').is_none() {
            return found;
        }
        if key == b"save_generation" {
            if found.is_some() {
                return None;
            }
            found = Some(cursor.integer()?);
        } else if key == b"version" {
            if cursor.integer().is_none() {
                return found;
            }
        } else if cursor.string(MAX_SKIPPED_STRING_BYTES, false).is_none() {
            return found;
        }
        cursor.space();
        match cursor.peek() {
            Some(b',') => cursor.offset += 1,
            Some(b'}') => return found,
            _ => return found,
        }
    }
    found
}

struct HeaderCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> HeaderCursor<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }

    fn space(&mut self) {
        while self
            .peek()
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.offset += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Option<()> {
        self.space();
        if self.peek()? != byte {
            return None;
        }
        self.offset += 1;
        Some(())
    }

    fn string(&mut self, limit: usize, key: bool) -> Option<&'a [u8]> {
        self.expect(b'"')?;
        let start = self.offset;
        while let Some(byte) = self.peek() {
            if byte == b'"' {
                let value = &self.bytes[start..self.offset];
                self.offset += 1;
                return (!key || !value.is_empty()).then_some(value);
            }
            if self.offset - start >= limit
                || byte == b'\\'
                || byte < 0x20
                || (key && !byte.is_ascii_alphanumeric() && byte != b'_')
            {
                return None;
            }
            self.offset += 1;
        }
        None
    }

    fn integer(&mut self) -> Option<u64> {
        self.space();
        let start = self.offset;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.offset += 1;
        }
        let digits = &self.bytes[start..self.offset];
        if digits.is_empty()
            || digits.len() > MAX_INTEGER_DIGITS
            || (digits.len() > 1 && digits[0] == b'0')
            || !self
                .peek()
                .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b',' | b'}'))
        {
            return None;
        }
        std::str::from_utf8(digits).ok()?.parse().ok()
    }
}
