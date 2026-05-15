use super::*;
use std::io::{self, Cursor, Read};

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
