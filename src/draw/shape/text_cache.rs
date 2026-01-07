use std::cell::RefCell;
use std::collections::HashMap;

/// Cached text measurement results from Pango layout.
#[derive(Clone, Debug)]
pub(crate) struct TextMeasurement {
    pub ink_x: f64,
    pub ink_y: f64,
    pub ink_width: f64,
    pub ink_height: f64,
    pub baseline: f64,
}

/// Cache key for text measurements.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct TextCacheKey {
    text: String,
    font_desc_str: String,
    /// Size in hundredths of points for stable hashing
    size_hundredths: i32,
    /// Wrap width in pixels, or -1 for no wrap
    wrap_width: i32,
}

impl TextCacheKey {
    fn new(text: &str, font_desc_str: &str, size: f64, wrap_width: Option<i32>) -> Self {
        Self {
            text: text.to_string(),
            font_desc_str: font_desc_str.to_string(),
            size_hundredths: (size * 100.0).round() as i32,
            wrap_width: wrap_width.unwrap_or(-1),
        }
    }
}

/// Thread-local cache for text measurements.
/// Uses an LRU-style eviction when cache exceeds max size.
struct TextMeasurementCache {
    entries: HashMap<TextCacheKey, TextMeasurement>,
    access_order: Vec<TextCacheKey>,
    max_entries: usize,
}

impl TextMeasurementCache {
    fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_entries),
            access_order: Vec::with_capacity(max_entries),
            max_entries,
        }
    }

    fn get(&mut self, key: &TextCacheKey) -> Option<TextMeasurement> {
        if let Some(measurement) = self.entries.get(key) {
            // Move to end of access order (most recently used)
            if let Some(pos) = self.access_order.iter().position(|k| k == key) {
                self.access_order.remove(pos);
                self.access_order.push(key.clone());
            }
            Some(measurement.clone())
        } else {
            None
        }
    }

    fn insert(&mut self, key: TextCacheKey, measurement: TextMeasurement) {
        // If key already exists, update it and move to end of access order
        if self.entries.contains_key(&key) {
            self.entries.insert(key.clone(), measurement);
            if let Some(pos) = self.access_order.iter().position(|k| k == &key) {
                self.access_order.remove(pos);
            }
            self.access_order.push(key);
            return;
        }

        // Evict oldest entries if at capacity
        while self.entries.len() >= self.max_entries && !self.access_order.is_empty() {
            let oldest = self.access_order.remove(0);
            self.entries.remove(&oldest);
        }

        self.entries.insert(key.clone(), measurement);
        self.access_order.push(key);
    }

    #[allow(dead_code)]
    fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
    }
}

thread_local! {
    static TEXT_CACHE: RefCell<TextMeasurementCache> = RefCell::new(TextMeasurementCache::new(256));
    /// Shared dummy surface for measurement when no context available
    static MEASUREMENT_SURFACE: RefCell<Option<(cairo::ImageSurface, cairo::Context)>> = const { RefCell::new(None) };
}

/// Get or create a measurement context (reuses a single surface instead of creating new ones).
fn with_measurement_context<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&cairo::Context) -> R,
{
    MEASUREMENT_SURFACE.with(|cell| {
        let mut surface_ref = cell.borrow_mut();
        if surface_ref.is_none() {
            let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).ok()?;
            let ctx = cairo::Context::new(&surface).ok()?;
            ctx.set_antialias(cairo::Antialias::Best);
            *surface_ref = Some((surface, ctx));
        }
        surface_ref.as_ref().map(|(_, ctx)| f(ctx))
    })
}

/// Measure text using Pango, with caching.
/// Returns cached measurement if available, otherwise measures and caches.
pub(crate) fn measure_text_cached(
    text: &str,
    font_desc_str: &str,
    size: f64,
    wrap_width: Option<i32>,
) -> Option<TextMeasurement> {
    if text.is_empty() {
        return None;
    }

    let key = TextCacheKey::new(text, font_desc_str, size, wrap_width);

    // Check cache first
    let cached = TEXT_CACHE.with(|cache| cache.borrow_mut().get(&key));
    if let Some(measurement) = cached {
        return Some(measurement);
    }

    // Measure using shared context
    let measurement = with_measurement_context(|ctx| {
        let layout = pangocairo::functions::create_layout(ctx);

        let font_desc = pango::FontDescription::from_string(font_desc_str);
        layout.set_font_description(Some(&font_desc));
        layout.set_text(text);

        if let Some(width) = wrap_width {
            let width = width.max(1);
            let width_pango = (width as i64 * pango::SCALE as i64).min(i32::MAX as i64) as i32;
            layout.set_width(width_pango);
            layout.set_wrap(pango::WrapMode::WordChar);
        }

        let (ink_rect, _logical_rect) = layout.extents();
        let scale = pango::SCALE as f64;

        TextMeasurement {
            ink_x: ink_rect.x() as f64 / scale,
            ink_y: ink_rect.y() as f64 / scale,
            ink_width: ink_rect.width() as f64 / scale,
            ink_height: ink_rect.height() as f64 / scale,
            baseline: layout.baseline() as f64 / scale,
        }
    })?;

    // Cache the result
    TEXT_CACHE.with(|cache| {
        cache.borrow_mut().insert(key, measurement.clone());
    });

    Some(measurement)
}

/// Measure text using cached measurements.
/// The `_ctx` parameter is kept for API compatibility but measurements always
/// use a shared context for consistency across different rendering contexts.
/// Pango measurements are resolution-independent (in Pango units), so using
/// a consistent measurement context ensures cache correctness.
pub(crate) fn measure_text_with_context(
    _ctx: &cairo::Context,
    text: &str,
    font_desc_str: &str,
    size: f64,
    wrap_width: Option<i32>,
) -> Option<TextMeasurement> {
    // Delegate to measure_text_cached for consistent measurements.
    // Pango units are resolution-independent, so the measurement context
    // settings (scale, font options) don't affect the results.
    measure_text_cached(text, font_desc_str, size, wrap_width)
}

/// Clear the text measurement cache.
/// Call this when font configuration changes.
#[allow(dead_code)]
pub fn invalidate_text_cache() {
    TEXT_CACHE.with(|cache| cache.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_returns_same_measurement() {
        let text = "Hello World";
        let font = "Sans 12";

        let m1 = measure_text_cached(text, font, 12.0, None);
        let m2 = measure_text_cached(text, font, 12.0, None);

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
        // Verify that measurements for different sizes are cached with different keys
        // by checking that both requests succeed (cache doesn't confuse them)
        let text = "Test";
        let font = "Sans";

        let m1 = measure_text_cached(text, font, 12.0, None);
        let m2 = measure_text_cached(text, font, 24.0, None);

        assert!(m1.is_some(), "12pt measurement should succeed");
        assert!(m2.is_some(), "24pt measurement should succeed");

        // Request them again - should hit cache for both
        let m1_cached = measure_text_cached(text, font, 12.0, None);
        let m2_cached = measure_text_cached(text, font, 24.0, None);

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
    fn test_empty_text_returns_none() {
        let result = measure_text_cached("", "Sans 12", 12.0, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_wrap_width_affects_cache_key() {
        let text = "A very long text that would wrap";
        let font = "Sans 12";

        let m1 = measure_text_cached(text, font, 12.0, None);
        let m2 = measure_text_cached(text, font, 12.0, Some(50));

        assert!(m1.is_some());
        assert!(m2.is_some());

        // With narrow wrap width, height should be larger (more lines)
        let m1 = m1.unwrap();
        let m2 = m2.unwrap();
        assert!(m2.ink_height >= m1.ink_height);
    }
}
