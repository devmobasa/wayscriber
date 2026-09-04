use crate::capture::types::CaptureError;

/// Decode a PNG buffer into raw ARGB8888 bytes.
#[allow(dead_code)]
pub fn decode_image_to_argb(data: &[u8]) -> Result<(Vec<u8>, u32, u32), CaptureError> {
    let decoder = png::Decoder::new(std::io::Cursor::new(data));
    let mut reader = decoder
        .read_info()
        .map_err(|e| CaptureError::ImageError(format!("Decode error: {}", e)))?;

    let output_size = reader
        .output_buffer_size()
        .ok_or_else(|| CaptureError::ImageError("PNG output buffer too large".to_string()))?;
    let mut buf = vec![0; output_size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| CaptureError::ImageError(format!("Decode error: {}", e)))?;

    let mut argb = Vec::with_capacity((info.width * info.height * 4) as usize);
    let iter: Box<dyn Iterator<Item = (u8, u8, u8, u8)>> = match info.color_type {
        png::ColorType::Rgba => Box::new(buf[..info.buffer_size()].as_chunks::<4>().0.iter().map(
            |c| {
                let [r, g, b, a] = *c;
                (r, g, b, a)
            },
        )),
        png::ColorType::Rgb => Box::new(buf[..info.buffer_size()].as_chunks::<3>().0.iter().map(
            |c| {
                let [r, g, b] = *c;
                (r, g, b, 0xFF)
            },
        )),
        other => {
            return Err(CaptureError::ImageError(format!(
                "Unsupported PNG color type: {:?}",
                other
            )));
        }
    };

    for (r, g, b, a) in iter {
        let a_f = a as f32 / 255.0;
        let pr = (r as f32 * a_f).round() as u8;
        let pg = (g as f32 * a_f).round() as u8;
        let pb = (b as f32 * a_f).round() as u8;
        // Cairo ARgb32 uses native-endian BGRA layout
        argb.push(pb);
        argb.push(pg);
        argb.push(pr);
        argb.push(a);
    }

    Ok((argb, info.width, info.height))
}
