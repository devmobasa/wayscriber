use std::cell::RefCell;
use std::collections::HashMap;

/// Cached text measurement results from Pango layout.
#[derive(Clone, Debug)]
pub(crate) struct TextMeasurement {
    pub ink_x: f64,
    pub ink_y: f64,
    pub ink_width: f64,
    pub ink_height: f64,
    pub logical_x: f64,
    pub logical_y: f64,
    pub logical_width: f64,
    pub logical_height: f64,
    pub baseline: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextContentExtents {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl TextMeasurement {
    /// Union of glyph ink and Pango's logical line cells. The logical extent
    /// preserves leading/trailing whitespace advances that ink omits.
    pub(crate) fn content_extents(&self, wrap_width: Option<i32>) -> TextContentExtents {
        let min_x = self.ink_x.min(self.logical_x);
        let min_y = self.ink_y.min(self.logical_y);
        let mut max_x = (self.ink_x + self.ink_width).max(self.logical_x + self.logical_width);
        if let Some(width) = wrap_width {
            max_x = max_x.max(width.max(1) as f64);
        }
        let max_y = (self.ink_y + self.ink_height).max(self.logical_y + self.logical_height);
        TextContentExtents {
            x: min_x,
            y: min_y,
            width: (max_x - min_x).max(0.0),
            height: (max_y - min_y).max(0.0),
        }
    }
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

        let (ink_rect, logical_rect) = layout.extents();
        let scale = pango::SCALE as f64;

        TextMeasurement {
            ink_x: ink_rect.x() as f64 / scale,
            ink_y: ink_rect.y() as f64 / scale,
            ink_width: ink_rect.width() as f64 / scale,
            ink_height: ink_rect.height() as f64 / scale,
            logical_x: logical_rect.x() as f64 / scale,
            logical_y: logical_rect.y() as f64 / scale,
            logical_width: logical_rect.width() as f64 / scale,
            logical_height: logical_rect.height() as f64 / scale,
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

/// Hit-test a point against a rendered text run, returning the caret byte
/// offset nearest the point. Coordinates are relative to the text's stored
/// origin `(x, y)`: `local_x = point_x - x`, and `local_y_from_baseline =
/// point_y - y` (the stored `y` is the first-line baseline). Layout-aware, so
/// it is correct for wrapped and multiline text. The caret snaps to the
/// trailing edge of a glyph when the point is on its right half. Returns `None`
/// only when no measurement context is available.
pub(crate) fn hit_test_text(
    text: &str,
    font_desc_str: &str,
    wrap_width: Option<i32>,
    local_x: f64,
    local_y_from_baseline: f64,
) -> Option<usize> {
    if text.is_empty() {
        return Some(0);
    }
    with_measurement_context(|ctx| {
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

        let scale = pango::SCALE as f64;
        // Convert the baseline-relative y into the layout's top-left frame.
        let local_y = local_y_from_baseline + layout.baseline() as f64 / scale;
        let x_pu = (local_x * scale)
            .round()
            .clamp(i32::MIN as f64, i32::MAX as f64) as i32;
        let y_pu = (local_y * scale)
            .round()
            .clamp(i32::MIN as f64, i32::MAX as f64) as i32;
        let (_inside, index, trailing) = layout.xy_to_index(x_pu, y_pu);

        // Advance past `trailing` characters so a click on a glyph's right half
        // lands the caret after it, keeping the result on a char boundary.
        hit_position_to_byte(text, index, trailing)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VisualLineDirection {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VisualCaretDirection {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VisualLineEdge {
    Start,
    End,
}

/// Return the adjacent Pango cursor position in physical left/right order.
/// Logical byte order is insufficient for RTL and mixed-direction text.
pub(crate) fn caret_on_adjacent_visual_position(
    text: &str,
    font_desc_str: &str,
    wrap_width: Option<i32>,
    byte_index: usize,
    direction: VisualCaretDirection,
) -> Option<usize> {
    with_measurement_context(|ctx| {
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

        let mut index = byte_index.min(text.len());
        while index > 0 && !text.is_char_boundary(index) {
            index -= 1;
        }
        let old_index = i32::try_from(index).unwrap_or(i32::MAX);
        let direction = match direction {
            VisualCaretDirection::Left => -1,
            VisualCaretDirection::Right => 1,
        };
        let (new_index, trailing) = layout.move_cursor_visually(true, old_index, 0, direction);

        // Pango uses sentinels when movement would leave either visual edge of
        // the layout. Keep the current logical position at those boundaries.
        if new_index < 0 || new_index == i32::MAX {
            return index;
        }
        hit_position_to_byte(text, new_index, trailing)
    })
}

/// Resolve which endpoint of a same-line selection is physically left/right.
/// For selections crossing visual lines, preserve the editor's established
/// document-order collapse behavior.
pub(crate) fn caret_at_visual_selection_edge(
    text: &str,
    font_desc_str: &str,
    wrap_width: Option<i32>,
    start: usize,
    end: usize,
    direction: VisualCaretDirection,
) -> Option<usize> {
    with_measurement_context(|ctx| {
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

        let start = start.min(text.len());
        let end = end.min(text.len());
        let (start_line, start_x) =
            layout.index_to_line_x(i32::try_from(start).unwrap_or(i32::MAX), false);
        let (end_line, end_x) =
            layout.index_to_line_x(i32::try_from(end).unwrap_or(i32::MAX), false);
        if start_line != end_line {
            return match direction {
                VisualCaretDirection::Left => start,
                VisualCaretDirection::Right => end,
            };
        }
        match direction {
            VisualCaretDirection::Left if start_x <= end_x => start,
            VisualCaretDirection::Left => end,
            VisualCaretDirection::Right if start_x >= end_x => start,
            VisualCaretDirection::Right => end,
        }
    })
}

/// Return the logical byte offset at the start/end of the current Pango visual
/// line, including lines introduced by soft wrapping.
pub(crate) fn caret_on_visual_line_edge(
    text: &str,
    font_desc_str: &str,
    wrap_width: Option<i32>,
    byte_index: usize,
    edge: VisualLineEdge,
) -> Option<usize> {
    with_measurement_context(|ctx| {
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

        let mut index = byte_index.min(text.len());
        while index > 0 && !text.is_char_boundary(index) {
            index -= 1;
        }
        let (line_index, _) =
            layout.index_to_line_x(i32::try_from(index).unwrap_or(i32::MAX), false);
        let Some(line) = layout.line_readonly(line_index) else {
            return index;
        };
        let start = usize::try_from(line.start_index()).unwrap_or(0);
        let mut end = start
            .saturating_add(usize::try_from(line.length()).unwrap_or(0))
            .min(text.len());
        if end > start && text.as_bytes().get(end - 1) == Some(&b'\n') {
            end -= 1;
        }
        match edge {
            VisualLineEdge::Start => start,
            VisualLineEdge::End => end,
        }
    })
}

/// Return the caret offset on the adjacent Pango visual line while preserving
/// the current horizontal layout position. This follows soft wrapping as well
/// as explicit newlines. At the first/last visual line it resolves to the
/// document start/end, matching the editor's existing boundary behavior.
pub(crate) fn caret_on_adjacent_visual_line(
    text: &str,
    font_desc_str: &str,
    wrap_width: Option<i32>,
    byte_index: usize,
    direction: VisualLineDirection,
) -> Option<usize> {
    with_measurement_context(|ctx| {
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

        let mut index = byte_index.min(text.len());
        while index > 0 && !text.is_char_boundary(index) {
            index -= 1;
        }
        let index_i32 = i32::try_from(index).unwrap_or(i32::MAX);
        let (line_index, x) = layout.index_to_line_x(index_i32, false);
        let target_line = match direction {
            VisualLineDirection::Up if line_index == 0 => return 0,
            VisualLineDirection::Up => line_index - 1,
            VisualLineDirection::Down if line_index + 1 >= layout.line_count() => {
                return text.len();
            }
            VisualLineDirection::Down => line_index + 1,
        };
        let Some(line) = layout.line_readonly(target_line) else {
            return index;
        };
        let hit = line.x_to_index(x);
        hit_position_to_byte(text, hit.index(), hit.trailing())
    })
}

fn hit_position_to_byte(text: &str, index: i32, trailing: i32) -> usize {
    let mut byte = (index.max(0) as usize).min(text.len());
    let mut remaining = trailing.max(0);
    let mut chars = text[byte..].chars();
    while remaining > 0 {
        match chars.next() {
            Some(ch) => byte += ch.len_utf8(),
            None => break,
        }
        remaining -= 1;
    }
    byte.min(text.len())
}

/// Geometry for drawing a vertical caret line, in pixels, relative to the
/// text's stored origin `(x, y)` where `y` is the first-line baseline. Draw the
/// caret from `(x + geom.x, y + geom.y_from_baseline)` downward `height` pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CaretGeometry {
    pub x: f64,
    /// Top of the caret line, measured from the first-line baseline (negative
    /// for the first line, larger for wrapped/lower lines).
    pub y_from_baseline: f64,
    pub height: f64,
}

/// Compute the caret geometry for `byte_index` into `text` using Pango's strong
/// cursor position, so it lands correctly on wrapped and multiline text and at
/// the string end. Works for empty text (caret at the origin). `byte_index` is
/// snapped down to a char boundary. Returns `None` only when no measurement
/// context is available.
pub(crate) fn caret_geometry_text(
    text: &str,
    font_desc_str: &str,
    wrap_width: Option<i32>,
    byte_index: usize,
) -> Option<CaretGeometry> {
    with_measurement_context(|ctx| {
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

        let scale = pango::SCALE as f64;
        // Snap to a char boundary at or below the requested byte so Pango is
        // never handed an index that splits a multi-byte character.
        let mut index = byte_index.min(text.len());
        while index > 0 && !text.is_char_boundary(index) {
            index -= 1;
        }
        let index_i32 = i32::try_from(index).unwrap_or(i32::MAX);
        let (strong, _weak) = layout.cursor_pos(index_i32);
        let baseline = layout.baseline() as f64 / scale;
        CaretGeometry {
            x: strong.x() as f64 / scale,
            y_from_baseline: strong.y() as f64 / scale - baseline,
            height: strong.height() as f64 / scale,
        }
    })
}

/// Full logical bounds of `text` — the advance width and line-box height,
/// including leading/trailing whitespace and empty trailing advance that the ink
/// box omits — in pixels, relative to the stored `(x, y)` baseline origin:
/// `(x_offset, y_from_baseline, width, height)`. Pango paints selection
/// backgrounds over these logical cells, so damage for a selection must cover
/// this box, not just the ink extents. Returns `None` when no context exists.
pub(crate) fn text_logical_bounds(
    text: &str,
    font_desc_str: &str,
    wrap_width: Option<i32>,
) -> Option<(f64, f64, f64, f64)> {
    with_measurement_context(|ctx| {
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
        let scale = pango::SCALE as f64;
        let (_ink, logical) = layout.extents();
        let baseline = layout.baseline() as f64 / scale;
        (
            logical.x() as f64 / scale,
            logical.y() as f64 / scale - baseline,
            logical.width() as f64 / scale,
            logical.height() as f64 / scale,
        )
    })
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
        // Far-left click lands at the start; far-right at the end; the exact
        // glyph widths do not matter, only the ordering and clamping.
        assert_eq!(
            hit_test_text("hello", "Sans 20", None, -100.0, 0.0),
            Some(0)
        );
        assert_eq!(
            hit_test_text("hello", "Sans 20", None, 100_000.0, 0.0),
            Some(5)
        );
        // Empty text always resolves to caret 0.
        assert_eq!(hit_test_text("", "Sans 20", None, 42.0, 0.0), Some(0));
    }

    #[test]
    fn hit_test_result_is_always_a_char_boundary() {
        // '你好' is two 3-byte chars; any x must land on 0, 3, or 6.
        let text = "你好";
        for x in [-10.0, 0.0, 5.0, 12.0, 30.0, 1000.0] {
            let offset = hit_test_text(text, "Sans 20", None, x, 0.0).unwrap();
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
