use std::io::Cursor;
use std::num::NonZeroU64;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EncodedImageFormat {
    Png,
    Jpeg,
    Gif,
}

#[derive(Debug)]
pub(crate) struct DecodedImage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

pub(crate) fn format_from_mime_or_bytes(
    mime_type: &str,
    bytes: &[u8],
) -> Option<EncodedImageFormat> {
    match mime_type {
        "image/png" => Some(EncodedImageFormat::Png),
        "image/jpeg" | "image/jpg" => Some(EncodedImageFormat::Jpeg),
        "image/gif" => Some(EncodedImageFormat::Gif),
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
        EncodedImageFormat::Gif => gif_dimensions(bytes),
    }
}

pub(crate) fn decode_rgba(
    format: EncodedImageFormat,
    bytes: &[u8],
) -> Result<DecodedImage, String> {
    match format {
        EncodedImageFormat::Png => decode_png_rgba(bytes),
        EncodedImageFormat::Jpeg => decode_jpeg_rgba(bytes),
        EncodedImageFormat::Gif => decode_gif_first_frame_rgba(bytes),
    }
}

fn guess_format(bytes: &[u8]) -> Option<EncodedImageFormat> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(EncodedImageFormat::Png);
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(EncodedImageFormat::Jpeg);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(EncodedImageFormat::Gif);
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
            for pixel in data.chunks_exact(3) {
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
            for pixel in data.chunks_exact(2) {
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
        rgba: rgb_to_rgba(&rgb),
    })
}

fn rgb_to_rgba(rgb: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
    for pixel in rgb.chunks_exact(3) {
        rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    }
    rgba
}

/// Upper bound on frames a pasted GIF may animate with. Tiny-frame GIFs can
/// pass the pixel budget with tens of thousands of frames, which would stress
/// tick scheduling; past this the paste degrades to a static first frame.
pub(crate) const MAX_ANIMATION_FRAMES: usize = 512;
/// Total composited RGBA pixels across all frames (frames × logical screen).
/// 32 Mpx caps the fully decoded animation at 128 MiB of pixel data.
pub(crate) const MAX_ANIMATION_TOTAL_PIXELS: u64 = 33_554_432;
/// Per-frame decoder allocation cap for untrusted clipboard bytes. Comfortably
/// above the 48 Mpx indexed frame the clipboard pixel budget allows.
const GIF_FRAME_MEMORY_LIMIT_BYTES: u64 = 64 * 1024 * 1024;

/// One composited frame of an animated GIF: the full logical screen, RGBA.
pub(crate) struct GifFrameRgba {
    pub(crate) delay: Duration,
    pub(crate) rgba: Vec<u8>,
}

/// Streaming GIF decoder that composites frames against the logical screen,
/// handling disposal methods and transparency. Frames only decode forward;
/// callers cache composited frames for random access.
pub(crate) struct GifStreamDecoder {
    decoder: gif::Decoder<Cursor<Vec<u8>>>,
    screen: gif_dispose::Screen,
}

impl GifStreamDecoder {
    pub(crate) fn new(bytes: &[u8]) -> Result<Self, String> {
        let decoder = gif_decoder(bytes.to_vec(), false)?;
        if decoder.width() == 0 || decoder.height() == 0 {
            return Err("GIF has zero dimensions".to_string());
        }
        let screen = gif_dispose::Screen::new_decoder(&decoder);
        Ok(Self { decoder, screen })
    }

    pub(crate) fn dimensions(&self) -> (u32, u32) {
        (
            u32::from(self.decoder.width()),
            u32::from(self.decoder.height()),
        )
    }

    /// Total playthroughs before holding the last frame; `None` = loop
    /// forever. Browser semantics: the NETSCAPE count is the number of
    /// *repetitions after* the initial playthrough, so `Finite(n)` plays
    /// n + 1 times, and an absent extension plays once. Reliable only after
    /// the first frame has been decoded, since the extension precedes the
    /// first image descriptor in the stream.
    pub(crate) fn loop_count(&self) -> Option<u32> {
        match self.decoder.repeat() {
            gif::Repeat::Infinite => None,
            gif::Repeat::Finite(0) => Some(1),
            gif::Repeat::Finite(n) => Some(u32::from(n).saturating_add(1)),
        }
    }

