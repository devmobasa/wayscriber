use super::command::ClipboardCommandRunner;
use super::*;
use crate::process_broker::BrokerOutput;
use std::io::{self, Cursor, Read};

struct LargeClipboardRunner {
    payload: Vec<u8>,
}

impl ClipboardCommandRunner for LargeClipboardRunner {
    fn list_types(&self) -> anyhow::Result<BrokerOutput> {
        Ok(BrokerOutput {
            status: 0,
            stdout: b"image/png\n".to_vec(),
            stderr: Vec::new(),
            timed_out: false,
            stdout_limit_reached: false,
        })
    }

    fn paste_mime(
        &self,
        _mime_type: &str,
        _timeout: Duration,
        output_cap: usize,
    ) -> anyhow::Result<BrokerOutput> {
        let stdout_limit_reached = self.payload.len() >= output_cap;
        Ok(BrokerOutput {
            status: if stdout_limit_reached { 137 } else { 0 },
            stdout: self.payload.iter().copied().take(output_cap).collect(),
            stderr: Vec::new(),
            timed_out: false,
            stdout_limit_reached,
        })
    }

    fn copy_selection(&self, _payload: &[u8], _timeout: Duration) -> anyhow::Result<BrokerOutput> {
        unreachable!("clipboard read tests never publish")
    }
}

struct ExactLimitThenError {
    bytes: Vec<u8>,
    read_once: bool,
}

impl Read for ExactLimitThenError {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.read_once {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "unexpected second read",
            ));
        }
        self.read_once = true;
        let len = self.bytes.len().min(buffer.len());
        buffer[..len].copy_from_slice(&self.bytes[..len]);
        Ok(len)
    }
}

#[test]
fn strict_read_rejects_data_over_limit() {
    let err = read_limited(Cursor::new(vec![1, 2, 3, 4, 5]), 4).expect_err("over limit");

    assert!(matches!(err, ClipboardReadError::TooLarge { limit: 4 }));
}

#[test]
fn prefix_read_returns_bounded_sample_for_data_over_limit() {
    let sample = read_prefix(Cursor::new(vec![1, 2, 3, 4, 5]), 4).expect("prefix sample");

    assert_eq!(sample.bytes, vec![1, 2, 3, 4]);
    assert!(sample.truncated);
}

#[test]
fn prefix_read_returns_when_first_read_reaches_limit() {
    let reader = ExactLimitThenError {
        bytes: vec![1, 2, 3, 4],
        read_once: false,
    };
    let sample = read_prefix(reader, 4).expect("prefix sample");

    assert_eq!(sample.bytes, vec![1, 2, 3, 4]);
    assert!(sample.truncated);
}

#[test]
fn prefix_read_marks_small_data_untruncated() {
    let sample = read_prefix(Cursor::new(vec![1, 2, 3]), 4).expect("prefix sample");

    assert_eq!(sample.bytes, vec![1, 2, 3]);
    assert!(!sample.truncated);
}

#[test]
fn broker_prefix_result_returns_a_truncated_clipboard_sample() {
    let runner = LargeClipboardRunner {
        payload: b"123456789".to_vec(),
    };

    let sample =
        read_clipboard_mime_prefix_with_runner("image/png", 4, Duration::from_secs(1), &runner)
            .expect("bounded prefix");

    assert_eq!(sample.bytes, b"1234");
    assert!(sample.truncated);
}

#[test]
fn broker_prefix_result_maps_an_oversized_full_read_to_too_large() {
    let runner = LargeClipboardRunner {
        payload: b"123456789".to_vec(),
    };

    let error = read_clipboard_mime_with_runner("image/png", 4, Duration::from_secs(1), &runner)
        .expect_err("oversized clipboard");

    assert_eq!(error, ClipboardReadError::TooLarge { limit: 4 });
}

#[test]
fn large_clipboard_fingerprints_include_the_bounded_content_hash() {
    let first = LargeClipboardRunner {
        payload: vec![b'a'; CLIPBOARD_FINGERPRINT_BYTES * 2],
    };
    let mut changed_payload = vec![b'a'; CLIPBOARD_FINGERPRINT_BYTES * 2];
    changed_payload[0] = b'b';
    let second = LargeClipboardRunner {
        payload: changed_payload,
    };

    let first = clipboard_fingerprint_with_runner(&first).expect("first fingerprint");
    let second = clipboard_fingerprint_with_runner(&second).expect("second fingerprint");

    assert_eq!(first.bounded_content_len, Some(CLIPBOARD_FINGERPRINT_BYTES));
    assert!(first.bounded_content_truncated);
    assert!(first.bounded_content_hash.is_some());
    assert_ne!(first.bounded_content_hash, second.bounded_content_hash);
}
