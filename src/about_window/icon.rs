//! The app icon shown in the About header.
//!
//! Decoded from an embedded PNG so the dialog does not depend on an installed
//! icon theme; a failed decode simply leaves the header text-only.

use std::io::Cursor;

use log::debug;
use png::Decoder;

/// 64px source, painted at the header size.
const ICON_PNG: &[u8] = include_bytes!("../../assets/tray/wayscriber-64.png");

/// Decode the icon into a Cairo surface with premultiplied ARGB pixels.
pub(super) fn load() -> Option<cairo::ImageSurface> {
    let (width, height, argb) = decode_premultiplied_argb(ICON_PNG)?;

    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)
        .map_err(|err| debug!("Failed to allocate about icon surface: {err}"))
        .ok()?;
    let stride = surface.stride() as usize;
    {
        let mut data = surface
            .data()
            .map_err(|err| debug!("Failed to access about icon surface: {err}"))
            .ok()?;
        for row in 0..height as usize {
            let source = &argb[row * width as usize * 4..(row + 1) * width as usize * 4];
            data[row * stride..row * stride + source.len()].copy_from_slice(source);
        }
    }
    surface.mark_dirty();
    Some(surface)
}

/// Decode RGBA PNG bytes into Cairo's native-endian premultiplied ARGB32.
fn decode_premultiplied_argb(bytes: &[u8]) -> Option<(i32, i32, Vec<u8>)> {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|err| debug!("Failed to read about icon PNG: {err}"))
        .ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|err| debug!("Failed to decode about icon PNG: {err}"))
        .ok()?;

    let pixels = &buffer[..info.buffer_size()];
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        _ => return None,
    };

    let mut argb = Vec::with_capacity(info.width as usize * info.height as usize * 4);
    for chunk in pixels.chunks_exact(channels) {
        let alpha = if channels == 4 { chunk[3] } else { 255 };
        let premultiply = |value: u8| ((u32::from(value) * u32::from(alpha) + 127) / 255) as u8;
        // Cairo ARGB32 is a native-endian u32, i.e. BGRA byte order on
        // little-endian hosts.
        argb.push(premultiply(chunk[2]));
        argb.push(premultiply(chunk[1]));
        argb.push(premultiply(chunk[0]));
        argb.push(alpha);
    }

    Some((info.width as i32, info.height as i32, argb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_icon_decodes_to_a_square_image() {
        let (width, height, argb) = decode_premultiplied_argb(ICON_PNG).expect("icon decodes");

        assert_eq!(width, 64);
        assert_eq!(height, 64);
        assert_eq!(argb.len(), 64 * 64 * 4);
    }

    #[test]
    fn transparent_pixels_are_fully_premultiplied_away() {
        let (_, _, argb) = decode_premultiplied_argb(ICON_PNG).expect("icon decodes");

        for pixel in argb.chunks_exact(4) {
            let alpha = pixel[3];
            assert!(
                pixel[0] <= alpha && pixel[1] <= alpha && pixel[2] <= alpha,
                "channel exceeds alpha: {pixel:?}"
            );
        }
    }

    #[test]
    fn garbage_input_is_rejected_without_panicking() {
        assert!(decode_premultiplied_argb(b"not a png").is_none());
    }

    #[test]
    fn produces_a_cairo_surface() {
        let surface = load().expect("icon surface");

        assert_eq!(surface.width(), 64);
        assert_eq!(surface.height(), 64);
    }
}