    /// Decodes and composites the next frame. `Ok(None)` = end of stream.
    pub(crate) fn next_frame(&mut self) -> Result<Option<GifFrameRgba>, String> {
        let Some(frame) = self
            .decoder
            .read_next_frame()
            .map_err(|err| err.to_string())?
        else {
            return Ok(None);
        };
        let delay = normalize_gif_delay(frame.delay);
        self.screen
            .blit_frame(frame)
            .map_err(|err| err.to_string())?;
        let pixels = self.screen.pixels_rgba();
        let mut rgba = Vec::with_capacity(pixels.width() * pixels.height() * 4);
        for row in pixels.rows() {
            for px in row {
                rgba.extend_from_slice(&[px.r, px.g, px.b, px.a]);
            }
        }
        Ok(Some(GifFrameRgba { delay, rgba }))
    }
}

/// GIF delays are u16 counts of 10 ms. 0 and 1 conventionally mean
/// "unspecified" and render at 100 ms in browsers; everything else is taken
/// literally, which also enforces the conventional 20 ms floor.
fn normalize_gif_delay(raw_10ms: u16) -> Duration {
    match raw_10ms {
        0 | 1 => Duration::from_millis(100),
        raw => Duration::from_millis(u64::from(raw) * 10),
    }
}

fn gif_decoder(
    bytes: Vec<u8>,
    skip_frame_decoding: bool,
) -> Result<gif::Decoder<Cursor<Vec<u8>>>, String> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::Indexed);
    options.set_memory_limit(gif::MemoryLimit::Bytes(
        NonZeroU64::new(GIF_FRAME_MEMORY_LIMIT_BYTES).expect("limit is nonzero"),
    ));
    options.skip_frame_decoding(skip_frame_decoding);
    options
        .read_info(Cursor::new(bytes))
        .map_err(|err| err.to_string())
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // informational fields; read by tests and future toast detail
pub(crate) struct AnimationProbe {
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Frames seen before returning; a lower bound when a budget tripped.
    pub(crate) frame_count: usize,
    pub(crate) total_decoded_pixels: u64,
    pub(crate) animated: bool,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // informational fields; read by tests and future toast detail
pub(crate) enum AnimationLimit {
    TooManyFrames { limit: usize },
    TooManyDecodedPixels { limit: u64 },
}

/// Whether a GIF is worth animating, shared by paste validation and playback.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // callers match on variants; fields feed tests and future toast detail
pub(crate) enum AnimationVerdict {
    Animate(AnimationProbe),
    StaticFallback {
        probe: AnimationProbe,
        reason: AnimationLimit,
    },
}

/// Full validation for untrusted paste payloads: every frame's LZW stream is
/// decoded (pixels discarded). Budget checks bail early, so an over-budget
/// GIF only pays for the frames under the cap.
pub(crate) fn gif_animation_verdict(bytes: &[u8]) -> Result<AnimationVerdict, String> {
    gif_verdict_impl(bytes, true)
}

/// Metadata-only scan (frame headers, no LZW decode). Cheap enough for the
/// main thread when only the animate-vs-static verdict is needed for bytes
/// that already passed full validation at paste time.
pub(crate) fn gif_animation_metadata_verdict(bytes: &[u8]) -> Result<AnimationVerdict, String> {
    gif_verdict_impl(bytes, false)
}

fn gif_verdict_impl(bytes: &[u8], decode_pixel_data: bool) -> Result<AnimationVerdict, String> {
    let mut decoder = gif_decoder(bytes.to_vec(), !decode_pixel_data)?;
    let width = u32::from(decoder.width());
    let height = u32::from(decoder.height());
    if width == 0 || height == 0 {
        return Err("GIF has zero dimensions".to_string());
    }
    let screen_pixels = u64::from(width) * u64::from(height);
    let has_global_palette = decoder.global_palette().is_some();
    let mut frame_count = 0usize;
    loop {
        let frame = if decode_pixel_data {
            decoder.read_next_frame().map_err(|err| err.to_string())?
        } else {
            decoder.next_frame_info().map_err(|err| err.to_string())?
        };
        let Some(frame) = frame else {
            break;
        };
        if frame.palette.is_none() && !has_global_palette {
            return Err("GIF frame has no color palette".to_string());
        }
        frame_count += 1;
        let total_decoded_pixels = screen_pixels.saturating_mul(frame_count as u64);
        let probe = AnimationProbe {
            width,
            height,
            frame_count,
            total_decoded_pixels,
            animated: frame_count > 1,
        };
        if frame_count > MAX_ANIMATION_FRAMES {
            return Ok(AnimationVerdict::StaticFallback {
                probe,
                reason: AnimationLimit::TooManyFrames {
                    limit: MAX_ANIMATION_FRAMES,
                },
            });
        }
        if total_decoded_pixels > MAX_ANIMATION_TOTAL_PIXELS {
            return Ok(AnimationVerdict::StaticFallback {
                probe,
                reason: AnimationLimit::TooManyDecodedPixels {
                    limit: MAX_ANIMATION_TOTAL_PIXELS,
                },
            });
        }
    }
    if frame_count == 0 {
        return Err("GIF contains no frames".to_string());
    }
    Ok(AnimationVerdict::Animate(AnimationProbe {
        width,
        height,
        frame_count,
        total_decoded_pixels: screen_pixels.saturating_mul(frame_count as u64),
        animated: frame_count > 1,
    }))
}

fn gif_dimensions(bytes: &[u8]) -> Result<(u32, u32), String> {
    if bytes.len() < 10 || !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return Err("GIF header is truncated or invalid".to_string());
    }
    let width = u32::from(u16::from_le_bytes([bytes[6], bytes[7]]));
    let height = u32::from(u16::from_le_bytes([bytes[8], bytes[9]]));
    Ok((width, height))
}

