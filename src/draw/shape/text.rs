use crate::draw::font::FontDescriptor;
use crate::util::Rect;

use super::bounds::ensure_positive_rect_f64;
use super::text_cache::{TextContentExtents, TextMeasurement, TextMeasurer};

pub(super) fn text_layout_metrics(
    measurer: &TextMeasurer,
    text: &str,
    size: f64,
    font_descriptor: &FontDescriptor,
    wrap_width: Option<i32>,
) -> Option<TextMeasurement> {
    if text.is_empty() {
        return None;
    }

    // Use cached text measurement instead of creating a new surface each time
    let font_desc_str = font_descriptor.to_pango_string(size);
    let measurement = measurer.measure(text, &font_desc_str, size, wrap_width)?;

    Some(measurement)
}

pub(super) fn text_bounds_from_metrics(
    x: f64,
    y: f64,
    metrics: &TextMeasurement,
    size: f64,
    background_enabled: bool,
    wrap_width: Option<i32>,
) -> Option<Rect> {
    let base_x = x;
    let base_y = y - metrics.baseline;
    let ink_max = metrics.ink_x + metrics.ink_width;
    let effective_max = if let Some(width) = wrap_width {
        ink_max.max(width.max(1) as f64)
    } else {
        ink_max
    };

    let mut min_x = base_x + metrics.ink_x;
    let mut max_x = base_x + effective_max;
    let mut min_y = base_y + metrics.ink_y;
    let mut max_y = min_y + metrics.ink_height;

    let content = metrics.content_extents(wrap_width);

    let stroke_padding = (size * 0.06) / 2.0;
    min_x -= stroke_padding;
    max_x += stroke_padding;
    min_y -= stroke_padding;
    max_y += stroke_padding;

    let shadow_offset = size * 0.04;
    min_x = min_x.min(base_x + metrics.ink_x + shadow_offset - stroke_padding);
    min_y = min_y.min(base_y + metrics.ink_y + shadow_offset - stroke_padding);
    max_x = max_x.max(base_x + effective_max + shadow_offset + stroke_padding);
    max_y = max_y.max(base_y + metrics.ink_y + metrics.ink_height + shadow_offset + stroke_padding);

    if background_enabled && content.width > 0.0 && content.height > 0.0 {
        let padding = size * 0.15;
        let bg_min_x = base_x + content.x - padding;
        let bg_min_y = base_y + content.y - padding;
        let bg_max_x = base_x + content.x + content.width + padding;
        let bg_max_y = base_y + content.y + content.height + padding;

        min_x = min_x.min(bg_min_x);
        min_y = min_y.min(bg_min_y);
        max_x = max_x.max(bg_max_x);
        max_y = max_y.max(bg_max_y);
    }

    ensure_positive_rect_f64(min_x, min_y, max_x, max_y)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn bounding_box_for_text_with(
    measurer: &TextMeasurer,
    x: i32,
    y: i32,
    text: &str,
    size: f64,
    font_descriptor: &FontDescriptor,
    background_enabled: bool,
    wrap_width: Option<i32>,
) -> Option<Rect> {
    let metrics = text_layout_metrics(measurer, text, size, font_descriptor, wrap_width)?;
    text_bounds_from_metrics(
        x as f64,
        y as f64,
        &metrics,
        size,
        background_enabled,
        wrap_width,
    )
}

const NOTE_PADDING_X_RATIO: f64 = 0.55;
const NOTE_PADDING_Y_RATIO: f64 = 0.4;
const NOTE_PADDING_MIN_X: f64 = 6.0;
const NOTE_PADDING_MIN_Y: f64 = 4.0;
const NOTE_SHADOW_OFFSET_RATIO: f64 = 0.18;
const NOTE_SHADOW_OFFSET_MIN: f64 = 3.0;
const NOTE_CORNER_RADIUS_RATIO: f64 = 0.2;
const NOTE_CORNER_RADIUS_MIN: f64 = 4.0;

/// Measurement-only glyph that gives an empty live note the same useful
/// minimum body it had when the editor injected its caret into the text run.
pub(crate) fn sticky_note_layout_text(text: &str) -> &str {
    if text.is_empty() { "_" } else { text }
}

pub(crate) struct StickyNoteLayout {
    pub note_x: f64,
    pub note_y: f64,
    pub note_width: f64,
    pub note_height: f64,
    pub shadow_offset: f64,
    pub corner_radius: f64,
}

pub(crate) struct StickyNoteTextLayout {
    pub layout: pango::Layout,
    pub content: TextContentExtents,
    pub baseline: f64,
}

pub(crate) fn sticky_note_layout(
    base_x: f64,
    base_y: f64,
    ink_x: f64,
    ink_y: f64,
    ink_width: f64,
    ink_height: f64,
    size: f64,
) -> StickyNoteLayout {
    let padding_x = (size * NOTE_PADDING_X_RATIO).max(NOTE_PADDING_MIN_X);
    let padding_y = (size * NOTE_PADDING_Y_RATIO).max(NOTE_PADDING_MIN_Y);
    let note_x = base_x + ink_x - padding_x;
    let note_y = base_y + ink_y - padding_y;
    let note_width = ink_width + padding_x * 2.0;
    let note_height = ink_height + padding_y * 2.0;
    let shadow_offset = (size * NOTE_SHADOW_OFFSET_RATIO).max(NOTE_SHADOW_OFFSET_MIN);
    let corner_radius = (size * NOTE_CORNER_RADIUS_RATIO).max(NOTE_CORNER_RADIUS_MIN);

    StickyNoteLayout {
        note_x,
        note_y,
        note_width,
        note_height,
        shadow_offset,
        corner_radius,
    }
}

pub(crate) fn sticky_note_text_layout_with_measurer(
    measurer: &crate::draw::TextMeasurer,
    ctx: &cairo::Context,
    text: &str,
    size: f64,
    font_descriptor: &FontDescriptor,
    wrap_width: Option<i32>,
) -> StickyNoteTextLayout {
    let font_desc_str = font_descriptor.to_pango_string(size);

    // Create layout for rendering (required for draw operations)
    let layout = pangocairo::functions::create_layout(ctx);
    let font_desc = pango::FontDescription::from_string(&font_desc_str);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);
    if let Some(width) = wrap_width {
        let width = width.max(1);
        let width_pango = (width as i64 * pango::SCALE as i64).min(i32::MAX as i64) as i32;
        layout.set_width(width_pango);
        layout.set_wrap(pango::WrapMode::WordChar);
    }

    // Use cached measurements if available, otherwise measure and cache
    if let Some(measurement) = measurer.measure(text, &font_desc_str, size, wrap_width) {
        StickyNoteTextLayout {
            layout,
            content: measurement.content_extents(wrap_width),
            baseline: measurement.baseline,
        }
    } else {
        // Fallback: measure directly (shouldn't happen for non-empty text)
        let (ink_rect, logical_rect) = layout.extents();
        let scale = pango::SCALE as f64;
        let baseline = layout.baseline() as f64 / scale;
        let measurement = TextMeasurement {
            ink_x: ink_rect.x() as f64 / scale,
            ink_y: ink_rect.y() as f64 / scale,
            ink_width: ink_rect.width() as f64 / scale,
            ink_height: ink_rect.height() as f64 / scale,
            logical_x: logical_rect.x() as f64 / scale,
            logical_y: logical_rect.y() as f64 / scale,
            logical_width: logical_rect.width() as f64 / scale,
            logical_height: logical_rect.height() as f64 / scale,
            baseline,
        };

        StickyNoteTextLayout {
            layout,
            content: measurement.content_extents(wrap_width),
            baseline,
        }
    }
}

