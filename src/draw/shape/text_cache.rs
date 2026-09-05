mod cache;
mod cursor;
mod owner;

use cache::{TextCacheKey, TextMeasurementCache};
pub use owner::TextMeasurer;

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

/// Run a public convenience operation with an isolated call-local owner.
/// Runtime paths should pass their persistent `TextMeasurer` explicitly.
pub(crate) fn with_scoped_measurer<R>(f: impl FnOnce(&TextMeasurer) -> R) -> R {
    let measurer = TextMeasurer::default();
    f(&measurer)
}

#[cfg(test)]
#[test]
fn scoped_convenience_measurements_do_not_share_cache_entries() {
    with_scoped_measurer(|measurer| {
        assert_eq!(measurer.cache_len(), 0);
        assert!(
            measurer
                .measure("first owner", "Sans", 14.0, None)
                .is_some()
        );
        assert_eq!(measurer.cache_len(), 1);
    });
    with_scoped_measurer(|measurer| {
        assert_eq!(measurer.cache_len(), 0);
        assert!(
            measurer
                .measure("second owner", "Sans", 14.0, None)
                .is_some()
        );
        assert_eq!(measurer.cache_len(), 1);
    });
}

/// Build a Pango layout configured exactly like the measurement and render
/// paths: same font description, same text, same wrap mode and width clamp.
/// Every caret, hit-test, and decoration helper goes through this, so their
/// geometry cannot drift from what `TextMeasurer::measure` and the renderer see.
pub(crate) fn configured_layout(
    ctx: &cairo::Context,
    text: &str,
    font_desc_str: &str,
    wrap_width: Option<i32>,
) -> pango::Layout {
    let layout = pangocairo::functions::create_layout(ctx);
    let font_desc = pango::FontDescription::from_string(font_desc_str);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);
    if let Some(width) = wrap_width {
        let width = width.max(1);
        let width_pango =
            (i64::from(width) * i64::from(pango::SCALE)).min(i64::from(i32::MAX)) as i32;
        layout.set_width(width_pango);
        layout.set_wrap(pango::WrapMode::WordChar);
    }
    layout
}

/// Snap `byte` down to the nearest UTF-8 char boundary at or below it (and to
/// the string length), so Pango is never handed an index that splits a
/// multi-byte character.
fn snap_char_boundary(text: &str, byte: usize) -> usize {
    let mut index = byte.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
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

/// Full logical bounds of a text run — the advance width and line-box height,
/// including the leading/trailing whitespace and empty trailing advance that
/// the ink box omits — in pixels, relative to the stored `(x, y)` baseline
/// origin. Pango paints selection backgrounds and preedit underlines over these
/// logical cells, so damage for either must cover this box, not just the ink.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LogicalBounds {
    pub x: f64,
    pub y_from_baseline: f64,
    pub width: f64,
    pub height: f64,
}

/// Everything the damage tracker needs about a preview's geometry, resolved
/// from a single layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextPreviewGeometry {
    /// Present only when a caret offset was requested.
    pub caret: Option<CaretGeometry>,
    pub logical: LogicalBounds,
}

fn caret_geometry_in(layout: &pango::Layout, text: &str, byte_index: usize) -> CaretGeometry {
    let scale = pango::SCALE as f64;
    let index = snap_char_boundary(text, byte_index);
    let index_i32 = i32::try_from(index).unwrap_or(i32::MAX);
    let (strong, _weak) = layout.cursor_pos(index_i32);
    let baseline = layout.baseline() as f64 / scale;
    CaretGeometry {
        x: strong.x() as f64 / scale,
        y_from_baseline: strong.y() as f64 / scale - baseline,
        height: strong.height() as f64 / scale,
    }
}

fn logical_bounds_in(layout: &pango::Layout) -> LogicalBounds {
    let scale = pango::SCALE as f64;
    let (_ink, logical) = layout.extents();
    let baseline = layout.baseline() as f64 / scale;
    LogicalBounds {
        x: logical.x() as f64 / scale,
        y_from_baseline: logical.y() as f64 / scale - baseline,
        width: logical.width() as f64 / scale,
        height: logical.height() as f64 / scale,
    }
}

#[cfg(test)]
mod tests;
