use crate::draw::shape::{sticky_note_layout, sticky_note_text_layout};
use crate::draw::{Color, FontDescriptor};
use std::f64::consts::{FRAC_PI_2, PI};

/// Renders text at a specified position with multi-line support using Pango.
///
/// Uses Pango for advanced font rendering with custom font support. The position (x, y)
/// represents the text baseline starting point for the first line.
/// Text containing newline characters ('\n') will be rendered across multiple lines
/// with proper line spacing determined by the font metrics.
///
/// Text is rendered with a contrasting stroke outline for better visibility
/// against any background color.
///
/// # Arguments
/// * `ctx` - Cairo drawing context to render to
/// * `x` - X coordinate of text baseline start
/// * `y` - Y coordinate of text baseline (first line)
/// * `text` - Text content to render (may contain '\n' for line breaks)
/// * `color` - Text color
/// * `size` - Font size in points
/// * `font_descriptor` - Font configuration (family, weight, style)
/// * `background_enabled` - Whether to draw background box behind text
#[allow(clippy::too_many_arguments)]
pub fn render_text(
    ctx: &cairo::Context,
    x: i32,
    y: i32,
    text: &str,
    color: Color,
    size: f64,
    font_descriptor: &FontDescriptor,
    background_enabled: bool,
    wrap_width: Option<i32>,
) {
    // Save context state to prevent settings from leaking to other drawing operations
    ctx.save().ok();

    // Use Best antialiasing (gray) instead of Subpixel for ARGB overlay
    // Subpixel can cause color fringing on transparent/composited surfaces
    ctx.set_antialias(cairo::Antialias::Best);

    // Create Pango layout for text rendering
    let layout = pangocairo::functions::create_layout(ctx);

    // Set font description from config
    let font_desc_str = font_descriptor.to_pango_string(size);
    let font_desc = pango::FontDescription::from_string(&font_desc_str);
    layout.set_font_description(Some(&font_desc));

    // Set the text (Pango handles newlines automatically)
    layout.set_text(text);
    if let Some(width) = wrap_width {
        let width = width.max(1);
        let width_pango = (width as i64 * pango::SCALE as i64).min(i32::MAX as i64) as i32;
        layout.set_width(width_pango);
        layout.set_wrap(pango::WrapMode::WordChar);
    }

    // Get layout extents for background and effects
    let (ink_rect, _logical_rect) = layout.extents();

    // Include ink rect offsets for italic/stroked glyphs with negative bearings
    let ink_x = ink_rect.x() as f64 / pango::SCALE as f64;
    let ink_y = ink_rect.y() as f64 / pango::SCALE as f64;
    let ink_width = ink_rect.width() as f64 / pango::SCALE as f64;
    let ink_height = ink_rect.height() as f64 / pango::SCALE as f64;

    // Calculate brightness to determine background/stroke color
    let brightness = color.r * 0.299 + color.g * 0.587 + color.b * 0.114;
    let (bg_r, bg_g, bg_b) = if brightness > 0.5 {
        (0.0, 0.0, 0.0) // Dark background/stroke for light text colors
    } else {
        (1.0, 1.0, 1.0) // Light background/stroke for dark text colors
    };

    // Adjust y position (Pango measures from top-left, we want baseline)
    let baseline = layout.baseline() as f64 / pango::SCALE as f64;
    let adjusted_y = y as f64 - baseline;

    // First pass: draw semi-transparent background rectangle (if enabled)
    if background_enabled && ink_width > 0.0 && ink_height > 0.0 {
        let padding = size * 0.15;
        // Use ink rect offsets to properly align background for italic/stroked glyphs
        ctx.rectangle(
            x as f64 + ink_x - padding,
            adjusted_y + ink_y - padding,
            ink_width + padding * 2.0,
            ink_height + padding * 2.0,
        );
        ctx.set_source_rgba(bg_r, bg_g, bg_b, 0.3);
        let _ = ctx.fill();
    }

    // Second pass: draw drop shadow for depth
    let shadow_offset = size * 0.04;
    ctx.move_to(x as f64 + shadow_offset, adjusted_y + shadow_offset);
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.4);
    pangocairo::functions::show_layout(ctx, &layout);

    // Third pass: render text with contrasting stroke outline
    ctx.move_to(x as f64, adjusted_y);

    // Create path from layout for stroking
    pangocairo::functions::layout_path(ctx, &layout);

    // Fully opaque stroke for maximum contrast and crispness
    ctx.set_source_rgba(bg_r, bg_g, bg_b, 1.0);
    ctx.set_line_width(size * 0.06);
    ctx.set_line_join(cairo::LineJoin::Round);
    let _ = ctx.stroke_preserve();

    // Fill with bright, full-intensity color
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    let _ = ctx.fill();

    // Restore context state
    ctx.restore().ok();
}

/// Renders a sticky note with a filled background and drop shadow.
#[allow(clippy::too_many_arguments)]
pub fn render_sticky_note(
    ctx: &cairo::Context,
    x: i32,
    y: i32,
    text: &str,
    background: Color,
    size: f64,
    font_descriptor: &FontDescriptor,
    wrap_width: Option<i32>,
) {
    if text.is_empty() {
        return;
    }

    ctx.save().ok();
    ctx.set_antialias(cairo::Antialias::Best);

    let text_layout = sticky_note_text_layout(ctx, text, size, font_descriptor, wrap_width);
    let base_x = x as f64;
    let base_y = y as f64 - text_layout.baseline;
    let ink_max = text_layout.ink_x + text_layout.ink_width;
    let effective_max = if let Some(width) = wrap_width {
        ink_max.max(width.max(1) as f64)
    } else {
        ink_max
    };
    let effective_ink_width = effective_max - text_layout.ink_x;
    let note_layout = sticky_note_layout(
        base_x,
        base_y,
        text_layout.ink_x,
        text_layout.ink_y,
        effective_ink_width,
        text_layout.ink_height,
        size,
    );

    let shadow_alpha = (0.25 * background.a).clamp(0.0, 0.35);
    ctx.set_source_rgba(0.0, 0.0, 0.0, shadow_alpha);
    draw_round_rect(
        ctx,
        note_layout.note_x + note_layout.shadow_offset,
        note_layout.note_y + note_layout.shadow_offset,
        note_layout.note_width,
        note_layout.note_height,
        note_layout.corner_radius,
    );
    let _ = ctx.fill();

    ctx.set_source_rgba(background.r, background.g, background.b, background.a);
    draw_round_rect(
        ctx,
        note_layout.note_x,
        note_layout.note_y,
        note_layout.note_width,
        note_layout.note_height,
        note_layout.corner_radius,
    );
    let _ = ctx.fill();

    let brightness = background.r * 0.299 + background.g * 0.587 + background.b * 0.114;
    let (text_r, text_g, text_b) = if brightness > 0.6 {
        (0.12, 0.12, 0.12)
    } else {
        (0.98, 0.98, 0.98)
    };
    ctx.move_to(base_x, base_y);
    ctx.set_source_rgba(text_r, text_g, text_b, 1.0);
    pangocairo::functions::show_layout(ctx, &text_layout.layout);

    ctx.restore().ok();
}

fn draw_round_rect(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let radius = r.min(w / 2.0).min(h / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + w - radius, y + radius, radius, -FRAC_PI_2, 0.0);
    ctx.arc(x + w - radius, y + h - radius, radius, 0.0, FRAC_PI_2);
    ctx.arc(x + radius, y + h - radius, radius, FRAC_PI_2, PI);
    ctx.arc(x + radius, y + radius, radius, PI, 3.0 * FRAC_PI_2);
    ctx.close_path();
}
