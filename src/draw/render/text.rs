use crate::draw::shape::{
    TextMeasurement, measure_text_with_context, sticky_note_layout, sticky_note_layout_text,
    sticky_note_text_layout,
};
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

    // Use cached measurements for ink rect (avoids repeated Pango measurement)
    let measurement = measure_text_with_context(ctx, text, &font_desc_str, size, wrap_width)
        .unwrap_or_else(|| {
            // Fallback: measure directly.
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
        });
    let content = measurement.content_extents(wrap_width);

    // Calculate brightness to determine background/stroke color
    let outline = text_outline_color(color);

    // Adjust y position (Pango measures from top-left, we want baseline)
    let adjusted_y = y as f64 - measurement.baseline;

    // First pass: draw semi-transparent background rectangle (if enabled)
    if background_enabled && content.width > 0.0 && content.height > 0.0 {
        let padding = size * 0.15;
        // Union ink and logical extents: ink preserves italic overhangs while
        // logical cells retain leading/trailing whitespace advances.
        ctx.rectangle(
            x as f64 + content.x - padding,
            adjusted_y + content.y - padding,
            content.width + padding * 2.0,
            content.height + padding * 2.0,
        );
        ctx.set_source_rgba(outline.r, outline.g, outline.b, 0.3);
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
    ctx.set_source_rgba(outline.r, outline.g, outline.b, outline.a);
    ctx.set_line_width(size * 0.06);
    ctx.set_line_join(cairo::LineJoin::Round);
    let _ = ctx.stroke_preserve();

    // Fill with bright, full-intensity color
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    let _ = ctx.fill();

    // Restore context state
    ctx.restore().ok();
}

/// Stroke width of the editor's standalone caret line at `size`.
pub fn caret_line_width(size: f64) -> f64 {
    (size * 0.06).max(1.5)
}

/// Width of the contrasting outline stroked beneath the caret line. The caret
/// is painted centred on its position, so it reaches `caret_outline_width / 2`
/// to either side. The damage tracker sizes the caret's repaint rectangle from
/// this, which is why the value lives here next to the code that paints it —
/// duplicating the constant is how the caret used to leave a trail on drags.
pub fn caret_outline_width(size: f64) -> f64 {
    caret_line_width(size) + 2.0
}

/// Opaque contrasting color used to outline plain text and its separate caret.
pub fn text_outline_color(color: Color) -> Color {
    let brightness = color.r * 0.299 + color.g * 0.587 + color.b * 0.114;
    if brightness > 0.5 {
        Color::new(0.0, 0.0, 0.0, 1.0)
    } else {
        Color::new(1.0, 1.0, 1.0, 1.0)
    }
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
    render_sticky_note_layout(
        ctx,
        x,
        y,
        text,
        true,
        background,
        size,
        font_descriptor,
        wrap_width,
    );
}

