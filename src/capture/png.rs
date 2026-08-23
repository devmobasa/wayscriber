use crate::capture::{CaptureError, ImageFormatMetadata, RenderedImage};
use crate::screen_pixels::PackedArgb32;

/// Encode tightly packed premultiplied native-order Cairo ARGB32 pixels as PNG.
pub fn encode_packed_argb32_png(pixels: &PackedArgb32) -> Result<RenderedImage, CaptureError> {
    if pixels.width() == 0 || pixels.height() == 0 {
        return Err(CaptureError::ImageError(
            "Packed ARGB32 PNG encoding requires non-empty pixels".to_string(),
        ));
    }
    let width = i32::try_from(pixels.width())
        .map_err(|_| CaptureError::ImageError("ARGB32 image width is too large".to_string()))?;
    let height = i32::try_from(pixels.height())
        .map_err(|_| CaptureError::ImageError("ARGB32 image height is too large".to_string()))?;
    let surface = cairo::ImageSurface::create_for_data(
        pixels.data().to_vec(),
        cairo::Format::ARgb32,
        width,
        height,
        pixels.stride(),
    )
    .map_err(|err| {
        CaptureError::ImageError(format!("Failed to create packed ARGB32 surface: {err}"))
    })?;
    let mut bytes = Vec::new();
    surface.write_to_png(&mut bytes).map_err(|err| {
        CaptureError::ImageError(format!("Failed to encode packed ARGB32 PNG: {err}"))
    })?;
    Ok(RenderedImage {
        bytes,
        format: ImageFormatMetadata::png(),
        width: pixels.width(),
        height: pixels.height(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native_argb(pixels: &[u32]) -> Vec<u8> {
        pixels
            .iter()
            .flat_map(|pixel| pixel.to_ne_bytes())
            .collect()
    }

    #[test]
    fn packed_argb32_png_preserves_alpha_and_premultiplied_channels() {
        let source_data = native_argb(&[
            0xFF00_00FF, // opaque blue
            0x0000_0000, // fully transparent
            0x8040_4040, // half-alpha premultiplied grey
        ]);
        let source = PackedArgb32::new(3, 1, 12, source_data.clone()).unwrap();

        let rendered = encode_packed_argb32_png(&source).expect("crop encodes");
        assert_eq!((rendered.width, rendered.height), (3, 1));
        assert_eq!(rendered.format, ImageFormatMetadata::png());

        let decoded = cairo::ImageSurface::create_from_png(&mut rendered.bytes.as_slice())
            .expect("encoded PNG decodes");
        let mut decoded = decoded;
        let stride = decoded.stride() as usize;
        let data = decoded.data().expect("decoded pixels");
        assert_eq!(&data[..12], &source_data);
        assert!(stride >= 12);
    }

    #[test]
    fn packed_argb32_png_rejects_empty_dimensions_before_cairo() {
        let zero_width = PackedArgb32::new(0, 4, 0, Vec::new()).unwrap();
        let zero_height = PackedArgb32::new(4, 0, 16, Vec::new()).unwrap();
        assert!(matches!(
            encode_packed_argb32_png(&zero_width),
            Err(CaptureError::ImageError(_))
        ));
        assert!(matches!(
            encode_packed_argb32_png(&zero_height),
            Err(CaptureError::ImageError(_))
        ));
    }
}
