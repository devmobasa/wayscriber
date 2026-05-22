use std::{error::Error, fmt};

const STANDARD_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DecodeError {
    InvalidByte { index: usize, byte: u8 },
    InvalidLength,
    InvalidPadding,
    NonCanonicalTrailingBits,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidByte { index, byte } => {
                write!(f, "invalid base64 byte 0x{byte:02x} at offset {index}")
            }
            Self::InvalidLength => write!(f, "invalid base64 length"),
            Self::InvalidPadding => write!(f, "invalid base64 padding"),
            Self::NonCanonicalTrailingBits => write!(f, "non-canonical base64 trailing bits"),
        }
    }
}

impl Error for DecodeError {}

pub(crate) fn encode_standard(bytes: &[u8]) -> String {
    let encoded_len = bytes.len().div_ceil(3) * 4;
    let mut encoded = String::with_capacity(encoded_len);

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        encoded.push(STANDARD_ALPHABET[(b0 >> 2) as usize] as char);
        encoded.push(STANDARD_ALPHABET[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);

        if chunk.len() >= 2 {
            encoded
                .push(STANDARD_ALPHABET[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }

        if chunk.len() == 3 {
            encoded.push(STANDARD_ALPHABET[(b2 & 0b0011_1111) as usize] as char);
        } else {
            encoded.push('=');
        }
    }

    encoded
}

pub(crate) fn decode_standard(encoded: &str) -> Result<Vec<u8>, DecodeError> {
    let mut values = Vec::with_capacity(encoded.len());
    let mut padding = 0usize;
    let mut saw_padding = false;

    for (index, byte) in encoded.bytes().enumerate() {
        if byte == b'=' {
            saw_padding = true;
            padding += 1;
            if padding > 2 {
                return Err(DecodeError::InvalidPadding);
            }
            continue;
        }

        if saw_padding {
            return Err(DecodeError::InvalidPadding);
        }

        let value = decode_value(byte).ok_or(DecodeError::InvalidByte { index, byte })?;
        values.push(value);
    }

    let remainder = values.len() % 4;
    if remainder == 1 {
        return Err(DecodeError::InvalidLength);
    }

    if !(values.len() + padding).is_multiple_of(4) {
        return Err(DecodeError::InvalidPadding);
    }

    if padding > 0 {
        match (padding, remainder) {
            (1, 3) | (2, 2) => {}
            _ => return Err(DecodeError::InvalidPadding),
        }
    }

    let mut decoded = Vec::with_capacity(values.len() / 4 * 3 + 2);
    let full_groups_len = values.len() / 4 * 4;
    for chunk in values[..full_groups_len].chunks_exact(4) {
        decoded.push((chunk[0] << 2) | (chunk[1] >> 4));
        decoded.push((chunk[1] << 4) | (chunk[2] >> 2));
        decoded.push((chunk[2] << 6) | chunk[3]);
    }

    match &values[full_groups_len..] {
        [] => {}
        [a, b] => {
            if b & 0b0000_1111 != 0 {
                return Err(DecodeError::NonCanonicalTrailingBits);
            }
            decoded.push((a << 2) | (b >> 4));
        }
        [a, b, c] => {
            if c & 0b0000_0011 != 0 {
                return Err(DecodeError::NonCanonicalTrailingBits);
            }
            decoded.push((a << 2) | (b >> 4));
            decoded.push((b << 4) | (c >> 2));
        }
        _ => unreachable!("base64 remainder length was checked"),
    }

    Ok(decoded)
}

fn decode_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{DecodeError, decode_standard, encode_standard};

    #[test]
    fn encodes_standard_base64_with_canonical_padding() {
        let vectors = [
            (b"".as_slice(), ""),
            (b"f".as_slice(), "Zg=="),
            (b"fo".as_slice(), "Zm8="),
            (b"foo".as_slice(), "Zm9v"),
            (b"foob".as_slice(), "Zm9vYg=="),
            (b"fooba".as_slice(), "Zm9vYmE="),
            (b"foobar".as_slice(), "Zm9vYmFy"),
            (b"hello world".as_slice(), "aGVsbG8gd29ybGQ="),
        ];

        for (decoded, encoded) in vectors {
            assert_eq!(encode_standard(decoded), encoded);
        }
    }

    #[test]
    fn decodes_canonically_padded_standard_base64() {
        let vectors = [
            ("", b"".as_slice()),
            ("Zg==", b"f".as_slice()),
            ("Zm8=", b"fo".as_slice()),
            ("Zm9v", b"foo".as_slice()),
            ("Zm9vYg==", b"foob".as_slice()),
            ("Zm9vYmE=", b"fooba".as_slice()),
            ("Zm9vYmFy", b"foobar".as_slice()),
            ("aGVsbG8gd29ybGQ=", b"hello world".as_slice()),
        ];

        for (encoded, decoded) in vectors {
            assert_eq!(decode_standard(encoded).unwrap(), decoded);
        }
    }

    #[test]
    fn rejects_invalid_base64() {
        assert_eq!(
            decode_standard("A").unwrap_err(),
            DecodeError::InvalidLength
        );
        assert_eq!(
            decode_standard("Zg").unwrap_err(),
            DecodeError::InvalidPadding
        );
        assert_eq!(
            decode_standard("Zm8").unwrap_err(),
            DecodeError::InvalidPadding
        );
        assert_eq!(
            decode_standard("A===").unwrap_err(),
            DecodeError::InvalidPadding
        );
        assert_eq!(
            decode_standard("AA=A").unwrap_err(),
            DecodeError::InvalidPadding
        );
        assert_eq!(
            decode_standard("Zg=A").unwrap_err(),
            DecodeError::InvalidPadding
        );
        assert_eq!(
            decode_standard("AB==").unwrap_err(),
            DecodeError::NonCanonicalTrailingBits
        );
    }
}
