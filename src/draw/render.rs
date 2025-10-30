//! Cairo-based rendering functions for shapes.

use super::color::Color;
use super::shape::Shape;
use crate::config::BoardConfig;
use crate::input::BoardMode;
use crate::util;

/// Renders board background for whiteboard/blackboard modes.
///
/// This function fills the entire canvas with a solid color when in
/// whiteboard or blackboard mode. For transparent mode, it does nothing
/// (background remains transparent).
///
/// Should be called after clearing the canvas but before rendering shapes.
///
/// # Arguments
/// * `ctx` - Cairo drawing context to render to
/// * `mode` - Current board mode
/// * `config` - Board configuration with color settings
pub fn render_board_background(ctx: &cairo::Context, mode: BoardMode, config: &BoardConfig) {
    if let Some(bg_color) = mode.background_color(config) {
        ctx.set_source_rgba(bg_color.r, bg_color.g, bg_color.b, bg_color.a);
        let _ = ctx.paint(); // Ignore errors - if paint fails, we'll just have transparent bg
    }
    // If None (Transparent mode), do nothing - background stays transparent
}

/// Renders all shapes in a collection to a Cairo context.
///
/// Iterates through the shapes slice and renders each one in order.
/// Shapes are drawn in the order they appear (first shape = bottom layer).
///
/// # Arguments
/// * `ctx` - Cairo drawing context to render to
/// * `shapes` - Slice of shapes to render
pub fn render_shapes(ctx: &cairo::Context, shapes: &[Shape]) {
    for shape in shapes {
        render_shape(ctx, shape);
    }
}

/// Renders a single shape to a Cairo context.
///
/// Dispatches to the appropriate internal rendering function based on shape type.
/// Handles all shape variants: Freehand, Line, Rect, Ellipse, Arrow, and Text.
///
/// # Arguments
/// * `ctx` - Cairo drawing context to render to
/// * `shape` - The shape to render
pub fn render_shape(ctx: &cairo::Context, shape: &Shape) {
    match shape {
        Shape::Freehand {
            points,
            color,
            thick,
        } => {
            render_freehand_borrowed(ctx, points, *color, *thick);
        }
        Shape::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            thick,
        } => {
            render_line(ctx, *x1, *y1, *x2, *y2, *color, *thick);
        }
        Shape::Rect {
            x,
            y,
            w,
            h,
            color,
            thick,
        } => {
            render_rect(ctx, *x, *y, *w, *h, *color, *thick);
        }
        Shape::Ellipse {
            cx,
            cy,
            rx,
            ry,
            color,
            thick,
        } => {
            render_ellipse(ctx, *cx, *cy, *rx, *ry, *color, *thick);
        }
        Shape::Arrow {
            x1,
            y1,
            x2,
            y2,
            color,
            thick,
            arrow_length,
            arrow_angle,
        } => {
            render_arrow(
                ctx,
                *x1,
                *y1,
                *x2,
                *y2,
                *color,
                *thick,
                *arrow_length,
                *arrow_angle,
            );
        }
        Shape::Text {
            x,
            y,
            text,
            color,
            size,
            font_descriptor,
            background_enabled,
        } => {
            render_text(
                ctx,
                *x,
                *y,
                text,
                *color,
                *size,
                font_descriptor,
                *background_enabled,
            );
        }
    }
}

/// Renders a circular click highlight with configurable fill/outline colors.
pub fn render_click_highlight(
    ctx: &cairo::Context,
    center_x: f64,
    center_y: f64,
    radius: f64,
    outline_thickness: f64,
    fill_color: Color,
    outline_color: Color,
    opacity: f64,
) {
    if opacity <= 0.0 {
        return;
    }

    let alpha = opacity.clamp(0.0, 1.0);
    let radius = radius.max(1.0);
    let _ = ctx.save();

    if fill_color.a > 0.0 {
        ctx.set_source_rgba(
            fill_color.r,
            fill_color.g,
            fill_color.b,
            fill_color.a * alpha,
        );
        ctx.arc(center_x, center_y, radius, 0.0, std::f64::consts::PI * 2.0);
        let _ = ctx.fill();
    }

    if outline_color.a > 0.0 && outline_thickness > 0.0 {
        ctx.set_source_rgba(
            outline_color.r,
            outline_color.g,
            outline_color.b,
            outline_color.a * alpha,
        );
        ctx.set_line_width(outline_thickness);
        ctx.arc(center_x, center_y, radius, 0.0, std::f64::consts::PI * 2.0);
        let _ = ctx.stroke();
    }

    let _ = ctx.restore();
}

