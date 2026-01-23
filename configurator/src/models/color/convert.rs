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

pub fn rgb_to_hsv(rgb: [f64; 3]) -> (f64, f64, f64) {
    let r = clamp01(rgb[0]);
    let g = clamp01(rgb[1]);
    let b = clamp01(rgb[2]);

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };

    let s = if max == 0.0 { 0.0 } else { delta / max };
    let v = max;

    (clamp01(h), clamp01(s), clamp01(v))
}

pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> [f64; 3] {
    let h = clamp01(h);
    let s = clamp01(s);
    let v = clamp01(v);

    if s == 0.0 {
        return [v, v, v];
    }

    let h6 = h * 6.0;
    let i = h6.floor();
    let f = h6 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    match (i as i32).rem_euclid(6) {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
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
