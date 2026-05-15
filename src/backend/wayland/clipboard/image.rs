use super::{
    ClipboardPasteResult, MAX_CLIPBOARD_IMAGE_PIXELS, WAYSCRIBER_SELECTION_MIME, file_list,
};
use crate::draw::EmbeddedImage;
use crate::image_decode::{
    EncodedImageFormat, decode_rgba, format_from_mime_or_bytes, image_dimensions,
};

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
    let Some(format) = format_from_mime_or_bytes(mime_type, &bytes) else {
        return ClipboardPasteResult::DecodeFailed(format!("unsupported MIME type {}", mime_type));
    };
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
    if let Err(err) = decode_rgba(format, &bytes) {
        return ClipboardPasteResult::DecodeFailed(err);
    }
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
    }
}

#[cfg(test)]
mod tests {
    use crate::backend::wayland::clipboard::MAX_CLIPBOARD_IMAGE_BYTES;

    #[test]
    fn image_byte_cap_leaves_room_for_default_persisted_create_history() {
        let encoded_len = MAX_CLIPBOARD_IMAGE_BYTES.div_ceil(3) * 4;
        let duplicated_history_len = encoded_len * 2;
        let default_session_budget = 10 * 1024 * 1024;
        let json_margin = 512 * 1024;

        assert!(duplicated_history_len + json_margin < default_session_budget);
    }
}
