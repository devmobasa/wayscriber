use crate::draw::font::FontDescriptor;
use crate::util::Rect;

use super::bounds::ensure_positive_rect_f64;

pub(crate) struct TextLayoutMetrics {
    pub(crate) ink_x: f64,
    pub(crate) ink_y: f64,
    pub(crate) ink_width: f64,
    pub(crate) ink_height: f64,
    pub(crate) baseline: f64,
}

pub(super) fn text_layout_metrics(
    text: &str,
    size: f64,
    font_descriptor: &FontDescriptor,
    wrap_width: Option<i32>,
) -> Option<TextLayoutMetrics> {
    if text.is_empty() {
        return None;
    }

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).ok()?;
    let ctx = cairo::Context::new(&surface).ok()?;

    ctx.set_antialias(cairo::Antialias::Best);

    let layout = pangocairo::functions::create_layout(&ctx);

    let font_desc_str = font_descriptor.to_pango_string(size);
    let font_desc = pango::FontDescription::from_string(&font_desc_str);
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
    let baseline = layout.baseline() as f64 / scale;

    Some(TextLayoutMetrics {
        ink_x: ink_rect.x() as f64 / scale,
        ink_y: ink_rect.y() as f64 / scale,
        ink_width: ink_rect.width() as f64 / scale,
        ink_height: ink_rect.height() as f64 / scale,
        baseline,
    })
}

pub(super) fn text_bounds_from_metrics(
    x: f64,
    y: f64,
    metrics: &TextLayoutMetrics,
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

    let effective_ink_width = effective_max - metrics.ink_x;

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

    if background_enabled && effective_ink_width > 0.0 && metrics.ink_height > 0.0 {
        let padding = size * 0.15;
        let bg_min_x = base_x + metrics.ink_x - padding;
        let bg_min_y = base_y + metrics.ink_y - padding;
        let bg_max_x = base_x + effective_max + padding;
        let bg_max_y = base_y + metrics.ink_y + metrics.ink_height + padding;

        min_x = min_x.min(bg_min_x);
        min_y = min_y.min(bg_min_y);
        max_x = max_x.max(bg_max_x);
        max_y = max_y.max(bg_max_y);
    }

    ensure_positive_rect_f64(min_x, min_y, max_x, max_y)
}

pub(crate) fn bounding_box_for_text(
    x: i32,
    y: i32,
    text: &str,
    size: f64,
    font_descriptor: &FontDescriptor,
    background_enabled: bool,
    wrap_width: Option<i32>,
) -> Option<Rect> {
    let metrics = text_layout_metrics(text, size, font_descriptor, wrap_width)?;
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
    pub ink_x: f64,
    pub ink_y: f64,
    pub ink_width: f64,
    pub ink_height: f64,
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

pub(crate) fn sticky_note_text_layout(
    ctx: &cairo::Context,
    text: &str,
    size: f64,
    font_descriptor: &FontDescriptor,
    wrap_width: Option<i32>,
) -> StickyNoteTextLayout {
    let layout = pangocairo::functions::create_layout(ctx);
    let font_desc_str = font_descriptor.to_pango_string(size);
    let font_desc = pango::FontDescription::from_string(&font_desc_str);
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

    let baseline = layout.baseline() as f64 / scale;

    StickyNoteTextLayout {
        layout,
        ink_x: ink_rect.x() as f64 / scale,
        ink_y: ink_rect.y() as f64 / scale,
        ink_width: ink_rect.width() as f64 / scale,
        ink_height: ink_rect.height() as f64 / scale,
        baseline,
    }
}

pub(crate) fn bounding_box_for_sticky_note(
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

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).ok()?;
    let ctx = cairo::Context::new(&surface).ok()?;

    ctx.set_antialias(cairo::Antialias::Best);

    let text_layout = sticky_note_text_layout(&ctx, text, size, font_descriptor, wrap_width);
    let base_x = x as f64;
    let base_y = y as f64 - text_layout.baseline;
    let ink_max = text_layout.ink_x + text_layout.ink_width;
    let effective_max = if let Some(width) = wrap_width {
        ink_max.max(width.max(1) as f64)
    } else {
        ink_max
    };
    let effective_ink_width = effective_max - text_layout.ink_x;
    let layout = sticky_note_layout(
        base_x,
        base_y,
        text_layout.ink_x,
        text_layout.ink_y,
        effective_ink_width,
        text_layout.ink_height,
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
