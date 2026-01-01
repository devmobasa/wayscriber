#[cfg(feature = "tray")]
use log::warn;
#[cfg(feature = "tray")]
use png::Decoder;

#[cfg(feature = "tray")]
pub(crate) fn decode_tray_icon_png() -> Option<Vec<ksni::Icon>> {
    const ICON_BYTES: &[u8] = include_bytes!("../../assets/tray_icon.png");
    let decoder = Decoder::new(ICON_BYTES);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let bytes = &buf[..info.buffer_size()];
    let mut data = Vec::with_capacity(bytes.len());
    // ksni expects ARGB32 (network byte order). If the PNG is grayscale+alpha,
    // expand channels to ARGB with duplicated gray.
    match info.color_type {
        png::ColorType::Rgba => {
            for chunk in bytes.chunks_exact(4) {
                data.push(chunk[3]); // A
                data.push(chunk[0]); // R
                data.push(chunk[1]); // G
                data.push(chunk[2]); // B
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for chunk in bytes.chunks_exact(2) {
                let g = chunk[0];
                let a = chunk[1];
                data.push(a);
                data.push(g);
                data.push(g);
                data.push(g);
            }
        }
        png::ColorType::Grayscale => {
            for &g in bytes {
                data.push(255);
                data.push(g);
                data.push(g);
                data.push(g);
            }
        }
        png::ColorType::Rgb => {
            for chunk in bytes.chunks_exact(3) {
                data.push(255);
                data.push(chunk[0]);
                data.push(chunk[1]);
                data.push(chunk[2]);
            }
        }
        _ => {
            warn!("Unsupported tray icon color type; falling back to empty icon");
            return None;
        }
    }
    Some(vec![ksni::Icon {
        width: info.width as i32,
        height: info.height as i32,
        data,
    }])
}

#[cfg(feature = "tray")]
pub(crate) fn fallback_tray_icon() -> Vec<ksni::Icon> {
    let size = 22;
    let mut data = Vec::with_capacity(size * size * 4);

    for y in 0..size {
        for x in 0..size {
            let (a, r, g, b) = if (2..=4).contains(&x) && (2..=4).contains(&y) {
                (255, 60, 60, 60)
            } else if (3..=5).contains(&x) && (5..=7).contains(&y) {
                (255, 180, 120, 60)
            } else if (4..=8).contains(&x) && (6..=14).contains(&y) {
                (255, 255, 220, 0)
            } else if (7..=9).contains(&x) && (13..=17).contains(&y) {
                (255, 180, 180, 180)
            } else if (8..=11).contains(&x) && (16..=19).contains(&y) {
                (255, 255, 150, 180)
            } else {
                (0, 0, 0, 0)
            };

            data.push(a);
            data.push(r);
            data.push(g);
            data.push(b);
        }
    }

    vec![ksni::Icon {
        width: size as i32,
        height: size as i32,
        data,
    }]
}
