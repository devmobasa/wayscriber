use super::*;
use crate::draw::{BLACK, Color, RED, WHITE};

#[test]
fn arrowhead_caps_at_thirty_percent_of_line_length() {
    let [(lx, ly), _] = calculate_arrowhead_custom(10, 10, 0, 10, 100.0, 30.0);
    let distance = ((10.0 - lx).powi(2) + (10.0 - ly).powi(2)).sqrt();
    assert!((distance - 3.0).abs() < f64::EPSILON);
}

#[test]
fn arrowhead_handles_degenerate_lines() {
    let [(lx, ly), (rx, ry)] = calculate_arrowhead_custom(5, 5, 5, 5, 15.0, 45.0);
    assert_eq!((lx, ly), (5.0, 5.0));
    assert_eq!((rx, ry), (5.0, 5.0));
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
