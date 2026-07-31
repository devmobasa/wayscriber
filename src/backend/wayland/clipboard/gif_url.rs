//! Rescuing animated GIFs from browser "Copy image".
//!
//! Browsers rasterize a copied GIF into a static `image/png` snapshot — the
//! GIF bytes are never offered. They do, however, put the image's source URL
//! on the clipboard. When the negotiated mime is a static raster and a source
//! URL ends in `.gif`, the paste worker downloads it (HTTPS only, capped at
//! the GIF paste budget, through the same broker-run `curl`/`wget` harness as
//! the update check) and validates it through the normal decode choke point.
//! Every failure is quiet: the caller falls back to the offered snapshot.

use super::{CLIPBOARD_READ_TIMEOUT, ClipboardPasteResult, MAX_CLIPBOARD_GIF_BYTES, image, system};
use std::time::Duration;

/// Mimes that carry the copied image's source URL, in trust order. Chromium's
/// is plain UTF-8; `text/x-moz-url` is UTF-16LE with the title on line two.
const SOURCE_URL_MIMES: [&str; 2] = ["chromium/x-source-url", "text/x-moz-url"];

/// A URL is a handful of bytes; anything larger is not a URL.
const MAX_URL_PAYLOAD_BYTES: usize = 8 * 1024;

/// One network round trip on the paste worker; generous enough for an 8 MiB
/// GIF on a slow link, short enough that a dead server cannot pin a paste.
const FETCH_TIMEOUT: Duration = Duration::from_secs(8);

/// Sent as the User-Agent: hosts like Wikimedia return 403 to anonymous
/// clients, and their policy asks applications to identify themselves with a
/// way to reach the project.
const FETCH_USER_AGENT: &str = concat!(
    "wayscriber/",
    env!("CARGO_PKG_VERSION"),
    " (https://wayscriber.com)"
);

/// Attempts the rescue. `Some` carries a fully validated GIF paste result;
/// `None` means "proceed with the negotiated snapshot" for any reason
/// (no URL offered, not a `.gif`, fetch failed, bytes not a valid GIF).
pub(super) fn rescue_animated_gif(offered: &[String]) -> Option<ClipboardPasteResult> {
    // Builds made with WAYSCRIBER_NO_UPDATE_CHECK=1 promise no outbound
    // requests at all; that promise covers this fetch too.
    if crate::update_check::compiled_out() {
        return None;
    }
    let url = offered
        .iter()
        .filter_map(|mime| {
            SOURCE_URL_MIMES
                .iter()
                .position(|candidate| candidate == mime)
                .map(|priority| (priority, mime))
        })
        .min_by_key(|(priority, _)| *priority)
        .and_then(|(_, mime)| read_source_url(mime))?;
    if !is_fetchable_gif_url(&url) {
        return None;
    }

    // URLs can carry signed query parameters or credentials; log the host only.
    log::info!(
        "Clipboard offers no image/gif; fetching GIF from {}",
        url_host(&url)
    );
    let bytes = match crate::update_check::fetch_bytes(
        &url,
        FETCH_TIMEOUT,
        MAX_CLIPBOARD_GIF_BYTES,
        FETCH_USER_AGENT,
    ) {
        Ok(bytes) => bytes,
        Err(err) => {
            log::info!("GIF source fetch failed, falling back to the snapshot: {err}");
            return None;
        }
    };
    match image::decode_clipboard_image("image/gif", bytes) {
        result @ ClipboardPasteResult::Image(_) => Some(result),
        other => {
            log::info!(
                "Fetched GIF source did not validate, falling back to the snapshot: {}",
                other.summary()
            );
            None
        }
    }
}

fn read_source_url(mime: &str) -> Option<String> {
    match system::read_clipboard_mime(mime, MAX_URL_PAYLOAD_BYTES, CLIPBOARD_READ_TIMEOUT) {
        Ok(bytes) => decode_url_payload(mime, &bytes),
        Err(err) => {
            log::debug!("Could not read clipboard source URL mime '{mime}': {err:?}");
            None
        }
    }
}

