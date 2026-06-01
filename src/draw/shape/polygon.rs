use crate::util::Rect;
use serde::{Deserialize, Serialize};

pub const REGULAR_POLYGON_MIN_SIDES: u8 = 3;
pub const REGULAR_POLYGON_MAX_SIDES: u8 = 12;
pub const REGULAR_POLYGON_DEFAULT_SIDES: u8 = 5;

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum PolygonKind {
    Triangle,
    Parallelogram,
    Rhombus,
    Regular { sides: u8 },
    Freeform,
}

impl PolygonKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Triangle => "Triangle",
            Self::Parallelogram => "Parallelogram",
            Self::Rhombus => "Rhombus",
            Self::Regular { .. } => "Regular Polygon",
            Self::Freeform => "Freeform Polygon",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolygonTemplate {
    Triangle,
    Parallelogram,
    Rhombus,
    Regular,
}

impl PolygonTemplate {
    pub(crate) fn kind(self, regular_sides: u8) -> PolygonKind {
        match self {
            Self::Triangle => PolygonKind::Triangle,
            Self::Parallelogram => PolygonKind::Parallelogram,
            Self::Rhombus => PolygonKind::Rhombus,
            Self::Regular => PolygonKind::Regular {
                sides: clamp_regular_sides(regular_sides),
            },
        }
    }
}

pub fn clamp_regular_sides(sides: u8) -> u8 {
    sides.clamp(REGULAR_POLYGON_MIN_SIDES, REGULAR_POLYGON_MAX_SIDES)
}

pub(crate) fn generated_points(
    template: PolygonTemplate,
    start: (i32, i32),
    end: (i32, i32),
    regular_sides: u8,
) -> Vec<(i32, i32)> {
    match template {
        PolygonTemplate::Triangle => triangle_points(start, end),
        PolygonTemplate::Parallelogram => parallelogram_points(start, end),
        PolygonTemplate::Rhombus => rhombus_points(start, end),
        PolygonTemplate::Regular => regular_polygon_points(start, end, regular_sides),
    }
}

pub fn has_minimum_distinct_points(points: &[(i32, i32)]) -> bool {
    if points.len() < 3 {
        return false;
    }

    let mut distinct = Vec::with_capacity(3);
    for point in points {
        if !distinct.contains(point) {
            distinct.push(*point);
            if distinct.len() >= 3 {
                return true;
            }
        }
    }
    false
}

pub(crate) fn bounding_box_for_polygon(points: &[(i32, i32)], thick: f64) -> Option<Rect> {
    if !has_minimum_distinct_points(points) {
        return None;
    }

    let mut min_x = points[0].0;
    let mut min_y = points[0].1;
    let mut max_x = points[0].0;
    let mut max_y = points[0].1;
    for &(x, y) in &points[1..] {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }

    let pad = ((thick / 2.0).ceil() as i32).max(1);
    Rect::from_min_max(min_x - pad, min_y - pad, max_x + pad, max_y + pad)
}

fn triangle_points(start: (i32, i32), end: (i32, i32)) -> Vec<(i32, i32)> {
    let (min_x, max_x) = sorted_pair(start.0, end.0);
    let (min_y, max_y) = sorted_pair(start.1, end.1);
    let mid_x = midpoint_i32(min_x, max_x);

    if end.1 >= start.1 {
        vec![(mid_x, min_y), (max_x, max_y), (min_x, max_y)]
    } else {
        vec![(mid_x, max_y), (min_x, min_y), (max_x, min_y)]
    }
}

fn parallelogram_points(start: (i32, i32), end: (i32, i32)) -> Vec<(i32, i32)> {
    let (min_x, max_x) = sorted_pair(start.0, end.0);
    let (min_y, max_y) = sorted_pair(start.1, end.1);
    let width = max_x - min_x;
    let skew = (width.abs() / 4).max(1);

    if end.0 >= start.0 {
        vec![
            (min_x + skew, min_y),
            (max_x, min_y),
            (max_x - skew, max_y),
            (min_x, max_y),
        ]
    } else {
        vec![
            (min_x, min_y),
            (max_x - skew, min_y),
            (max_x, max_y),
            (min_x + skew, max_y),
        ]
    }
}

fn rhombus_points(start: (i32, i32), end: (i32, i32)) -> Vec<(i32, i32)> {
    let (min_x, max_x) = sorted_pair(start.0, end.0);
    let (min_y, max_y) = sorted_pair(start.1, end.1);
    let mid_x = midpoint_i32(min_x, max_x);
    let mid_y = midpoint_i32(min_y, max_y);
    vec![
        (mid_x, min_y),
        (max_x, mid_y),
        (mid_x, max_y),
        (min_x, mid_y),
    ]
}

fn regular_polygon_points(start: (i32, i32), end: (i32, i32), sides: u8) -> Vec<(i32, i32)> {
    let sides = clamp_regular_sides(sides);
    let center_x = (start.0 as f64 + end.0 as f64) / 2.0;
    let center_y = (start.1 as f64 + end.1 as f64) / 2.0;
    let radius_x = (end.0 - start.0).abs() as f64 / 2.0;
    let radius_y = (end.1 - start.1).abs() as f64 / 2.0;
    let radius = radius_x.min(radius_y);
    let start_angle = -std::f64::consts::FRAC_PI_2;

    (0..sides)
        .map(|index| {
            let angle = start_angle + std::f64::consts::TAU * f64::from(index) / f64::from(sides);
            (
                (center_x + angle.cos() * radius).round() as i32,
                (center_y + angle.sin() * radius).round() as i32,
            )
        })
        .collect()
}

const fn sorted_pair(a: i32, b: i32) -> (i32, i32) {
    if a <= b { (a, b) } else { (b, a) }
}

fn midpoint_i32(a: i32, b: i32) -> i32 {
    ((i64::from(a) + i64::from(b)) / 2) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_kind_uses_explicit_tagged_serialization() {
        let json = serde_json::to_string(&PolygonKind::Triangle).unwrap();
        assert_eq!(json, r#"{"type":"triangle"}"#);
    }

    #[test]
    fn regular_polygon_kind_serializes_sides() {
        let json = serde_json::to_string(&PolygonKind::Regular { sides: 6 }).unwrap();
        assert_eq!(json, r#"{"type":"regular","sides":6}"#);
    }

    #[test]
    fn validity_requires_three_distinct_points() {
        assert!(!has_minimum_distinct_points(&[(1, 1), (1, 1), (2, 2)]));
        assert!(has_minimum_distinct_points(&[
            (1, 1),
            (1, 1),
            (2, 2),
            (3, 3)
        ]));
    }

    #[test]
    fn regular_sides_clamp_to_supported_range() {
        assert_eq!(clamp_regular_sides(2), 3);
        assert_eq!(clamp_regular_sides(9), 9);
        assert_eq!(clamp_regular_sides(80), 12);
    }
}
