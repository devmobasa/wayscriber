use std::sync::Arc;

use crate::image_decode::{self, EncodedImageFormat};

use super::limits::validate_source;
use super::{PinCreateError, PinRefusal};

#[derive(Debug, Clone)]
pub(crate) struct PinImage {
    pub png: Arc<Vec<u8>>,
    pub argb32: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub stride: i32,
}

pub(crate) fn decode_png_bytes(
    bytes: Vec<u8>,
    declared_width: u32,
    declared_height: u32,
) -> Result<PinImage, PinCreateError> {
    validate_source(bytes.len(), declared_width, declared_height)?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(PinRefusal::InvalidImage.into());
    }
    let header_dimensions = image_decode::image_dimensions(EncodedImageFormat::Png, &bytes)
        .map_err(|_| PinCreateError::Refused(PinRefusal::InvalidImage))?;
    if header_dimensions != (declared_width, declared_height) {
        return Err(PinRefusal::MetadataMismatch.into());
    }
    let decoded = image_decode::decode_rgba(EncodedImageFormat::Png, &bytes)
        .map_err(|_| PinCreateError::Refused(PinRefusal::InvalidImage))?;
    if (decoded.width, decoded.height) != (declared_width, declared_height) {
        return Err(PinRefusal::MetadataMismatch.into());
    }
    let stride_u32 = declared_width
        .checked_mul(4)
        .ok_or(PinRefusal::LimitExceeded)?;
    let stride = i32::try_from(stride_u32).map_err(|_| PinRefusal::LimitExceeded)?;
    let mut argb32 = decoded.rgba;
    rgba_to_premultiplied_argb32_in_place(&mut argb32);
    Ok(PinImage {
        png: Arc::new(bytes),
        argb32: Arc::new(argb32),
        width: declared_width,
        height: declared_height,
        stride,
    })
}

fn rgba_to_premultiplied_argb32_in_place(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        let premultiply = |channel: u8| (u32::from(channel) * alpha + 127) / 255;
        let native = (alpha << 24)
            | (premultiply(pixel[0]) << 16)
            | (premultiply(pixel[1]) << 8)
            | premultiply(pixel[2]);
        pixel.copy_from_slice(&native.to_ne_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_pixel_png() -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[200, 100, 50, 128]).unwrap();
        writer.finish().unwrap();
        bytes
    }

    #[test]
    fn decode_retains_exact_png_and_premultiplies_for_cairo() {
        let bytes = one_pixel_png();
        let decoded = decode_png_bytes(bytes.clone(), 1, 1).unwrap();
        assert_eq!(&*decoded.png, &bytes);
        let expected = ((128_u32 << 24) | (100 << 16) | (50 << 8) | 25).to_ne_bytes();
        assert_eq!(&*decoded.argb32, &expected);
    }

    #[test]
    fn header_dimensions_must_match_declared_metadata() {
        assert!(matches!(
            decode_png_bytes(one_pixel_png(), 2, 1),
            Err(PinCreateError::Refused(PinRefusal::MetadataMismatch))
        ));
    }

    #[test]
    fn non_png_is_rejected_before_decode() {
        assert!(matches!(
            decode_png_bytes(b"not a png".to_vec(), 1, 1),
            Err(PinCreateError::Refused(PinRefusal::InvalidImage))
        ));
    }

    #[test]
    fn opaque_argb_conversion_preserves_channels_in_native_cairo_order() {
        let mut pixel = [1, 2, 3, 255];
        rgba_to_premultiplied_argb32_in_place(&mut pixel);
        assert_eq!(pixel, 0xff01_0203_u32.to_ne_bytes());
    }
}
