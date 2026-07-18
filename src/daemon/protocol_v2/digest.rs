use anyhow::{Context, Result, bail};
use glib::{Checksum, ChecksumType};

const SHA256_DIGEST_BYTES: usize = 32;
const LOWER_HEX: &[u8; 16] = b"0123456789abcdef";

/// Computes the canonical lowercase SHA-256 representation used by protocol v2.
///
/// GLib is already part of Wayscriber's Cairo/Pango runtime dependency graph.
/// Keeping the adapter private prevents GLib types and formatting choices from
/// becoming part of the serialized protocol contract.
pub(super) fn sha256_hex(bytes: &[u8]) -> Result<String> {
    let mut checksum = Checksum::new(ChecksumType::Sha256)
        .context("GLib SHA-256 checksum implementation is unavailable")?;
    checksum.update(bytes);
    let digest = checksum.digest();
    if digest.len() != SHA256_DIGEST_BYTES {
        bail!(
            "GLib SHA-256 returned {} bytes instead of {SHA256_DIGEST_BYTES}",
            digest.len()
        );
    }

    let mut encoded = String::with_capacity(SHA256_DIGEST_BYTES * 2);
    for byte in digest {
        encoded.push(LOWER_HEX[usize::from(byte >> 4)] as char);
        encoded.push(LOWER_HEX[usize::from(byte & 0x0f)] as char);
    }
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_hex(value: &str) -> Vec<u8> {
        assert!(value.len().is_multiple_of(2));
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let pair = std::str::from_utf8(pair).unwrap();
                u8::from_str_radix(pair, 16).unwrap()
            })
            .collect()
    }

    #[test]
    fn matches_nist_sha256_byte_oriented_short_message_vectors() {
        // NIST CAVP 11.0 SHA256ShortMsg.rsp, including the SHA-256
        // 55/56-byte padding boundary and complete 63/64-byte blocks.
        const VECTORS: [(&str, &str); 5] = [
            (
                "",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                "3ebfb06db8c38d5ba037f1363e118550aad94606e26835a01af05078533cc25f2f39573c04b632f62f68c294ab31f2a3e2a1a0d8c2be51",
                "6595a2ef537a69ba8583dfbf7f5bec0ab1f93ce4c8ee1916eff44a93af5749c4",
            ),
            (
                "2d52447d1244d2ebc28650e7b05654bad35b3a68eedc7f8515306b496d75f3e73385dd1b002625024b81a02f2fd6dffb6e6d561cb7d0bd7a",
                "cfb88d6faf2de3a69d36195acec2e255e2af2b7d933997f348e09f6ce5758360",
            ),
            (
                "e2f76e97606a872e317439f1a03fcd92e632e5bd4e7cbc4e97f1afc19a16fde92d77cbe546416b51640cddb92af996534dfd81edb17c4424cf1ac4d75aceeb",
                "18041bd4665083001fba8c5411d2d748e8abbfdcdfd9218cb02b68a78e7d4c23",
            ),
            (
                "5a86b737eaea8ee976a0a24da63e7ed7eefad18a101c1211e2b3650c5187c2a8a650547208251f6d4237e661c7bf4c77f335390394c37fa1a9f9be836ac28509",
                "42e61e174fbb3897d6dd6cef3dd2802fe67b331953b06114a65c772859dfc1aa",
            ),
        ];

        for (message, expected) in VECTORS {
            assert_eq!(sha256_hex(&decode_hex(message)).unwrap(), expected);
        }
    }

    #[test]
    fn matches_nist_million_a_long_message_vector() {
        let message = vec![b'a'; 1_000_000];
        assert_eq!(
            sha256_hex(&message).unwrap(),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
}