/// Render the live sticky-note editor, using a measurement-only placeholder
/// when its buffer is empty so the background remains visible behind the caret.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_sticky_note_preview(
    ctx: &cairo::Context,
    x: i32,
    y: i32,
    text: &str,
    background: Color,
    size: f64,
    font_descriptor: &FontDescriptor,
    wrap_width: Option<i32>,
) {
    render_sticky_note_layout(
        ctx,
        x,
        y,
        sticky_note_layout_text(text),
        !text.is_empty(),
        background,
        size,
        font_descriptor,
        wrap_width,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_sticky_note_layout(
    ctx: &cairo::Context,
    x: i32,
    y: i32,
    layout_text: &str,
    paint_text: bool,
    background: Color,
    size: f64,
    font_descriptor: &FontDescriptor,
    wrap_width: Option<i32>,
) {
    ctx.save().ok();
    ctx.set_antialias(cairo::Antialias::Best);

    let text_layout = sticky_note_text_layout(ctx, layout_text, size, font_descriptor, wrap_width);
    let base_x = x as f64;
    let base_y = y as f64 - text_layout.baseline;
    let note_layout = sticky_note_layout(
        base_x,
        base_y,
        text_layout.content.x,
        text_layout.content.y,
        text_layout.content.width,
        text_layout.content.height,
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

    if paint_text {
        let fg = sticky_note_foreground(background);
        ctx.move_to(base_x, base_y);
        ctx.set_source_rgba(fg.r, fg.g, fg.b, fg.a);
        pangocairo::functions::show_layout(ctx, &text_layout.layout);
    }

    ctx.restore().ok();
}

/// Contrasting foreground (text/caret) color for a sticky note of the given
/// background: near-black on light notes, near-white on dark ones. Shared so the
/// live caret matches the committed note text instead of vanishing into the
/// background fill.
pub fn sticky_note_foreground(background: Color) -> Color {
    let brightness = background.r * 0.299 + background.g * 0.587 + background.b * 0.114;
    if brightness > 0.6 {
        Color {
            r: 0.12,
            g: 0.12,
            b: 0.12,
            a: 1.0,
        }
    } else {
        Color {
            r: 0.98,
            g: 0.98,
            b: 0.98,
            a: 1.0,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{
        Color, FontDescriptor, caret_outline_width, render_sticky_note, render_sticky_note_preview,
        render_text, sticky_note_foreground, text_outline_color,
    };

    fn alpha_at(surface: &mut cairo::ImageSurface, x: i32, y: i32) -> u8 {
        surface.flush();
        let stride = surface.stride() as usize;
        let offset = y as usize * stride + x as usize * 4;
        surface.data().unwrap()[offset + 3]
    }

    #[test]
    fn text_outline_contrasts_with_light_and_dark_caret_colors() {
        let light_outline = text_outline_color(Color::new(1.0, 1.0, 1.0, 1.0));
        assert!(light_outline.r < 0.5);

        let dark_outline = text_outline_color(Color::new(0.0, 0.0, 0.0, 1.0));
        assert!(dark_outline.r > 0.5);
    }

    #[test]
    fn foreground_is_dark_on_light_notes_and_light_on_dark_notes() {
        let light = sticky_note_foreground(Color::new(0.95, 0.9, 0.2, 1.0));
        assert!(light.r < 0.5, "dark text on a light note");

        let dark = sticky_note_foreground(Color::new(0.1, 0.1, 0.15, 1.0));
        assert!(dark.r > 0.5, "light text on a dark note");

        // Always fully opaque so the caret is never washed out.
        assert_eq!(light.a, 1.0);
        assert_eq!(dark.a, 1.0);
    }

    #[test]
    fn empty_sticky_note_still_draws_its_preview_background() {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();
        {
            let ctx = cairo::Context::new(&surface).unwrap();
            render_sticky_note_preview(
                &ctx,
                40,
                50,
                "",
                Color::new(0.95, 0.9, 0.2, 1.0),
                20.0,
                &FontDescriptor::default(),
                None,
            );
        }
        surface.flush();
        assert!(
            surface.data().unwrap().iter().any(|byte| *byte != 0),
            "an empty note needs a visible background behind its separate caret"
        );
    }

    #[test]
    fn empty_committed_sticky_note_remains_invisible() {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();
        {
            let ctx = cairo::Context::new(&surface).unwrap();
            render_sticky_note(
                &ctx,
                40,
                50,
                "",
                Color::new(0.95, 0.9, 0.2, 1.0),
                20.0,
                &FontDescriptor::default(),
                None,
            );
        }
        surface.flush();
        assert!(
            surface.data().unwrap().iter().all(|byte| *byte == 0),
            "the emptied source shape stays hidden while its live edit preview moves"
        );
    }

    #[test]
    fn live_plain_text_background_covers_a_trailing_space_caret() {
        let text = "A                    ";
        let font = FontDescriptor::default();
        let size = 20.0;
        let origin = (20, 60);
        let caret = crate::draw::shape::caret_geometry_text(
            text,
            &font.to_pango_string(size),
            None,
            text.len(),
        )
        .unwrap();
        let sample_x = origin.0 + caret.x.round() as i32;
        let sample_y = origin.1 + (caret.y_from_baseline + caret.height / 2.0).round() as i32;
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 400, 120).unwrap();
        {
            let ctx = cairo::Context::new(&surface).unwrap();
            render_text(
                &ctx,
                origin.0,
                origin.1,
                text,
                Color::new(1.0, 1.0, 1.0, 1.0),
                size,
                &font,
                true,
                None,
            );
        }

        assert!(
            alpha_at(&mut surface, sample_x, sample_y) > 0,
            "the live background must extend under the end caret after trailing spaces"
        );
    }

    #[test]
    fn live_sticky_note_background_covers_a_trailing_space_caret() {
        let text = "A                    ";
        let font = FontDescriptor::default();
        let size = 20.0;
        let origin = (20, 60);
        let caret = crate::draw::shape::caret_geometry_text(
            text,
            &font.to_pango_string(size),
            None,
            text.len(),
        )
        .unwrap();
        let sample_x = origin.0 + caret.x.round() as i32;
        let sample_y = origin.1 + (caret.y_from_baseline + caret.height / 2.0).round() as i32;
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 400, 120).unwrap();
        {
            let ctx = cairo::Context::new(&surface).unwrap();
            render_sticky_note_preview(
                &ctx,
                origin.0,
                origin.1,
                text,
                Color::new(0.95, 0.9, 0.2, 1.0),
                size,
                &font,
                None,
            );
        }

        assert!(
            alpha_at(&mut surface, sample_x, sample_y) > 0,
            "the live note must extend under the end caret after trailing spaces"
        );
    }

    /// Bounding box of every pixel a render call actually touched.
    fn painted_extents(surface: &mut cairo::ImageSurface) -> Option<(i32, i32, i32, i32)> {
        surface.flush();
        let width = surface.width();
        let height = surface.height();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        let (mut min_x, mut min_y) = (i32::MAX, i32::MAX);
        let (mut max_x, mut max_y) = (i32::MIN, i32::MIN);
        for y in 0..height {
            for x in 0..width {
                if data[y as usize * stride + x as usize * 4 + 3] != 0 {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        (min_x <= max_x).then_some((min_x, min_y, max_x, max_y))
    }

    #[test]
    fn text_damage_box_covers_every_painted_pixel() {
        // `bounding_box_for_text` is what the dirty tracker repaints when a block
        // moves. Anything render_text paints outside it survives the clear and
        // trails behind the drag, so the two must never drift apart.
        let font = FontDescriptor::default();
        for (text, size, background) in [
            ("kako", 32.0, false),
            ("sta ima", 48.0, false),
            ("gjpqy", 24.0, false),
            ("kako", 32.0, true),
            ("Multi\nline", 28.0, false),
        ] {
            let origin = (150, 220);
            let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 600, 420).unwrap();
            {
                let ctx = cairo::Context::new(&surface).unwrap();
                render_text(
                    &ctx,
                    origin.0,
                    origin.1,
                    text,
                    Color::new(1.0, 0.5, 0.1, 1.0),
                    size,
                    &font,
                    background,
                    None,
                );
            }
            let (min_x, min_y, max_x, max_y) =
                painted_extents(&mut surface).expect("text paints something");
            let bounds = crate::draw::shape::bounding_box_for_text(
                origin.0, origin.1, text, size, &font, background, None,
            )
            .expect("non-empty text has damage bounds");
            assert!(
                min_x >= bounds.x
                    && min_y >= bounds.y
                    && max_x < bounds.x + bounds.width
                    && max_y < bounds.y + bounds.height,
                "{text:?} size={size} bg={background}: painted \
                 ({min_x},{min_y})..({max_x},{max_y}) escapes damage box {bounds:?}"
            );
        }
    }

    #[test]
    fn caret_stroke_stays_within_its_advertised_width() {
        // The damage tracker sizes the caret's repaint from `caret_outline_width`;
        // the stroke is centred, so it must not reach further than half of it.
        let font = FontDescriptor::default();
        for size in [16.0_f64, 20.0, 32.0, 48.0] {
            let color = Color::new(1.0, 0.5, 0.1, 1.0);
            let geom =
                crate::draw::shape::caret_geometry_text("hi", &font.to_pango_string(size), None, 1)
                    .unwrap();
            let origin = (150, 220);
            let caret_x = f64::from(origin.0) + geom.x;
            let top = f64::from(origin.1) + geom.y_from_baseline;
            let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 600, 420).unwrap();
            {
                let ctx = cairo::Context::new(&surface).unwrap();
                let outline = text_outline_color(color);
                ctx.set_source_rgba(outline.r, outline.g, outline.b, outline.a);
                ctx.set_line_width(caret_outline_width(size));
                ctx.move_to(caret_x, top);
                ctx.line_to(caret_x, top + geom.height);
                let _ = ctx.stroke();
            }
            let (min_x, _, max_x, _) =
                painted_extents(&mut surface).expect("the caret paints something");
            let half = caret_outline_width(size) / 2.0;
            assert!(
                f64::from(min_x) >= (caret_x - half).floor()
                    && f64::from(max_x) <= (caret_x + half).ceil(),
                "size={size}: caret painted {min_x}..{max_x}, advertised \
                 {}..{}",
                caret_x - half,
                caret_x + half
            );
        }
    }
}
