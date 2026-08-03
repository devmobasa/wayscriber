fn clamp01(value: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn parse_component(value: &str, fallback: f64) -> f64 {
    value.trim().parse::<f64>().map(clamp01).unwrap_or(fallback)
}

pub fn parse_triplet_values(values: &[String; 3]) -> [f64; 3] {
    [
        parse_component(&values[0], 0.0),
        parse_component(&values[1], 0.0),
        parse_component(&values[2], 0.0),
    ]
}

pub fn parse_quad_values(values: &[String; 4]) -> [f64; 4] {
    [
        parse_component(&values[0], 0.0),
        parse_component(&values[1], 0.0),
        parse_component(&values[2], 0.0),
        parse_component(&values[3], 1.0),
    ]
}

fn to_u8(value: f64) -> u8 {
    let clamped = clamp01(value);
    (clamped * 255.0).round().clamp(0.0, 255.0) as u8
}

fn from_u8(value: u8) -> f64 {
    f64::from(value) / 255.0
}

pub fn hex_from_rgb(rgb: [f64; 3]) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        to_u8(rgb[0]),
        to_u8(rgb[1]),
        to_u8(rgb[2])
    )
}

pub fn hex_from_rgba(rgba: [f64; 4]) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        to_u8(rgba[0]),
        to_u8(rgba[1]),
        to_u8(rgba[2]),
        to_u8(rgba[3])
    )
}

pub fn parse_hex(value: &str) -> Option<([f64; 3], Option<f64>)> {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    let bytes = hex.as_bytes();
    if bytes.len() != 6 && bytes.len() != 8 {
        return None;
    }

    let mut out = [0u8; 4];
    let count = bytes.len() / 2;
    for (index, slot) in out.iter_mut().enumerate().take(count) {
        let start = index * 2;
        let chunk = &hex[start..start + 2];
        match u8::from_str_radix(chunk, 16) {
            Ok(parsed) => *slot = parsed,
            Err(_) => return None,
        }
    }

    let rgb = [from_u8(out[0]), from_u8(out[1]), from_u8(out[2])];
    let alpha = if bytes.len() == 8 {
        Some(from_u8(out[3]))
    } else {
        None
    };

    Some((rgb, alpha))
}
