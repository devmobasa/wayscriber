use super::highlight::render_click_highlight;
use super::primitives::{render_arrow, render_ellipse, render_line, render_rect};
use super::strokes::render_freehand_borrowed;
use crate::draw::frame::DrawnShape;
use crate::draw::shape::{step_marker_outline_thickness, step_marker_radius};
use crate::draw::{Color, Shape};

/// Renders a selection halo overlay for a drawn shape.
pub fn render_selection_halo(ctx: &cairo::Context, drawn: &DrawnShape) {
    let glow = Color {
        r: 0.3,
        g: 0.55,
        b: 1.0,
        a: 0.35,
    };
    let outline_width = 4.0;

    // Ensure halo does not modify primary drawing state.
    let _ = ctx.save();
    match &drawn.shape {
        Shape::Freehand { points, thick, .. } => {
            render_freehand_borrowed(ctx, points, glow, thick + outline_width);
        }
        Shape::FreehandPressure { points, .. } => {
            // For pressure lines, we render the same variable-width line but with extra thickness
            // Split points into coords and thickness
            let coords: Vec<(i32, i32)> = points.iter().map(|&(x, y, _)| (x, y)).collect();
            let thickness: Vec<f32> = points
                .iter()
                .map(|&(_, _, t)| t + outline_width as f32)
                .collect();

            use super::strokes::render_freehand_pressure_borrowed;
            render_freehand_pressure_borrowed(ctx, &coords, &thickness, glow);
        }
        Shape::Line {
            x1,
            y1,
            x2,
            y2,
            thick,
            ..
        } => {
            render_line(ctx, *x1, *y1, *x2, *y2, glow, thick + outline_width);
        }
        Shape::Rect {
            x,
            y,
            w,
            h,
            thick,
            fill,
            ..
        } => {
            render_rect(ctx, *x, *y, *w, *h, *fill, glow, thick + outline_width);
        }
        Shape::Ellipse {
            cx,
            cy,
            rx,
            ry,
            fill,
            thick,
            ..
        } => {
            render_ellipse(ctx, *cx, *cy, *rx, *ry, *fill, glow, thick + outline_width);
        }
        Shape::Arrow {
            x1,
            y1,
            x2,
            y2,
            thick,
            arrow_length,
            arrow_angle,
            head_at_end,
            ..
        } => {
            render_arrow(
                ctx,
                *x1,
                *y1,
                *x2,
                *y2,
                glow,
                thick + outline_width,
                *arrow_length,
                *arrow_angle,
                *head_at_end,
            );
        }
        Shape::MarkerStroke { points, thick, .. } => {
            render_freehand_borrowed(ctx, points, glow, thick + outline_width);
        }
        Shape::StepMarker { x, y, label, .. } => {
            let radius = step_marker_radius(label.value, label.size, &label.font_descriptor);
            let outline = step_marker_outline_thickness(label.size);
            let halo_radius = radius + outline_width;
            let fill = Color {
                a: glow.a * 0.4,
                ..glow
            };
            render_click_highlight(
                ctx,
                *x as f64,
                *y as f64,
                halo_radius,
                outline + outline_width,
                fill,
                glow,
                1.0,
            );
        }
        Shape::EraserStroke { points, brush } => {
            let outline = brush.size + outline_width;
            render_freehand_borrowed(ctx, points, glow, outline);
        }
        Shape::Text { .. } => {
            if let Some(bounds) = drawn.shape.bounding_box() {
                let padding = 4.0;
                let x = bounds.x as f64 - padding;
                let y = bounds.y as f64 - padding;
                let w = bounds.width as f64 + padding * 2.0;
                let h = bounds.height as f64 + padding * 2.0;
                ctx.set_source_rgba(glow.r, glow.g, glow.b, glow.a * 0.6);
                ctx.rectangle(x, y, w, h);
                let _ = ctx.fill();
                ctx.set_source_rgba(glow.r, glow.g, glow.b, glow.a);
                ctx.set_line_width(2.0);
                ctx.rectangle(x, y, w, h);
                let _ = ctx.stroke();
            }
        }
        Shape::StickyNote { .. } => {
            if let Some(bounds) = drawn.shape.bounding_box() {
                let padding = 4.0;
                let x = bounds.x as f64 - padding;
                let y = bounds.y as f64 - padding;
                let w = bounds.width as f64 + padding * 2.0;
                let h = bounds.height as f64 + padding * 2.0;
                ctx.set_source_rgba(glow.r, glow.g, glow.b, glow.a * 0.6);
                ctx.rectangle(x, y, w, h);
                let _ = ctx.fill();
                ctx.set_source_rgba(glow.r, glow.g, glow.b, glow.a);
                ctx.set_line_width(2.0);
                ctx.rectangle(x, y, w, h);
                let _ = ctx.stroke();
            }
        }
    }
    let _ = ctx.restore();
}
