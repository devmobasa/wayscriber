pub(super) const EPS: f64 = 1e-6;

pub(super) fn distance_point_to_segment(
    point: (i32, i32),
    start: (i32, i32),
    end: (i32, i32),
) -> f64 {
    let (px, py) = (point.0 as f64, point.1 as f64);
    let (x1, y1) = (start.0 as f64, start.1 as f64);
    let (x2, y2) = (end.0 as f64, end.1 as f64);
    let vx = x2 - x1;
    let vy = y2 - y1;
    let len_sq = vx * vx + vy * vy;
    if len_sq.abs() < EPS {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }
    let t = ((px - x1) * vx + (py - y1) * vy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj_x = x1 + t * vx;
    let proj_y = y1 + t * vy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

pub(super) fn distance_point_to_point(a: (i32, i32), b: (i32, i32)) -> f64 {
    let dx = (a.0 - b.0) as f64;
    let dy = (a.1 - b.1) as f64;
    (dx * dx + dy * dy).sqrt()
}

pub(super) fn point_in_triangle(
    p: (f64, f64),
    a: (f64, f64),
    b: (f64, f64),
    c: (f64, f64),
) -> bool {
    let (px, py) = p;
    let (ax, ay) = a;
    let (bx, by) = b;
    let (cx, cy) = c;
    let v0 = (cx - ax, cy - ay);
    let v1 = (bx - ax, by - ay);
    let v2 = (px - ax, py - ay);

    let dot00 = v0.0 * v0.0 + v0.1 * v0.1;
    let dot01 = v0.0 * v1.0 + v0.1 * v1.1;
    let dot02 = v0.0 * v2.0 + v0.1 * v2.1;
    let dot11 = v1.0 * v1.0 + v1.1 * v1.1;
    let dot12 = v1.0 * v2.0 + v1.1 * v2.1;

    let denom = dot00 * dot11 - dot01 * dot01;
    if denom.abs() < EPS {
        return false;
    }
    let inv_denom = 1.0 / denom;
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

    u >= -EPS && v >= -EPS && (u + v) <= 1.0 + EPS
}

pub(super) fn to_i32_pair(p: (f64, f64)) -> (i32, i32) {
    (p.0.round() as i32, p.1.round() as i32)
}

pub(super) fn p_as_i32(p: (f64, f64)) -> (i32, i32) {
    (p.0.round() as i32, p.1.round() as i32)
}
