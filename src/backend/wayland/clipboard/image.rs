use super::{
    ClipboardPasteResult, MAX_CLIPBOARD_GIF_BYTES, MAX_CLIPBOARD_IMAGE_BYTES,
    MAX_CLIPBOARD_IMAGE_PIXELS, WAYSCRIBER_SELECTION_MIME, file_list,
};
use crate::draw::EmbeddedImage;
use crate::image_decode::{
    EncodedImageFormat, decode_rgba, format_from_mime_or_bytes, gif_animation_verdict,
    image_dimensions,
};

pub(super) fn choose_supported_mime(offered: &[String]) -> Option<String> {
    // GIF outranks PNG: sources offering both give PNG as a rasterized first
    // frame, and picking it would lose the animation permanently.
    if let Some(mime) = [
        WAYSCRIBER_SELECTION_MIME,
        "image/gif",
        "image/png",
        "image/jpeg",
        "image/jpg",
    ]
    .into_iter()
    .find(|candidate| offered.iter().any(|mime| mime == candidate))
    .map(ToString::to_string)
    {
        return Some(mime);
    }

    offered
        .iter()
        .find(|mime| file_list::is_uri_list_mime(mime))
        .cloned()
}

/// Per-format byte cap, enforced after sniffing so file pastes (read before
/// the format is known) get the same limits as direct clipboard reads.
pub(super) fn clipboard_byte_limit(format: EncodedImageFormat) -> usize {
    match format {
        EncodedImageFormat::Gif => MAX_CLIPBOARD_GIF_BYTES,
        EncodedImageFormat::Png | EncodedImageFormat::Jpeg => MAX_CLIPBOARD_IMAGE_BYTES,
    }
}

pub(super) fn decode_clipboard_image(mime_type: &str, bytes: Vec<u8>) -> ClipboardPasteResult {
    let encoded_bytes = bytes.len();
    let Some(format) = format_from_mime_or_bytes(mime_type, &bytes) else {
        return ClipboardPasteResult::DecodeFailed(format!("unsupported MIME type {}", mime_type));
    };
    let byte_limit = clipboard_byte_limit(format);
    if encoded_bytes > byte_limit {
        return ClipboardPasteResult::TooLarge { limit: byte_limit };
    }
    let dimensions = match image_dimensions(format, &bytes) {
        Ok(dimensions) => dimensions,
        Err(err) => return ClipboardPasteResult::DecodeFailed(err),
    };
    let pixels = dimensions.0 as u64 * dimensions.1 as u64;
    if pixels > MAX_CLIPBOARD_IMAGE_PIXELS {
        return ClipboardPasteResult::TooManyPixels {
            width: dimensions.0,
            height: dimensions.1,
            limit: MAX_CLIPBOARD_IMAGE_PIXELS,
        };
    }
    match format {
        // Full-stream validation: every frame's LZW data decodes (pixels
        // discarded). An over-budget animation still pastes — it renders as a
        // static first frame, and the apply path raises an info toast.
        EncodedImageFormat::Gif => {
            if let Err(err) = gif_animation_verdict(&bytes) {
                return ClipboardPasteResult::DecodeFailed(err);
            }
        }
        EncodedImageFormat::Png | EncodedImageFormat::Jpeg => {
            if let Err(err) = decode_rgba(format, &bytes) {
                return ClipboardPasteResult::DecodeFailed(err);
            }
        }
    }
    log::info!(
        "Decoded clipboard image: offered_mime={}, stored_mime={}, dimensions={}x{}, encoded_bytes={}",
        mime_type,
        canonical_image_mime_type(format),
        dimensions.0,
        dimensions.1,
        encoded_bytes
    );
    ClipboardPasteResult::Image(EmbeddedImage {
        mime_type: canonical_image_mime_type(format).to_string(),
        width: dimensions.0,
        height: dimensions.1,
        bytes,
    })
}