fn decode_gif_first_frame_rgba(bytes: &[u8]) -> Result<DecodedImage, String> {
    let mut decoder = GifStreamDecoder::new(bytes)?;
    let (width, height) = decoder.dimensions();
    let frame = decoder
        .next_frame()?
        .ok_or_else(|| "GIF contains no frames".to_string())?;
    Ok(DecodedImage {
        width,
        height,
        rgba: frame.rgba,
    })
}

fn pixel_count(width: u32, height: u32) -> Result<usize, String> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| "image dimensions are too large".to_string())?;
    usize::try_from(pixels).map_err(|_| "image dimensions are too large".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // Palette shared by all test GIFs: red, green, blue, white.
    const TEST_PALETTE: [u8; 12] = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
    const RED: u8 = 0;
    const BLUE: u8 = 2;

    struct TestGifFrame {
        pixels: Vec<u8>,
        width: u16,
        height: u16,
        left: u16,
        top: u16,
        delay: u16,
        dispose: gif::DisposalMethod,
        transparent: Option<u8>,
    }

    fn solid_frame(width: u16, height: u16, color_index: u8, delay: u16) -> TestGifFrame {
        TestGifFrame {
            pixels: vec![color_index; usize::from(width) * usize::from(height)],
            width,
            height,
            left: 0,
            top: 0,
            delay,
            dispose: gif::DisposalMethod::Keep,
            transparent: None,
        }
    }

    fn tiny_gif(
        width: u16,
        height: u16,
        frames: &[TestGifFrame],
        repeat: Option<gif::Repeat>,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut bytes, width, height, &TEST_PALETTE).unwrap();
            if let Some(repeat) = repeat {
                encoder.set_repeat(repeat).unwrap();
            }
            for frame in frames {
                let encoded = gif::Frame {
                    width: frame.width,
                    height: frame.height,
                    left: frame.left,
                    top: frame.top,
                    buffer: frame.pixels.clone().into(),
                    delay: frame.delay,
                    dispose: frame.dispose,
                    transparent: frame.transparent,
                    ..Default::default()
                };
                encoder.write_frame(&encoded).unwrap();
            }
        }
        bytes
    }

    #[test]
    fn guess_format_detects_gif_signatures() {
        assert_eq!(
            guess_format(b"GIF87a\x02\x00\x02\x00"),
            Some(EncodedImageFormat::Gif)
        );
        let bytes = tiny_gif(2, 2, &[solid_frame(2, 2, RED, 10)], None);
        assert_eq!(guess_format(&bytes), Some(EncodedImageFormat::Gif));
        assert_eq!(
            format_from_mime_or_bytes("image/gif", &[]),
            Some(EncodedImageFormat::Gif)
        );
    }

    #[test]
    fn gif_dimensions_read_the_logical_screen_descriptor() {
        let bytes = tiny_gif(3, 2, &[solid_frame(3, 2, RED, 10)], None);
        assert_eq!(
            image_dimensions(EncodedImageFormat::Gif, &bytes),
            Ok((3, 2))
        );
        assert!(image_dimensions(EncodedImageFormat::Gif, b"GIF89a").is_err());
    }

    #[test]
    fn decode_gif_first_frame_composites_rgba() {
        let bytes = tiny_gif(2, 2, &[solid_frame(2, 2, RED, 10)], None);
        let image = decode_rgba(EncodedImageFormat::Gif, &bytes).unwrap();
        assert_eq!((image.width, image.height), (2, 2));
        assert_eq!(image.rgba, [255, 0, 0, 255].repeat(4));
    }

    #[test]
    fn gif_delays_normalize_unspecified_values_to_100ms() {
        let frames = [
            solid_frame(2, 2, RED, 0),
            solid_frame(2, 2, BLUE, 1),
            solid_frame(2, 2, RED, 7),
        ];
        let bytes = tiny_gif(2, 2, &frames, None);
        let mut decoder = GifStreamDecoder::new(&bytes).unwrap();
        let delays: Vec<_> = std::iter::from_fn(|| decoder.next_frame().unwrap())
            .map(|frame| frame.delay)
            .collect();
        assert_eq!(
            delays,
            [
                Duration::from_millis(100),
                Duration::from_millis(100),
                Duration::from_millis(70),
            ]
        );
    }

    #[test]
    fn gif_disposal_keep_composites_partial_frames() {
        let full_red = solid_frame(2, 2, RED, 10);
        let corner_blue = TestGifFrame {
            pixels: vec![BLUE],
            width: 1,
            height: 1,
            left: 1,
            top: 1,
            delay: 10,
            dispose: gif::DisposalMethod::Keep,
            transparent: None,
        };
        let bytes = tiny_gif(2, 2, &[full_red, corner_blue], None);
        let mut decoder = GifStreamDecoder::new(&bytes).unwrap();
        decoder.next_frame().unwrap().unwrap();
        let second = decoder.next_frame().unwrap().unwrap();
        let red = [255, 0, 0, 255];
        let blue = [0, 0, 255, 255];
        assert_eq!(second.rgba, [red, red, red, blue].concat());
    }

    #[test]
    fn gif_disposal_background_clears_the_frame_region() {
        let mut full_red = solid_frame(2, 2, RED, 10);
        full_red.dispose = gif::DisposalMethod::Background;
        let corner_blue = TestGifFrame {
            pixels: vec![BLUE],
            width: 1,
            height: 1,
            left: 0,
            top: 0,
            delay: 10,
            dispose: gif::DisposalMethod::Keep,
            transparent: None,
        };
        let bytes = tiny_gif(2, 2, &[full_red, corner_blue], None);
        let mut decoder = GifStreamDecoder::new(&bytes).unwrap();
        decoder.next_frame().unwrap().unwrap();
        let second = decoder.next_frame().unwrap().unwrap();
        let blue = [0, 0, 255, 255];
        let clear = [0, 0, 0, 0];
        assert_eq!(second.rgba, [blue, clear, clear, clear].concat());
    }

    #[test]
    fn gif_loop_count_maps_netscape_semantics() {
        let frames = || [solid_frame(2, 2, RED, 10), solid_frame(2, 2, BLUE, 10)];
        // Finite(n) means n repetitions after the first playthrough (browser
        // semantics), so the total playthrough count is n + 1.
        let cases = [
            (None, Some(1)),
            (Some(gif::Repeat::Infinite), None),
            (Some(gif::Repeat::Finite(3)), Some(4)),
        ];
        for (repeat, expected) in cases {
            let bytes = tiny_gif(2, 2, &frames(), repeat);
            let mut decoder = GifStreamDecoder::new(&bytes).unwrap();
            decoder.next_frame().unwrap().unwrap();
            assert_eq!(decoder.loop_count(), expected, "repeat case {repeat:?}");
        }
    }

    #[test]
    fn gif_animation_verdict_accepts_gifs_within_budget() {
        let bytes = tiny_gif(
            2,
            2,
            &[solid_frame(2, 2, RED, 10), solid_frame(2, 2, BLUE, 10)],
            None,
        );
        for verdict in [
            gif_animation_verdict(&bytes).unwrap(),
            gif_animation_metadata_verdict(&bytes).unwrap(),
        ] {
            let AnimationVerdict::Animate(probe) = verdict else {
                panic!("expected Animate, got {verdict:?}");
            };
            assert!(probe.animated);
            assert_eq!(probe.frame_count, 2);
            assert_eq!(probe.total_decoded_pixels, 8);
        }

        let single = tiny_gif(2, 2, &[solid_frame(2, 2, RED, 10)], None);
        let AnimationVerdict::Animate(probe) = gif_animation_verdict(&single).unwrap() else {
            panic!("expected Animate for single-frame GIF");
        };
        assert!(!probe.animated);
    }

    #[test]
    fn gif_animation_verdict_trips_the_frame_budget() {
        let frames: Vec<_> = (0..=MAX_ANIMATION_FRAMES)
            .map(|_| solid_frame(1, 1, RED, 10))
            .collect();
        let bytes = tiny_gif(1, 1, &frames, None);
        for verdict in [
            gif_animation_verdict(&bytes).unwrap(),
            gif_animation_metadata_verdict(&bytes).unwrap(),
        ] {
            let AnimationVerdict::StaticFallback { probe, reason } = verdict else {
                panic!("expected StaticFallback, got {verdict:?}");
            };
            assert_eq!(probe.frame_count, MAX_ANIMATION_FRAMES + 1);
            assert!(matches!(reason, AnimationLimit::TooManyFrames { .. }));
        }
    }

    #[test]
    fn gif_animation_verdict_trips_the_pixel_budget() {
        // Two 4100x4100 frames total ~33.6 Mpx, just over the 32 Mpx cap.
        let side = 4100u16;
        let frames = [
            solid_frame(side, side, RED, 10),
            solid_frame(side, side, BLUE, 10),
        ];
        let bytes = tiny_gif(side, side, &frames, None);
        for verdict in [
            gif_animation_verdict(&bytes).unwrap(),
            gif_animation_metadata_verdict(&bytes).unwrap(),
        ] {
            let AnimationVerdict::StaticFallback { probe, reason } = verdict else {
                panic!("expected StaticFallback, got {verdict:?}");
            };
            assert_eq!(probe.frame_count, 2);
            assert!(matches!(
                reason,
                AnimationLimit::TooManyDecodedPixels { .. }
            ));
        }
    }

    #[test]
    fn gif_animation_verdict_rejects_malformed_payloads() {
        let bytes = tiny_gif(2, 2, &[solid_frame(2, 2, RED, 10)], None);
        assert!(gif_animation_verdict(&bytes[..20]).is_err());

        // Structurally valid GIF with no frames at all. The decoder keeps one
        // byte of lookahead past the header, so the trailer is doubled.
        let mut empty = b"GIF89a".to_vec();
        empty.extend_from_slice(&[2, 0, 2, 0, 0x80, 0, 0]);
        empty.extend_from_slice(&[255, 0, 0, 0, 0, 255]);
        empty.extend_from_slice(&[0x3B, 0x3B]);
        assert_eq!(
            gif_animation_verdict(&empty).unwrap_err(),
            "GIF contains no frames"
        );

        // Zero-dimension logical screen.
        let mut zero = b"GIF89a".to_vec();
        zero.extend_from_slice(&[0, 0, 0, 0, 0x80, 0, 0]);
        zero.extend_from_slice(&[255, 0, 0, 0, 0, 255]);
        zero.extend_from_slice(&[0x3B, 0x3B]);
        assert_eq!(
            gif_animation_verdict(&zero).unwrap_err(),
            "GIF has zero dimensions"
        );
        assert_eq!(
            GifStreamDecoder::new(&zero).err(),
            Some("GIF has zero dimensions".to_string())
        );
    }
}
