use std::io::Cursor;

use crate::screen_pixels::EmbeddedImageLimits;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EncodedImageFormat {
    Png,
    Jpeg,
}

#[derive(Debug)]
pub(crate) struct DecodedImage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

/// The largest image one decode may produce.
///
/// The decoder checks the header against this budget before it allocates a
/// frame buffer, so a small, highly compressible file cannot claim gigabytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DecodeLimits {
    max_pixels: u64,
}

impl DecodeLimits {
    /// A screenshot of the whole output layout, which is cropped to one output
    /// after decoding. 16384 pixels per side is the cap zune-jpeg already
    /// applied by default and a common maximum GPU texture size.
    pub(crate) const SCREEN_CAPTURE: Self = Self {
        max_pixels: 16_384 * 16_384,
    };

    fn check(self, width: u32, height: u32) -> Result<(), String> {
        let within = u64::from(width)
            .checked_mul(u64::from(height))
            .is_some_and(|pixels| pixels <= self.max_pixels);
        if within {
            return Ok(());
        }

        Err(format!(
            "image {width}x{height} exceeds the {} pixel decode budget",
            self.max_pixels
        ))
    }
}

impl From<EmbeddedImageLimits> for DecodeLimits {
    fn from(limits: EmbeddedImageLimits) -> Self {
        Self {
            max_pixels: limits.max_pixels(),
        }
    }
}

pub(crate) fn format_from_mime_or_bytes(
    mime_type: &str,
    bytes: &[u8],
) -> Option<EncodedImageFormat> {
    match mime_type {
        "image/png" => Some(EncodedImageFormat::Png),
        "image/jpeg" | "image/jpg" => Some(EncodedImageFormat::Jpeg),
        _ => guess_format(bytes),
    }
}

#[allow(dead_code)]
pub(crate) fn image_dimensions(
    format: EncodedImageFormat,
    bytes: &[u8],
) -> Result<(u32, u32), String> {
    match format {
        EncodedImageFormat::Png => png_dimensions(bytes),
        EncodedImageFormat::Jpeg => jpeg_dimensions(bytes),
    }
}

/// Decodes an image to RGBA8, or fails without allocating a frame buffer
/// when the header declares more pixels than `limits` allows.
pub(crate) fn decode_rgba(
    format: EncodedImageFormat,
    bytes: &[u8],
    limits: DecodeLimits,
) -> Result<DecodedImage, String> {
    match format {
        EncodedImageFormat::Png => decode_png_rgba(bytes, limits),
        EncodedImageFormat::Jpeg => decode_jpeg_rgba(bytes, limits),
    }
}

fn guess_format(bytes: &[u8]) -> Option<EncodedImageFormat> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(EncodedImageFormat::Png);
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(EncodedImageFormat::Jpeg);
    }
    None
}

#[allow(dead_code)]
fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32), String> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let reader = decoder.read_info().map_err(|err| err.to_string())?;
    let info = reader.info();
    Ok((info.width, info.height))
}

fn decode_png_rgba(bytes: &[u8], limits: DecodeLimits) -> Result<DecodedImage, String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let header = decoder.read_header_info().map_err(|err| err.to_string())?;
    limits.check(header.width, header.height)?;

    let mut reader = decoder.read_info().map_err(|err| err.to_string())?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| "PNG output buffer is too large".to_string())?;
    let mut buffer = vec![0; size];
    let output = reader
        .next_frame(&mut buffer)
        .map_err(|err| err.to_string())?;
    if output.bit_depth != png::BitDepth::Eight {
        return Err(format!("unsupported PNG bit depth {:?}", output.bit_depth));
    }

    let data = &buffer[..output.buffer_size()];
    let rgba = normalize_png_rgba(output.color_type, output.width, output.height, data)?;
    Ok(DecodedImage {
        width: output.width,
        height: output.height,
        rgba,
    })
}

