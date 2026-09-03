use super::*;
use crate::draw::FontDescriptor;

fn create_test_input_state() -> InputState {
    crate::input::state::test_support::TestInputStateBuilder::default()
        .thickness(3.0)
        .eraser_size(12.0)
        .font_descriptor(FontDescriptor {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        })
        .custom_section_enabled(false)
        .build()
}

#[test]
fn sample_eraser_path_points_densifies_long_segments() {
    let state = create_test_input_state();
    let points = vec![(0, 0), (20, 0)];
    let sampled = state.sample_eraser_path_points(&points);

    assert!(sampled.len() > points.len());
    assert_eq!(sampled.first().copied(), Some((0, 0)));
    assert_eq!(sampled.last().copied(), Some((20, 0)));
}

#[test]
fn sample_eraser_path_points_returns_borrowed_for_single_point() {
    let state = create_test_input_state();
    let points = vec![(5, 7)];
    let sampled = state.sample_eraser_path_points(&points);

    assert!(matches!(sampled, std::borrow::Cow::Borrowed(_)));
    assert_eq!(sampled.as_ref(), points.as_slice());
}

#[test]
fn sample_eraser_path_points_returns_borrowed_for_dense_segments() {
    let state = create_test_input_state();
    let points = vec![(0, 0), (2, 0), (4, 0)];
    let sampled = state.sample_eraser_path_points(&points);

    assert!(matches!(sampled, std::borrow::Cow::Borrowed(_)));
    assert_eq!(sampled.as_ref(), points.as_slice());
}

#[test]
fn sample_eraser_path_points_avoids_duplicate_points_when_sampling() {
    let state = create_test_input_state();
    let points = vec![(0, 0), (0, 20), (0, 20), (0, 40)];
    let sampled = state.sample_eraser_path_points(&points);

    assert!(matches!(sampled, std::borrow::Cow::Owned(_)));
    for pair in sampled.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
    assert_eq!(sampled.first().copied(), Some((0, 0)));
    assert_eq!(sampled.last().copied(), Some((0, 40)));
}
