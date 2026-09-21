use std::io::Cursor;

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

impl DecodedImage {
    pub(crate) fn into_cairo_argb(mut self, stride: usize) -> Result<Vec<u8>, String> {
        if self.width == 0 || self.height == 0 {
            return Err("decoded image is empty".to_string());
        }

        let row_len = pixel_count(self.width, 1)?
            .checked_mul(4)
            .ok_or_else(|| "image dimensions are too large".to_string())?;
        let height = usize::try_from(self.height)
            .map_err(|_| "image dimensions are too large".to_string())?;
        let required_len = row_len
            .checked_mul(height)
            .ok_or_else(|| "image dimensions are too large".to_string())?;
        if self.rgba.len() != required_len || stride < row_len {
            return Err("decoded image has an unexpected RGBA layout".to_string());
        }

        for pixel in self.rgba.as_chunks_mut::<4>().0 {
            let [r, g, b, a] = *pixel;
            let premul = |channel: u8| ((u16::from(channel) * u16::from(a) + 127) / 255) as u8;
            let [r, g, b] = [premul(r), premul(g), premul(b)];
            *pixel = if cfg!(target_endian = "little") {
                [b, g, r, a]
            } else {
                [a, r, g, b]
            };
        }

        if stride == row_len {
            return Ok(self.rgba);
        }

        let mut padded = vec![
            0;
            stride
                .checked_mul(height)
                .ok_or("image stride is too large")?
        ];
        for (source, target) in self
            .rgba
            .chunks_exact(row_len)
            .zip(padded.chunks_exact_mut(stride))
        {
            target[..row_len].copy_from_slice(source);
        }
        Ok(padded)
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

pub(crate) fn decode_rgba(
    format: EncodedImageFormat,
    bytes: &[u8],
) -> Result<DecodedImage, String> {
    match format {
        EncodedImageFormat::Png => decode_png_rgba(bytes),
        EncodedImageFormat::Jpeg => decode_jpeg_rgba(bytes),
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

fn decode_png_rgba(bytes: &[u8]) -> Result<DecodedImage, String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|err| err.to_string())?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| "PNG output buffer is too large".to_string())?;
    let rgba_len = pixel_count(reader.info().width, reader.info().height)?
        .checked_mul(4)
        .ok_or_else(|| "image dimensions are too large".to_string())?;
    let mut buffer = vec![0; size.max(rgba_len)];
    let output = reader
        .next_frame(&mut buffer)
        .map_err(|err| err.to_string())?;
    if output.bit_depth != png::BitDepth::Eight {
        return Err(format!("unsupported PNG bit depth {:?}", output.bit_depth));
    }

    buffer.truncate(output.buffer_size());
    let rgba = normalize_png_rgba(output.color_type, output.width, output.height, buffer)?;
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
    mut data: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let pixels = pixel_count(width, height)?;
    let rgba_len = pixels
        .checked_mul(4)
        .ok_or_else(|| "image dimensions are too large".to_string())?;

    match color_type {
        png::ColorType::Rgba => {
            if data.len() != rgba_len {
                return Err("decoded PNG RGBA data has an unexpected length".to_string());
            }
            Ok(data)
        }
        png::ColorType::Rgb => {
            if data.len() != pixels * 3 {
                return Err("decoded PNG RGB data has an unexpected length".to_string());
            }
            Ok(expand_rgb_to_rgba(data, rgba_len))
        }
        png::ColorType::Grayscale => {
            if data.len() != pixels {
                return Err("decoded PNG grayscale data has an unexpected length".to_string());
            }
            data.resize(rgba_len, 0);
            for index in (0..pixels).rev() {
                let gray = data[index];
                data[index * 4..index * 4 + 4].copy_from_slice(&[gray, gray, gray, 255]);
            }
            Ok(data)
        }
        png::ColorType::GrayscaleAlpha => {
            if data.len() != pixels * 2 {
                return Err("decoded PNG grayscale-alpha data has an unexpected length".to_string());
            }
            data.resize(rgba_len, 0);
            for index in (0..pixels).rev() {
                let gray = data[index * 2];
                let alpha = data[index * 2 + 1];
                data[index * 4..index * 4 + 4].copy_from_slice(&[gray, gray, gray, alpha]);
            }
            Ok(data)
        }
        png::ColorType::Indexed => Err("indexed PNG data was not expanded".to_string()),
    }
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

fn decode_jpeg_rgba(bytes: &[u8]) -> Result<DecodedImage, String> {
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    let rgb = decoder.decode().map_err(|err| err.to_string())?;
    let info = decoder
        .info()
        .ok_or_else(|| "JPEG did not include dimensions".to_string())?;
    let width = u32::from(info.width);
    let height = u32::from(info.height);
    let expected_len = pixel_count(width, height)?
        .checked_mul(3)
        .ok_or_else(|| "image dimensions are too large".to_string())?;
    if rgb.len() != expected_len {
        return Err("decoded JPEG RGB data has an unexpected length".to_string());
    }

    Ok(DecodedImage {
        width,
        height,
        rgba: expand_rgb_to_rgba(
            rgb,
            pixel_count(width, height)?
                .checked_mul(4)
                .ok_or_else(|| "image dimensions are too large".to_string())?,
        ),
    })
}

fn expand_rgb_to_rgba(mut rgb: Vec<u8>, rgba_len: usize) -> Vec<u8> {
    let pixels = rgb.len() / 3;
    rgb.resize(rgba_len, 0);
    for index in (0..pixels).rev() {
        let source = index * 3;
        let target = index * 4;
        let [r, g, b] = [rgb[source], rgb[source + 1], rgb[source + 2]];
        rgb[target..target + 4].copy_from_slice(&[r, g, b, 255]);
    }
    rgb
}

fn pixel_count(width: u32, height: u32) -> Result<usize, String> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| "image dimensions are too large".to_string())?;
    usize::try_from(pixels).map_err(|_| "image dimensions are too large".to_string())
}