/// Render freehand stroke (polyline through points)
///
/// This function accepts a borrowed slice, avoiding clones for better performance.
/// Use this for rendering provisional shapes during drawing to prevent quadratic behavior.
pub fn render_freehand_borrowed(
    ctx: &cairo::Context,
    points: &[(i32, i32)],
    color: Color,
    thick: f64,
) {
    if points.is_empty() {
        return;
    }

    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Start at first point
    let (x0, y0) = points[0];
    ctx.move_to(x0 as f64, y0 as f64);

    // Draw lines through all points
    for &(x, y) in &points[1..] {
        ctx.line_to(x as f64, y as f64);
    }

    let _ = ctx.stroke();
}

/// Render a straight line
fn render_line(ctx: &cairo::Context, x1: i32, y1: i32, x2: i32, y2: i32, color: Color, thick: f64) {
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.set_line_cap(cairo::LineCap::Round);

    ctx.move_to(x1 as f64, y1 as f64);
    ctx.line_to(x2 as f64, y2 as f64);
    let _ = ctx.stroke();
}

/// Render a rectangle (outline)
fn render_rect(ctx: &cairo::Context, x: i32, y: i32, w: i32, h: i32, color: Color, thick: f64) {
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.set_line_join(cairo::LineJoin::Miter);

    // Normalize rectangle to handle any legacy data with negative dimensions
    // (InputState already normalizes, but this ensures consistent rendering)
    let (norm_x, norm_w) = if w >= 0 {
        (x as f64, w as f64)
    } else {
        ((x + w) as f64, (-w) as f64)
    };
    let (norm_y, norm_h) = if h >= 0 {
        (y as f64, h as f64)
    } else {
        ((y + h) as f64, (-h) as f64)
    };

    ctx.rectangle(norm_x, norm_y, norm_w, norm_h);
    let _ = ctx.stroke();
}

/// Render an ellipse using Cairo's arc with scaling
fn render_ellipse(
    ctx: &cairo::Context,
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    color: Color,
    thick: f64,
) {
    if rx == 0 || ry == 0 {
        return;
    }

    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);

    ctx.save().ok();
    ctx.translate(cx as f64, cy as f64);
    ctx.scale(rx as f64, ry as f64);
    ctx.arc(0.0, 0.0, 1.0, 0.0, 2.0 * std::f64::consts::PI);
    ctx.restore().ok();

    let _ = ctx.stroke();
}

/// Render an arrow (line with arrowhead pointing towards start)
#[allow(clippy::too_many_arguments)]
fn render_arrow(
    ctx: &cairo::Context,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    color: Color,
    thick: f64,
    arrow_length: f64,
    arrow_angle: f64,
) {
    // Draw the main line
    render_line(ctx, x1, y1, x2, y2, color, thick);

    // Draw arrowhead at (x1, y1) pointing towards start
    // Returns [left_point, right_point]
    let arrow_points = util::calculate_arrowhead_custom(x1, y1, x2, y2, arrow_length, arrow_angle);

    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Draw left line of arrowhead (from start to left point)
    ctx.move_to(x1 as f64, y1 as f64);
    ctx.line_to(arrow_points[0].0, arrow_points[0].1);
    let _ = ctx.stroke();

    // Draw right line of arrowhead (from start to right point)
    ctx.move_to(x1 as f64, y1 as f64);
    ctx.line_to(arrow_points[1].0, arrow_points[1].1);
    let _ = ctx.stroke();
}

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
    font_descriptor: &super::FontDescriptor,
    background_enabled: bool,
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

/// Fills the entire surface with a semi-transparent tinted background.
///
/// Creates a barely visible dark tint (0.05 alpha) to confirm the overlay is active
/// without obscuring the screen content. This function is kept for potential future use.
///
/// # Arguments
/// * `ctx` - Cairo drawing context to fill
/// * `width` - Surface width in pixels
/// * `height` - Surface height in pixels
#[allow(dead_code)]
pub fn fill_transparent(ctx: &cairo::Context, width: i32, height: i32) {
    // Use a very slight tint so we can see the overlay is there
    // 0.05 alpha = barely visible, just enough to confirm it's working
    ctx.set_source_rgba(0.1, 0.1, 0.1, 0.05);
    ctx.set_operator(cairo::Operator::Source);
    ctx.rectangle(0.0, 0.0, width as f64, height as f64);
    let _ = ctx.fill();
}
