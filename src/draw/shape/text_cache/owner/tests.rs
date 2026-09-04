mod bounds;

use super::*;
use crate::draw::shape::text_cache::{VisualCaretDirection, VisualLineDirection, VisualLineEdge};

fn metrics(value: &TextMeasurement) -> [f64; 9] {
    [
        value.ink_x,
        value.ink_y,
        value.ink_width,
        value.ink_height,
        value.logical_x,
        value.logical_y,
        value.logical_width,
        value.logical_height,
        value.baseline,
    ]
}

#[test]
fn construction_and_empty_measurement_leave_context_lazy() {
    let owner = TextMeasurer::default();
    assert!(owner.context.borrow().is_none());
    assert!(owner.cache.borrow().entries.is_empty());
    assert!(owner.measure("", "Sans 12", 12.0, None).is_none());
    assert_eq!(owner.hit_test_text("", "Sans 12", None, 0.0, 0.0), Some(0));
    assert!(owner.context.borrow().is_none());
    assert!(
        owner
            .caret_geometry_text("", "Sans 12", None, 0)
            .unwrap()
            .height
            > 0.0
    );
    assert!(owner.context.borrow().is_some());
}

#[test]
fn independent_owners_retain_distinct_contexts_and_numeric_entries() {
    let first = TextMeasurer::default();
    let second = TextMeasurer::default();
    let expected = first.measure("shared", "Sans 12", 12.0, None).unwrap();
    assert!(second.context.borrow().is_none());
    let actual = second.measure("shared", "Sans 12", 12.0, None).unwrap();
    assert_eq!(metrics(&expected), metrics(&actual));
    let first_ctx = first.context.borrow().as_ref().unwrap().clone();
    let second_ctx = second.context.borrow().as_ref().unwrap().clone();
    assert_ne!(first_ctx.to_raw_none(), second_ctx.to_raw_none());
    first.measure("first only", "Sans 12", 12.0, None).unwrap();
    assert_eq!(first.cache.borrow().entries.len(), 2);
    assert_eq!(second.cache.borrow().entries.len(), 1);
    drop(first);
    assert_eq!(
        metrics(&second.measure("shared", "Sans 12", 12.0, None).unwrap()),
        metrics(&expected)
    );
}

#[test]
fn hits_reuse_numeric_results_and_promote_within_the_default_budget() {
    let owner = TextMeasurer::default();
    for index in 0..256 {
        owner
            .measure(&format!("entry {index}"), "Sans 12", 12.0, None)
            .unwrap();
    }
    let first_key = TextCacheKey::new("entry 0", "Sans 12", 12.0, None);
    // A recognizable cached value proves the owner hit path avoids recomputing.
    owner
        .cache
        .borrow_mut()
        .entries
        .get_mut(&first_key)
        .unwrap()
        .baseline = -123.0;
    assert_eq!(
        owner
            .measure("entry 0", "Sans 12", 12.0, None)
            .unwrap()
            .baseline,
        -123.0
    );
    owner.measure("new", "Sans 12", 12.0, None).unwrap();
    let cache = owner.cache.borrow();
    assert_eq!(cache.entries.len(), 256);
    assert!(cache.entries.contains_key(&first_key));
    assert!(
        !cache
            .entries
            .contains_key(&TextCacheKey::new("entry 1", "Sans 12", 12.0, None))
    );
}

#[test]
fn destination_settings_do_not_change_canonical_measurements() {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 8, 8).unwrap();
    let destination = cairo::Context::new(&surface).unwrap();
    destination.scale(2.0, 3.0);
    destination.set_antialias(cairo::Antialias::None);
    let mut options = cairo::FontOptions::new().unwrap();
    options.set_hint_metrics(cairo::HintMetrics::Off);
    destination.set_font_options(&options);
    let owner = TextMeasurer::default();
    let fresh = TextMeasurer::default();
    let actual = crate::draw::shape::text_cache::measure_text_with_context(
        &destination,
        "Hello 你好\nאבג",
        "Sans 16",
        16.0,
        Some(70),
    )
    .unwrap();
    let expected = fresh
        .measure("Hello 你好\nאבג", "Sans 16", 16.0, Some(70))
        .unwrap();
    assert_eq!(metrics(&actual), metrics(&expected));
    owner
        .with_measurement_context(|ctx| {
            assert_eq!(ctx.antialias(), cairo::Antialias::Best);
            assert_eq!(ctx.matrix(), cairo::Matrix::identity());
            let target = cairo::ImageSurface::try_from(ctx.target()).unwrap();
            assert_eq!((target.width(), target.height()), (1, 1));
        })
        .unwrap();
}

#[test]
fn initialization_borrow_is_released_before_nested_measurement() {
    let owner = TextMeasurer::default();
    owner
        .with_measurement_context(|outer| {
            let measured = owner.measure("nested miss", "Sans 12", 12.0, None).unwrap();
            assert!(measured.logical_width > 0.0);
            owner
                .with_measurement_context(|inner| {
                    assert_eq!(outer.to_raw_none(), inner.to_raw_none())
                })
                .unwrap();
        })
        .unwrap();
}

#[test]
fn explicit_cursor_geometry_preserves_wrap_bidi_and_utf8_boundaries() {
    let owner = TextMeasurer::default();
    let text = "abc אבג 你好\nsecond line";
    let font = "Sans 18";
    for byte in 0..=text.len() {
        let geometry = owner
            .caret_geometry_text(text, font, Some(80), byte)
            .unwrap();
        assert!(geometry.height > 0.0);
        let preview = owner
            .text_preview_geometry(text, font, Some(80), Some(byte))
            .unwrap();
        assert_eq!(preview.caret, Some(geometry));
        for direction in [VisualCaretDirection::Left, VisualCaretDirection::Right] {
            let next = owner
                .caret_on_adjacent_visual_position(text, font, Some(80), byte, direction)
                .unwrap();
            assert!(text.is_char_boundary(next));
        }
        for direction in [VisualLineDirection::Up, VisualLineDirection::Down] {
            let next = owner
                .caret_on_adjacent_visual_line(text, font, Some(80), byte, direction)
                .unwrap();
            assert!(text.is_char_boundary(next));
        }
        for edge in [VisualLineEdge::Start, VisualLineEdge::End] {
            let next = owner
                .caret_on_visual_line_edge(text, font, Some(80), byte, edge)
                .unwrap();
            assert!(text.is_char_boundary(next));
        }
    }
    let rtl = "אבג";
    let start = owner.caret_geometry_text(rtl, font, None, 0).unwrap();
    let end = owner
        .caret_geometry_text(rtl, font, None, rtl.len())
        .unwrap();
    assert!(start.x > end.x);
    // Interior boundaries in an RTL run have the opposite visual order to
    // their byte order. Avoid equating the terminal index's line position
    // with Pango's strong cursor rectangle at the paragraph boundary.
    let mixed = "abc אבג xyz";
    assert_eq!(
        owner.caret_at_visual_selection_edge(mixed, font, None, 6, 8, VisualCaretDirection::Left),
        Some(8)
    );
    assert_eq!(
        owner.caret_at_visual_selection_edge(mixed, font, None, 6, 8, VisualCaretDirection::Right),
        Some(6)
    );
}