#[cfg(test)]
mod tests {
    use super::{DecodedImage, EncodedImageFormat, decode_rgba, normalize_png_rgba};

    fn png(color_type: png::ColorType, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(color_type);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(pixels)
                .unwrap();
        }
        bytes
    }

    #[test]
    fn png_rgb_and_rgba_decode_to_the_same_pixels() {
        for (color_type, pixels) in [
            (png::ColorType::Rgb, &[80, 160, 240][..]),
            (png::ColorType::Rgba, &[80, 160, 240, 255][..]),
        ] {
            let decoded = decode_rgba(EncodedImageFormat::Png, &png(color_type, pixels)).unwrap();
            assert_eq!((decoded.width, decoded.height), (1, 1));
            assert_eq!(decoded.rgba, [80, 160, 240, 255]);
        }
    }

    #[test]
    fn png_grayscale_expansion_preserves_alpha() {
        for (color_type, pixels, expected) in [
            (png::ColorType::Grayscale, &[80][..], &[80, 80, 80, 255][..]),
            (
                png::ColorType::GrayscaleAlpha,
                &[80, 128][..],
                &[80, 80, 80, 128][..],
            ),
        ] {
            let decoded = decode_rgba(EncodedImageFormat::Png, &png(color_type, pixels)).unwrap();
            assert_eq!(decoded.rgba, expected);
        }
    }

    #[test]
    fn png_rgb_expansion_keeps_each_pixel_in_order() {
        let rgb = vec![10, 20, 30, 40, 50, 60];

        let rgba = normalize_png_rgba(png::ColorType::Rgb, 2, 1, rgb).unwrap();

        assert_eq!(rgba, [10, 20, 30, 255, 40, 50, 60, 255]);
    }

    #[test]
    fn cairo_conversion_reuses_packed_rgba_storage_and_premultiplies_alpha() {
        let decoded = DecodedImage {
            width: 2,
            height: 1,
            rgba: vec![200, 100, 50, 128, 10, 20, 30, 0],
        };
        let allocation = decoded.rgba.as_ptr();

        let argb = decoded.into_cairo_argb(8).unwrap();

        assert_eq!(argb.as_ptr(), allocation);
        if cfg!(target_endian = "little") {
            assert_eq!(argb, [25, 50, 100, 128, 0, 0, 0, 0]);
        } else {
            assert_eq!(argb, [128, 100, 50, 25, 0, 0, 0, 0]);
        }
    }

    #[test]
    fn cairo_conversion_pads_rows_when_the_destination_stride_requires_it() {
        let decoded = DecodedImage {
            width: 1,
            height: 2,
            rgba: vec![255, 0, 0, 255, 0, 255, 0, 255],
        };

        let argb = decoded.into_cairo_argb(8).unwrap();

        if cfg!(target_endian = "little") {
            assert_eq!(
                argb,
                [0, 0, 255, 255, 0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 0, 0]
            );
        } else {
            assert_eq!(
                argb,
                [255, 255, 0, 0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 0, 0, 0]
            );
        }
    }

    #[test]
    fn decode_jpeg_rgba_preserves_cmyk_jpeg_colors() {
        let bytes = crate::base64::decode_standard(CMYK_RED_JPEG).unwrap();

        let image = decode_rgba(EncodedImageFormat::Jpeg, &bytes).unwrap();

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.rgba, [255, 0, 0, 255]);
    }

    const CMYK_RED_JPEG: &str = "\
        /9j/7gAOQWRvYmUAZAAAAAAC/9sAQwADAgICAgIDAgICAwMDAwQGBAQEBAQIBgYFBgkI\
        CgoJCAkJCgwPDAoLDgsJCQ0RDQ4PEBAREAoMEhMSEBMPEBAQ/9sAQwEDAwMEAwQIBAQI\
        EAsJCxAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBA\
        QEBAQ/8AAFAgAAQABBAERAAIRAQMRAQQRAP/EABUAAQEAAAAAAAAAAAAAAAAAAAgJ/8Q\
        AFBABAAAAAAAAAAAAAAAAAAAAAP/EABUBAQEAAAAAAAAAAAAAAAAAAAcJ/8QAFBEBAAA\
        AAAAAAAAAAAAAAAAAAP/aAA4EAQACEQMRBAAAPwBEHNKpVN//2Q==";
}
