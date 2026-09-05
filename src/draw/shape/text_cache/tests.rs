use super::*;

fn measurement(width: f64) -> TextMeasurement {
    TextMeasurement {
        ink_x: 0.0,
        ink_y: 0.0,
        ink_width: width,
        ink_height: 10.0,
        logical_x: 0.0,
        logical_y: 0.0,
        logical_width: width,
        logical_height: 10.0,
        baseline: 8.0,
    }
}

#[test]
fn hit_test_maps_x_extremes_to_buffer_ends() {
    let measurer = TextMeasurer::default();
    // Far-left click lands at the start; far-right at the end; the exact
    // glyph widths do not matter, only the ordering and clamping.
    assert_eq!(
        measurer.hit_test_text("hello", "Sans 20", None, -100.0, 0.0),
        Some(0)
    );
    assert_eq!(
        measurer.hit_test_text("hello", "Sans 20", None, 100_000.0, 0.0),
        Some(5)
    );
    // Empty text always resolves to caret 0.
    assert_eq!(
        measurer.hit_test_text("", "Sans 20", None, 42.0, 0.0),
        Some(0)
    );
}

#[test]
fn hit_test_result_is_always_a_char_boundary() {
    let measurer = TextMeasurer::default();
    // '你好' is two 3-byte chars; any x must land on 0, 3, or 6.
    let text = "你好";
    for x in [-10.0, 0.0, 5.0, 12.0, 30.0, 1000.0] {
        let offset = measurer
            .hit_test_text(text, "Sans 20", None, x, 0.0)
            .unwrap();
        assert!(
            text.is_char_boundary(offset),
            "offset {offset} split a char"
        );
    }
}

#[test]
fn caret_geometry_advances_left_to_right_and_has_height() {
    let start = caret_geometry_text("hello", "Sans 20", None, 0).unwrap();
    let end = caret_geometry_text("hello", "Sans 20", None, 5).unwrap();
    assert!(start.x >= 0.0);
    assert!(
        end.x > start.x,
        "the end caret must sit to the right of the start caret"
    );
    assert!(start.height > 0.0, "the caret must have a visible height");
}

#[test]
fn caret_geometry_works_on_empty_text() {
    // An empty buffer still needs a visible caret at the origin.
    let geom = caret_geometry_text("", "Sans 20", None, 0).unwrap();
    assert_eq!(geom.x, 0.0);
    assert!(geom.height > 0.0);
}

#[test]
fn caret_geometry_snaps_off_boundary_indices_down() {
    // Byte 2 is inside the 3-byte '你'; it must resolve like byte 0, not panic.
    let at_zero = caret_geometry_text("你a", "Sans 20", None, 0).unwrap();
    let off_boundary = caret_geometry_text("你a", "Sans 20", None, 2).unwrap();
    assert_eq!(at_zero, off_boundary);
}

#[test]
fn test_cache_returns_same_measurement() {
    let measurer = crate::draw::TextMeasurer::default();
    let text = "Hello World";
    let font = "Sans 12";

    let m1 = measurer.measure(text, font, 12.0, None);
    let m2 = measurer.measure(text, font, 12.0, None);

    assert!(m1.is_some());
    assert!(m2.is_some());

    let m1 = m1.unwrap();
    let m2 = m2.unwrap();

    assert_eq!(m1.ink_width, m2.ink_width);
    assert_eq!(m1.ink_height, m2.ink_height);
    assert_eq!(m1.baseline, m2.baseline);
}

#[test]
fn test_different_sizes_use_different_cache_keys() {
    let measurer = crate::draw::TextMeasurer::default();
    // Verify that measurements for different sizes are cached with different keys
    // by checking that both requests succeed (cache doesn't confuse them)
    let text = "Test";
    let font = "Sans";

    let m1 = measurer.measure(text, font, 12.0, None);
    let m2 = measurer.measure(text, font, 24.0, None);

    assert!(m1.is_some(), "12pt measurement should succeed");
    assert!(m2.is_some(), "24pt measurement should succeed");

    // Request them again - should hit cache for both
    let m1_cached = measurer.measure(text, font, 12.0, None);
    let m2_cached = measurer.measure(text, font, 24.0, None);

    let m1 = m1.unwrap();
    let m1_cached = m1_cached.unwrap();

    // Verify cache returns consistent results for same parameters
    assert_eq!(m1.ink_width, m1_cached.ink_width);
    assert_eq!(m1.ink_height, m1_cached.ink_height);

    let m2 = m2.unwrap();
    let m2_cached = m2_cached.unwrap();

    assert_eq!(m2.ink_width, m2_cached.ink_width);
    assert_eq!(m2.ink_height, m2_cached.ink_height);
}

#[test]
fn test_cache_evicts_oldest_entry_at_capacity() {
    let mut cache = TextMeasurementCache::new(2);
    let key_a = TextCacheKey::new("A", "Sans", 12.0, None);
    let key_b = TextCacheKey::new("B", "Sans", 12.0, None);
    let key_c = TextCacheKey::new("C", "Sans", 12.0, None);

    cache.insert(key_a.clone(), measurement(10.0));
    cache.insert(key_b.clone(), measurement(20.0));
    cache.insert(key_c.clone(), measurement(30.0));

    assert!(cache.get(&key_a).is_none());
    assert_eq!(cache.get(&key_b).unwrap().ink_width, 20.0);
    assert_eq!(cache.get(&key_c).unwrap().ink_width, 30.0);
}

#[test]
fn test_get_refreshes_lru_order_before_eviction() {
    let mut cache = TextMeasurementCache::new(2);
    let key_a = TextCacheKey::new("A", "Sans", 12.0, None);
    let key_b = TextCacheKey::new("B", "Sans", 12.0, None);
    let key_c = TextCacheKey::new("C", "Sans", 12.0, None);

    cache.insert(key_a.clone(), measurement(10.0));
    cache.insert(key_b.clone(), measurement(20.0));
    assert_eq!(cache.get(&key_a).unwrap().ink_width, 10.0);
    cache.insert(key_c.clone(), measurement(30.0));

    assert!(cache.get(&key_b).is_none());
    assert_eq!(cache.get(&key_a).unwrap().ink_width, 10.0);
    assert_eq!(cache.get(&key_c).unwrap().ink_width, 30.0);
}

#[test]
fn test_insert_existing_key_updates_cached_measurement() {
    let mut cache = TextMeasurementCache::new(2);
    let key = TextCacheKey::new("A", "Sans", 12.0, None);

    cache.insert(key.clone(), measurement(10.0));
    cache.insert(key.clone(), measurement(42.0));

    assert_eq!(cache.get(&key).unwrap().ink_width, 42.0);
    assert_eq!(cache.entries.len(), 1);
}

#[test]
fn test_empty_text_returns_none() {
    let measurer = crate::draw::TextMeasurer::default();
    let result = measurer.measure("", "Sans 12", 12.0, None);
    assert!(result.is_none());
}

#[test]
fn test_wrap_width_affects_cache_key() {
    let measurer = crate::draw::TextMeasurer::default();
    let text = "A very long text that would wrap";
    let font = "Sans 12";

    let m1 = measurer.measure(text, font, 12.0, None);
    let m2 = measurer.measure(text, font, 12.0, Some(50));

    assert!(m1.is_some());
    assert!(m2.is_some());

    // With narrow wrap width, height should be larger (more lines)
    let m1 = m1.unwrap();
    let m2 = m2.unwrap();
    assert!(m2.ink_height >= m1.ink_height);
}
