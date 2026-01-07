use super::primitives::{render_arrow, render_ellipse, render_line, render_rect};
use super::strokes::{
    render_eraser_stroke, render_freehand_borrowed, render_marker_stroke_borrowed,
};
use super::text::{render_sticky_note, render_text};
use super::types::EraserReplayContext;
use crate::draw::frame::DrawnShape;
use crate::draw::shape::Shape;
use crate::draw::shape::{ARROW_LABEL_BACKGROUND, arrow_label_layout};

/// Renders all shapes in a collection to a Cairo context.
///
/// Iterates through the shapes slice and renders each one in order.
/// Shapes are drawn in the order they appear (first shape = bottom layer).
///
/// # Arguments
/// * `ctx` - Cairo drawing context to render to
/// * `shapes` - Slice of shapes to render
/// * `eraser_ctx` - Optional eraser replay context (required to render eraser strokes)
pub fn render_shapes(
    ctx: &cairo::Context,
    shapes: &[DrawnShape],
    eraser_ctx: Option<&EraserReplayContext>,
) {
    for drawn in shapes {
        match &drawn.shape {
            Shape::EraserStroke { points, brush } => {
                if let Some(ctx_eraser) = eraser_ctx {
                    render_eraser_stroke(ctx, points, brush, ctx_eraser);
                }
            }
            other => render_shape(ctx, other),
        }
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
