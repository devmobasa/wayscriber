use super::*;
use crate::draw::{BLACK, Color, RED, WHITE};

#[test]
fn arrowhead_triangle_caps_at_forty_percent_of_line_length() {
    // Line length = 10, requested head length = 100 -> capped at 40% = 4.
    let geometry = calculate_arrowhead_triangle_custom(10, 10, 0, 10, 1.0, 100.0, 30.0)
        .expect("non-degenerate line should yield geometry");
    let distance = ((geometry.tip.0 - geometry.base.0).powi(2)
        + (geometry.tip.1 - geometry.base.1).powi(2))
    .sqrt();
    assert!((distance - 4.0).abs() < f64::EPSILON);
}

#[test]
fn arrowhead_triangle_handles_degenerate_lines() {
    let geometry = calculate_arrowhead_triangle_custom(5, 5, 5, 5, 2.0, 15.0, 45.0);
    assert!(geometry.is_none());
}

#[test]
fn ellipse_bounds_compute_center_and_radii() {
    let (cx, cy, rx, ry) = ellipse_bounds(0, 0, 10, 4);
    assert_eq!((cx, cy, rx, ry), (5, 2, 5, 2));
}

#[test]
fn key_and_name_color_mappings_round_trip() {
    assert_eq!(key_to_color('r').unwrap(), RED);
    assert_eq!(key_to_color('K').unwrap(), BLACK);
    assert!(key_to_color('x').is_none());
    assert_eq!(name_to_color("white").unwrap(), WHITE);
    assert!(name_to_color("chartreuse").is_none());
}

#[test]
fn color_to_name_matches_known_colors() {
    assert_eq!(color_to_name(&RED), "Red");
    assert_eq!(color_to_name(&BLACK), "Black");
    assert_eq!(
        color_to_name(&Color {
            r: 0.42,
            g: 0.42,
            b: 0.42,
            a: 1.0
        }),
        "Custom"
    );
}

#[test]
fn rect_contains_is_min_inclusive_max_exclusive() {
    let rect = Rect::new(0, 0, 10, 10).unwrap();
    assert!(rect.contains(0, 0));
    assert!(rect.contains(9, 9));
    assert!(!rect.contains(10, 10));
    assert!(!rect.contains(-1, 0));
}

#[test]
fn rect_inflated_returns_none_when_degenerate() {
    let rect = Rect::new(0, 0, 2, 2).unwrap();
    assert!(rect.inflated(-2).is_none());
}
