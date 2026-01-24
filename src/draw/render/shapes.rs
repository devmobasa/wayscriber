use super::highlight::render_click_highlight;
use super::primitives::{render_arrow, render_ellipse, render_line, render_rect};
use super::strokes::{
    render_freehand_borrowed, render_freehand_pressure_borrowed, render_marker_stroke_borrowed,
};
use super::text::{render_sticky_note, render_text};
use crate::draw::shape::Shape;
use crate::draw::shape::{
    ARROW_LABEL_BACKGROUND, arrow_label_layout, measure_text_with_context,
    step_marker_outline_thickness, step_marker_radius,
};
use crate::draw::Color;

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
            label,
        } => {
            let (tip_x, tip_y, tail_x, tail_y) = if *head_at_end {
                (*x2, *y2, *x1, *y1)
            } else {
                (*x1, *y1, *x2, *y2)
            };
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
            );
            if let Some(label) = label {
                let label_text = label.value.to_string();
                if let Some(layout) = arrow_label_layout(
                    tip_x,
                    tip_y,
                    tail_x,
                    tail_y,
                    *thick,
                    &label_text,
                    label.size,
                    &label.font_descriptor,
                ) {
                    render_text(
                        ctx,
                        layout.x,
                        layout.y,
                        &label_text,
                        *color,
                        label.size,
                        &label.font_descriptor,
                        ARROW_LABEL_BACKGROUND,
                        None,
                    );
                }
            }
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
            render_text(
                ctx,
                *x,
                *y,
                text,
                *color,
                *size,
                font_descriptor,
                *background_enabled,
                *wrap_width,
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
            let brightness = color.r * 0.299 + color.g * 0.587 + color.b * 0.114;
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
                render_text(
                    ctx,
                    baseline_x,
                    baseline_y,
                    &label_text,
                    text_color,
                    label.size,
                    &label.font_descriptor,
                    false,
                    None,
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
    }
}
