use super::{ClipboardPasteResult, WAYSCRIBER_SELECTION_MIME, file_list};
use crate::draw::EmbeddedImage;
use crate::image_decode::{
    EncodedImageFormat, decode_rgba, format_from_mime_or_bytes, image_dimensions,
};
use crate::screen_pixels::EmbeddedImageLimits;

pub(super) fn choose_supported_mime(offered: &[String]) -> Option<String> {
    if let Some(mime) = [
        WAYSCRIBER_SELECTION_MIME,
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

pub(super) fn decode_clipboard_image(mime_type: &str, bytes: Vec<u8>) -> ClipboardPasteResult {
    let encoded_bytes = bytes.len();
    let Some(format) = format_from_mime_or_bytes(mime_type, &bytes) else {
        return ClipboardPasteResult::DecodeFailed(format!("unsupported MIME type {}", mime_type));
    };
    let dimensions = match image_dimensions(format, &bytes) {
        Ok(dimensions) => dimensions,
        Err(err) => return ClipboardPasteResult::DecodeFailed(err),
    };
    let limits = EmbeddedImageLimits::default();
    if !limits.allows_pixels(dimensions.0, dimensions.1) {
        return ClipboardPasteResult::TooManyPixels {
            width: dimensions.0,
            height: dimensions.1,
            limit: limits.max_pixels(),
        };
    }
    if let Err(err) = decode_rgba(format, &bytes) {
        return ClipboardPasteResult::DecodeFailed(err);
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
        bytes: bytes.into(),
    })
}

fn canonical_image_mime_type(format: EncodedImageFormat) -> &'static str {
    match format {
        EncodedImageFormat::Png => "image/png",
        EncodedImageFormat::Jpeg => "image/jpeg",
    }
}

#[cfg(test)]
mod tests {
    use crate::screen_pixels::EmbeddedImageLimits;

    #[test]
    fn image_byte_cap_leaves_room_for_default_persisted_create_history() {
        let encoded_len = EmbeddedImageLimits::default().max_bytes().div_ceil(3) * 4;
        let duplicated_history_len = encoded_len * 2;
        let default_session_budget = 50 * 1024 * 1024;
        let json_margin = 512 * 1024;

        assert!(duplicated_history_len + json_margin < default_session_budget);
    }
}