fn canonical_image_mime_type(format: EncodedImageFormat) -> &'static str {
    match format {
        EncodedImageFormat::Png => "image/png",
        EncodedImageFormat::Jpeg => "image/jpeg",
        EncodedImageFormat::Gif => "image/gif",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::clipboard::MAX_CLIPBOARD_IMAGE_BYTES;

    #[test]
    fn image_byte_cap_leaves_room_for_default_persisted_create_history() {
        let encoded_len = MAX_CLIPBOARD_IMAGE_BYTES.div_ceil(3) * 4;
        let duplicated_history_len = encoded_len * 2;
        let default_session_budget = 50 * 1024 * 1024;
        let json_margin = 512 * 1024;

        assert!(duplicated_history_len + json_margin < default_session_budget);
    }

    #[test]
    fn gif_byte_cap_leaves_room_for_default_persisted_create_history() {
        let encoded_len = MAX_CLIPBOARD_GIF_BYTES.div_ceil(3) * 4;
        let duplicated_history_len = encoded_len * 2;
        let default_session_budget = 50 * 1024 * 1024;
        let json_margin = 512 * 1024;

        assert!(duplicated_history_len + json_margin < default_session_budget);
    }

    fn offered(mimes: &[&str]) -> Vec<String> {
        mimes.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn gif_outranks_rasterized_png_but_not_the_private_selection() {
        assert_eq!(
            choose_supported_mime(&offered(&["image/png", "image/gif"])),
            Some("image/gif".to_string())
        );
        assert_eq!(
            choose_supported_mime(&offered(&[
                "image/gif",
                super::super::WAYSCRIBER_SELECTION_MIME
            ])),
            Some(super::super::WAYSCRIBER_SELECTION_MIME.to_string())
        );
        assert_eq!(
            choose_supported_mime(&offered(&["image/png", "image/jpeg"])),
            Some("image/png".to_string())
        );
    }

    /// 2x2 GIF with one solid frame per entry in `frame_colors`.
    fn tiny_gif(frame_colors: &[u8]) -> Vec<u8> {
        let palette = [255, 0, 0, 0, 0, 255];
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut bytes, 2, 2, &palette).unwrap();
            for &color in frame_colors {
                let mut frame = gif::Frame::default();
                frame.width = 2;
                frame.height = 2;
                frame.buffer = vec![color; 4].into();
                frame.delay = 10;
                encoder.write_frame(&frame).unwrap();
            }
        }
        bytes
    }

    #[test]
    fn gif_paste_decodes_and_stores_original_bytes() {
        let bytes = tiny_gif(&[0, 1]);
        let result = decode_clipboard_image("image/gif", bytes.clone());
        let ClipboardPasteResult::Image(image) = result else {
            panic!("expected Image result");
        };
        assert_eq!(image.mime_type, "image/gif");
        assert_eq!((image.width, image.height), (2, 2));
        assert_eq!(image.bytes, bytes, "original payload stored verbatim");
    }

    #[test]
    fn gif_paste_sniffs_octet_stream_payloads() {
        // The file-manager URI path reads bytes before any format is known.
        let result = decode_clipboard_image("application/octet-stream", tiny_gif(&[0]));
        assert!(matches!(result, ClipboardPasteResult::Image(_)));
    }

    #[test]
    fn oversized_non_gif_octet_stream_is_rejected_after_sniffing() {
        // A PNG-looking payload above the 3 MiB cap must be rejected even
        // though the URI-list read cap is the more permissive GIF limit.
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.resize(MAX_CLIPBOARD_IMAGE_BYTES + 1, 0);
        let result = decode_clipboard_image("application/octet-stream", bytes);
        assert!(
            matches!(
                result,
                ClipboardPasteResult::TooLarge {
                    limit: MAX_CLIPBOARD_IMAGE_BYTES
                }
            ),
            "got {result:?}"
        );
    }

    #[test]
    fn truncated_gif_paste_fails_decode() {
        let bytes = tiny_gif(&[0, 1]);
        let result = decode_clipboard_image("image/gif", bytes[..20].to_vec());
        assert!(matches!(result, ClipboardPasteResult::DecodeFailed(_)));
    }
}