fn normalize_png_rgba(
    color_type: png::ColorType,
    width: u32,
    height: u32,
    data: &[u8],
) -> Result<Vec<u8>, String> {
    let pixels = pixel_count(width, height)?;
    let mut rgba = Vec::with_capacity(
        pixels
            .checked_mul(4)
            .ok_or_else(|| "image dimensions are too large".to_string())?,
    );

    match color_type {
        png::ColorType::Rgba => {
            if data.len() != pixels * 4 {
                return Err("decoded PNG RGBA data has an unexpected length".to_string());
            }
            rgba.extend_from_slice(data);
        }
        png::ColorType::Rgb => {
            if data.len() != pixels * 3 {
                return Err("decoded PNG RGB data has an unexpected length".to_string());
            }
            for pixel in data.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            if data.len() != pixels {
                return Err("decoded PNG grayscale data has an unexpected length".to_string());
            }
            for &gray in data {
                rgba.extend_from_slice(&[gray, gray, gray, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            if data.len() != pixels * 2 {
                return Err("decoded PNG grayscale-alpha data has an unexpected length".to_string());
            }
            for pixel in data.as_chunks::<2>().0 {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        png::ColorType::Indexed => {
            return Err("indexed PNG data was not expanded".to_string());
        }
    }

    Ok(rgba)
}

#[allow(dead_code)]
fn jpeg_dimensions(bytes: &[u8]) -> Result<(u32, u32), String> {
    let mut decoder =
        zune_jpeg::JpegDecoder::new(zune_jpeg::zune_core::bytestream::ZCursor::new(bytes));
    decoder.decode_headers().map_err(|err| err.to_string())?;
    let info = decoder
        .info()
        .ok_or_else(|| "JPEG headers did not include dimensions".to_string())?;
    Ok((u32::from(info.width), u32::from(info.height)))
}

fn decode_jpeg_rgba(bytes: &[u8], limits: DecodeLimits) -> Result<DecodedImage, String> {
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder.decode_headers().map_err(|err| err.to_string())?;
    let info = decoder
        .info()
        .ok_or_else(|| "JPEG did not include dimensions".to_string())?;
    let width = u32::from(info.width);
    let height = u32::from(info.height);
    limits.check(width, height)?;

    let rgb = decoder.decode().map_err(|err| err.to_string())?;
    let expected_len = pixel_count(width, height)?
        .checked_mul(3)
        .ok_or_else(|| "image dimensions are too large".to_string())?;
    if rgb.len() != expected_len {
        return Err("decoded JPEG RGB data has an unexpected length".to_string());
    }

    Ok(DecodedImage {
        width,
        height,
        rgba: rgb_to_rgba(&rgb),
    })
}

fn rgb_to_rgba(rgb: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
    for pixel in rgb.as_chunks::<3>().0 {
        rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    }
    rgba
}

fn pixel_count(width: u32, height: u32) -> Result<usize, String> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| "image dimensions are too large".to_string())?;
    usize::try_from(pixels).map_err(|_| "image dimensions are too large".to_string())
}

#[cfg(test)]
mod tests {
    use super::{DecodeLimits, EncodedImageFormat, decode_rgba};
    use crate::screen_pixels::EmbeddedImageLimits;

    fn encode_rgba_png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&vec![0x7f; width as usize * height as usize * 4])
            .unwrap();
        writer.finish().unwrap();
        bytes
    }

    /// A PNG signature and IHDR declaring `width`x`height`, with no image data.
    fn png_header_only(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        drop(encoder.write_header().unwrap());
        bytes
    }

    #[test]
    fn decode_jpeg_rgba_preserves_cmyk_jpeg_colors() {
        let bytes = crate::base64::decode_standard(CMYK_RED_JPEG).unwrap();

        let image = decode_rgba(
            EncodedImageFormat::Jpeg,
            &bytes,
            EmbeddedImageLimits::default().into(),
        )
        .unwrap();

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.rgba, [255, 0, 0, 255]);
    }

    #[test]
    fn png_header_over_the_embedded_budget_is_rejected_before_image_data() {
        let bytes = png_header_only(20_000, 20_000);

        let err = decode_rgba(
            EncodedImageFormat::Png,
            &bytes,
            EmbeddedImageLimits::default().into(),
        )
        .unwrap_err();

        assert!(
            err.contains("20000x20000 exceeds the 48000000 pixel decode budget"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn png_decode_admits_exactly_the_budget_and_rejects_one_pixel_more() {
        let bytes = encode_rgba_png(2, 2);

        let at_budget = decode_rgba(
            EncodedImageFormat::Png,
            &bytes,
            DecodeLimits { max_pixels: 4 },
        )
        .unwrap();
        let over_budget = decode_rgba(
            EncodedImageFormat::Png,
            &bytes,
            DecodeLimits { max_pixels: 3 },
        );

        assert_eq!((at_budget.width, at_budget.height), (2, 2));
        assert_eq!(at_budget.rgba.len(), 16);
        assert!(over_budget.unwrap_err().contains("decode budget"));
    }

    #[test]
    fn jpeg_header_over_the_budget_is_rejected_before_decoding() {
        let bytes = crate::base64::decode_standard(CMYK_RED_JPEG).unwrap();

        let err = decode_rgba(
            EncodedImageFormat::Jpeg,
            &bytes,
            DecodeLimits { max_pixels: 0 },
        )
        .unwrap_err();

        assert!(
            err.contains("1x1 exceeds the 0 pixel decode budget"),
            "{err}"
        );
    }

    const CMYK_RED_JPEG: &str = "\
        /9j/7gAOQWRvYmUAZAAAAAAC/9sAQwADAgICAgIDAgICAwMDAwQGBAQEBAQIBgYFBgkI\
        CgoJCAkJCgwPDAoLDgsJCQ0RDQ4PEBAREAoMEhMSEBMPEBAQ/9sAQwEDAwMEAwQIBAQI\
        EAsJCxAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBA\
        QEBAQ/8AAFAgAAQABBAERAAIRAQMRAQQRAP/EABUAAQEAAAAAAAAAAAAAAAAAAAgJ/8Q\
        AFBABAAAAAAAAAAAAAAAAAAAAAP/EABUBAQEAAAAAAAAAAAAAAAAAAAcJ/8QAFBEBAAA\
        AAAAAAAAAAAAAAAAAAP/aAA4EAQACEQMRBAAAPwBEHNKpVN//2Q==";
}