pub(crate) fn bounding_box_for_sticky_note_with(
    measurer: &TextMeasurer,
    x: i32,
    y: i32,
    text: &str,
    size: f64,
    font_descriptor: &FontDescriptor,
    wrap_width: Option<i32>,
) -> Option<Rect> {
    if text.is_empty() {
        return None;
    }
    bounding_box_for_sticky_note_layout(measurer, x, y, text, size, font_descriptor, wrap_width)
}

pub(crate) fn bounding_box_for_sticky_note_preview_with(
    measurer: &TextMeasurer,
    x: i32,
    y: i32,
    text: &str,
    size: f64,
    font_descriptor: &FontDescriptor,
    wrap_width: Option<i32>,
) -> Option<Rect> {
    bounding_box_for_sticky_note_layout(
        measurer,
        x,
        y,
        sticky_note_layout_text(text),
        size,
        font_descriptor,
        wrap_width,
    )
}

fn bounding_box_for_sticky_note_layout(
    measurer: &TextMeasurer,
    x: i32,
    y: i32,
    text: &str,
    size: f64,
    font_descriptor: &FontDescriptor,
    wrap_width: Option<i32>,
) -> Option<Rect> {
    // Use cached text measurement instead of creating a new surface each time
    let font_desc_str = font_descriptor.to_pango_string(size);
    let measurement = measurer.measure(text, &font_desc_str, size, wrap_width)?;

    let base_x = x as f64;
    let base_y = y as f64 - measurement.baseline;
    let content = measurement.content_extents(wrap_width);
    let layout = sticky_note_layout(
        base_x,
        base_y,
        content.x,
        content.y,
        content.width,
        content.height,
        size,
    );

    let note_min_x = layout.note_x;
    let note_min_y = layout.note_y;
    let note_max_x = layout.note_x + layout.note_width;
    let note_max_y = layout.note_y + layout.note_height;

    let shadow_min_x = note_min_x + layout.shadow_offset;
    let shadow_min_y = note_min_y + layout.shadow_offset;
    let shadow_max_x = note_max_x + layout.shadow_offset;
    let shadow_max_y = note_max_y + layout.shadow_offset;

    let min_x = note_min_x.min(shadow_min_x);
    let min_y = note_min_y.min(shadow_min_y);
    let max_x = note_max_x.max(shadow_max_x);
    let max_y = note_max_y.max(shadow_max_y);

    ensure_positive_rect_f64(min_x, min_y, max_x, max_y)
}
