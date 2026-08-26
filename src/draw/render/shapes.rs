use super::blur::{render_black_out_rect, render_blur_placeholder};
use super::highlight::render_click_highlight;
use super::image::render_image_shape;
use super::pressure_strokes::render_freehand_pressure_borrowed;
use super::primitives::{render_arrow, render_ellipse, render_line, render_polygon, render_rect};
use super::strokes::{render_freehand_borrowed, render_marker_stroke_borrowed};
use super::text::{render_sticky_note, render_text_over};
use crate::draw::Color;
use crate::draw::shape::Shape;
use crate::draw::shape::{
    ARROW_LABEL_BACKGROUND, arrow_label_ends, arrow_label_layout, measure_text_with_context,
    step_marker_outline_thickness, step_marker_radius,
};

/// Renders a single shape to a Cairo context.
///
/// Dispatches to the appropriate internal rendering function based on shape type.
/// Handles all shape variants: Freehand, Line, Rect, Ellipse, Arrow, and Text.
///
/// # Arguments
/// * `ctx` - Cairo drawing context to render to
/// * `shape` - The shape to render
pub fn render_shape(ctx: &cairo::Context, shape: &Shape) {
    render_shape_over(ctx, shape, None);
}

/// `render_shape`, plus what the caller knows about the background behind it.
///
/// Only text reads it, and only when the target cannot be probed. See
/// [`crate::draw::render_text_over`].
pub fn render_shape_over(
    ctx: &cairo::Context,
    shape: &Shape,
    known_background_luminance: Option<f64>,
) {
    match shape {
        Shape::Freehand {
            points,
            color,
            thick,
        } => {
            render_freehand_borrowed(ctx, points, *color, *thick);
        }
        Shape::FreehandPressure { points, color } => {
            let coords: Vec<(i32, i32)> = points.iter().map(|&(x, y, _)| (x, y)).collect();
            let thickness: Vec<f32> = points.iter().map(|&(_, _, t)| t).collect();
            render_freehand_pressure_borrowed(ctx, &coords, &thickness, *color);
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
            fill,
            color,
            thick,
        } => {
            render_rect(ctx, *x, *y, *w, *h, *fill, *color, *thick);
        }
        Shape::Ellipse {
            cx,
            cy,
            rx,
            ry,
            fill,
            color,
            thick,
        } => {
            render_ellipse(ctx, *cx, *cy, *rx, *ry, *fill, *color, *thick);
        }
        Shape::Polygon {
            points,
            fill,
            color,
            thick,
            ..
        } => {
            render_polygon(ctx, points, *fill, *color, *thick);
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
            head_at_end,
            style,
            bend,
            label,
        } => {
            // Only the label needs these: `render_arrow` reads `head_at_end`
            // itself. `Double` deliberately ignores the flag here, matching the
            // outline it draws either way.
            let (tip_x, tip_y, tail_x, tail_y) =
                arrow_label_ends(*x1, *y1, *x2, *y2, *head_at_end, *style);
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
                *head_at_end,
                *style,
                *bend,
            );
            if let Some(label) = label {
                let label_text = label.value.to_string();
                if let Some(layout) = arrow_label_layout(
                    tip_x,
                    tip_y,
                    tail_x,
                    tail_y,
                    *thick,
                    style.effective_bend(*bend),
                    &label_text,
                    label.size,
                    &label.font_descriptor,
                ) {
                    render_text_over(
                        ctx,
                        layout.x,
                        layout.y,
                        &label_text,
                        *color,
                        label.size,
                        &label.font_descriptor,
                        ARROW_LABEL_BACKGROUND,
                        None,
                        known_background_luminance,
                    );
                }
            }
        }
        Shape::BlurRect {
            x,
            y,
            w,
            h,
            strength: _,
            style,
        } => {
            // Without a captured backdrop the sampling styles can only stand in
            // with a placeholder, but a black out is already its final form.
            if style.needs_backdrop() {
                render_blur_placeholder(ctx, *x, *y, *w, *h, false);
            } else {
                render_black_out_rect(ctx, *x, *y, *w, *h);
            }
        }
        Shape::Spotlight { .. } => {
            // Nothing to draw per-shape: the spotlight pass paints one dim layer
            // for every region at once, before annotations are drawn.
        }
        Shape::Text {
            x,
            y,
            text,
            color,
            size,
            font_descriptor,
            background_enabled,
            wrap_width,
        } => {
            render_text_over(
                ctx,
                *x,
                *y,
                text,
                *color,
                *size,
                font_descriptor,
                *background_enabled,
                *wrap_width,
                known_background_luminance,
            );
        }
        Shape::StepMarker { x, y, color, label } => {
            let label_text = label.value.to_string();
            let radius = step_marker_radius(label.value, label.size, &label.font_descriptor);
            let outline_thickness = step_marker_outline_thickness(label.size);
            let alpha = color.a.clamp(0.0, 1.0);
            let fill_color = Color {
                a: (alpha * 0.9).clamp(0.0, 1.0),
                ..*color
            };
            let brightness = super::color_luminance(*color);
            let (outline_color, text_color) = if brightness > 0.6 {
                (
                    Color {
                        r: 0.05,
                        g: 0.05,
                        b: 0.05,
                        a: 0.85 * alpha,
                    },
                    Color {
                        r: 0.12,
                        g: 0.12,
                        b: 0.12,
                        a: alpha,
                    },
                )
            } else {
                (
                    Color {
                        r: 0.98,
                        g: 0.98,
                        b: 0.98,
                        a: 0.9 * alpha,
                    },
                    Color {
                        r: 0.98,
                        g: 0.98,
                        b: 0.98,
                        a: alpha,
                    },
                )
            };
            render_click_highlight(
                ctx,
                *x as f64,
                *y as f64,
                radius,
                outline_thickness,
                fill_color,
                outline_color,
                1.0,
            );
            let font_desc = label.font_descriptor.to_pango_string(label.size);
            if let Some(metrics) =
                measure_text_with_context(ctx, &label_text, &font_desc, label.size, None)
            {
                let center_offset_x = metrics.ink_x + metrics.ink_width / 2.0;
                let center_offset_y = metrics.ink_y + metrics.ink_height / 2.0;
                let baseline_x = (*x as f64 - center_offset_x).round() as i32;
                let baseline_y = (*y as f64 - center_offset_y + metrics.baseline).round() as i32;
                render_text_over(
                    ctx,
                    baseline_x,
                    baseline_y,
                    &label_text,
                    text_color,
                    label.size,
                    &label.font_descriptor,
                    false,
                    None,
                    known_background_luminance,
                );
            }
        }
        Shape::StickyNote {
            x,
            y,
            text,
            background,
            size,
            font_descriptor,
            wrap_width,
        } => {
            render_sticky_note(
                ctx,
                *x,
                *y,
                text,
                *background,
                *size,
                font_descriptor,
                *wrap_width,
            );
        }
        Shape::MarkerStroke {
            points,
            color,
            thick,
        } => {
            render_marker_stroke_borrowed(ctx, points, *color, *thick);
        }
        Shape::EraserStroke { .. } => {
            // Eraser strokes require an eraser replay context; ignore in generic rendering.
        }
        Shape::Image { x, y, w, h, data } => {
            render_image_shape(ctx, *x, *y, *w, *h, data);
        }
    }
}