/// First line of the payload as a trimmed string, decoding `text/x-moz-url`'s
/// UTF-16LE when needed.
fn decode_url_payload(mime: &str, bytes: &[u8]) -> Option<String> {
    let text = if mime == "text/x-moz-url" && looks_utf16le(bytes) {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16(&units).ok()?
    } else {
        String::from_utf8(bytes.to_vec()).ok()?
    };
    let url = text.lines().next()?.trim().trim_start_matches('\u{feff}');
    (!url.is_empty()).then(|| url.to_string())
}

/// An `https://…` URL is never valid UTF-16LE-as-UTF-8 and vice versa: ASCII
/// text encoded as UTF-16LE has a NUL as every second byte.
fn looks_utf16le(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xff, 0xfe]) || bytes.iter().skip(1).step_by(2).take(8).any(|&b| b == 0)
}

/// Host portion of an already-validated `https://` URL, for logging. Strips
/// any `user:password@` userinfo so credentials can never reach the log.
fn url_host(url: &str) -> &str {
    url.get(8..)
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .map(|authority| authority.rsplit('@').next().unwrap_or(authority))
        .unwrap_or("<unparseable>")
}

/// HTTPS only, and the path (query/fragment ignored) must end in `.gif`.
fn is_fetchable_gif_url(url: &str) -> bool {
    let https = url
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"));
    if !https || url.len() <= 8 {
        return false;
    }
    let path = url[8..].split(['?', '#']).next().unwrap_or_default();
    path.to_ascii_lowercase().ends_with(".gif")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_validation_requires_https_and_a_gif_path() {
        assert!(is_fetchable_gif_url(
            "https://upload.wikimedia.org/wikipedia/commons/2/2c/Rotating_earth_%28large%29.gif"
        ));
        assert!(is_fetchable_gif_url("https://example.com/a.GIF?width=200"));
        assert!(is_fetchable_gif_url("https://example.com/a.gif#frag"));
        assert!(is_fetchable_gif_url("HTTPS://example.com/a.gif"));
        assert!(!is_fetchable_gif_url("http://example.com/a.gif"));
        assert!(!is_fetchable_gif_url("https://example.com/a.png"));
        assert!(!is_fetchable_gif_url("https://example.com/gif"));
        assert!(!is_fetchable_gif_url("https://"));
        assert!(!is_fetchable_gif_url("file:///tmp/a.gif"));
        assert!(!is_fetchable_gif_url(""));
    }

    #[test]
    fn logged_host_never_contains_credentials() {
        assert_eq!(url_host("https://example.com/a.gif"), "example.com");
        assert_eq!(
            url_host("https://example.com:8443/a.gif"),
            "example.com:8443"
        );
        assert_eq!(
            url_host("https://user:secret@example.com/a.gif"),
            "example.com"
        );
        assert_eq!(
            url_host("https://user:se@cret@example.com/a.gif?tok=n"),
            "example.com"
        );
    }

    #[test]
    fn url_payloads_decode_plain_and_utf16le_forms() {
        assert_eq!(
            decode_url_payload("chromium/x-source-url", b"https://example.com/a.gif"),
            Some("https://example.com/a.gif".to_string())
        );

        // text/x-moz-url: UTF-16LE, URL on line one, title on line two.
        let mut moz = Vec::new();
        for unit in "https://example.com/a.gif\nEarth".encode_utf16() {
            moz.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(
            decode_url_payload("text/x-moz-url", &moz),
            Some("https://example.com/a.gif".to_string())
        );

        // With a byte-order mark.
        let mut with_bom = vec![0xff, 0xfe];
        with_bom.extend_from_slice(&moz);
        assert_eq!(
            decode_url_payload("text/x-moz-url", &with_bom),
            Some("https://example.com/a.gif".to_string())
        );

        assert_eq!(decode_url_payload("chromium/x-source-url", b"   \n"), None);
        assert_eq!(
            decode_url_payload("chromium/x-source-url", &[0xff, 0x00]),
            None
        );
    }
}
