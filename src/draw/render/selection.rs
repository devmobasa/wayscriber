use super::highlight::render_click_highlight;
use super::primitives::{render_arrow, render_ellipse, render_line, render_rect};
use super::strokes::render_freehand_borrowed;
use crate::draw::frame::DrawnShape;
use crate::draw::shape::{step_marker_outline_thickness, step_marker_radius};
use crate::draw::{Color, Shape};
use crate::util::Rect;

/// Selection handle size in pixels
const HANDLE_SIZE: f64 = 8.0;
/// Selection handle border width
const HANDLE_BORDER: f64 = 1.5;

/// Selection accent color (blue)
const SELECTION_COLOR: Color = Color {
    r: 0.3,
    g: 0.55,
    b: 1.0,
    a: 0.9,
};

/// Selection glow color (semi-transparent blue)
const SELECTION_GLOW: Color = Color {
    r: 0.3,
    g: 0.55,
    b: 1.0,
    a: 0.35,
};

/// Renders a selection halo overlay for a drawn shape.
pub fn render_selection_halo(ctx: &cairo::Context, drawn: &DrawnShape) {
    let glow = SELECTION_GLOW;
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

/// Renders selection handles (corner resize handles) for a bounding box.
pub fn render_selection_handles(ctx: &cairo::Context, bounds: &Rect) {
    let _ = ctx.save();

    let x = bounds.x as f64;
    let y = bounds.y as f64;
    let w = bounds.width as f64;
    let h = bounds.height as f64;

    // Draw dashed bounding box
    ctx.set_source_rgba(SELECTION_COLOR.r, SELECTION_COLOR.g, SELECTION_COLOR.b, 0.6);
    ctx.set_line_width(1.0);
    ctx.set_dash(&[4.0, 4.0], 0.0);
    ctx.rectangle(x, y, w, h);
    let _ = ctx.stroke();
    ctx.set_dash(&[], 0.0); // Reset dash

    // Draw corner handles
    let half = HANDLE_SIZE / 2.0;
    let corners = [
        (x, y),         // Top-left
        (x + w, y),     // Top-right
        (x, y + h),     // Bottom-left
        (x + w, y + h), // Bottom-right
    ];

    for (cx, cy) in corners {
        // Handle fill (white)
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.rectangle(cx - half, cy - half, HANDLE_SIZE, HANDLE_SIZE);
        let _ = ctx.fill();

        // Handle border (selection blue)
        ctx.set_source_rgba(
            SELECTION_COLOR.r,
            SELECTION_COLOR.g,
            SELECTION_COLOR.b,
            SELECTION_COLOR.a,
        );
        ctx.set_line_width(HANDLE_BORDER);
        ctx.rectangle(cx - half, cy - half, HANDLE_SIZE, HANDLE_SIZE);
        let _ = ctx.stroke();
    }

    // Draw edge midpoint handles (for non-proportional resize)
    let edge_midpoints = [
        (x + w / 2.0, y),     // Top center
        (x + w / 2.0, y + h), // Bottom center
        (x, y + h / 2.0),     // Left center
        (x + w, y + h / 2.0), // Right center
    ];

    let edge_handle_size = HANDLE_SIZE * 0.75;
    let edge_half = edge_handle_size / 2.0;

    for (ex, ey) in edge_midpoints {
        // Handle fill (white)
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        ctx.rectangle(
            ex - edge_half,
            ey - edge_half,
            edge_handle_size,
            edge_handle_size,
        );
        let _ = ctx.fill();

        // Handle border (selection blue, slightly lighter)
        ctx.set_source_rgba(SELECTION_COLOR.r, SELECTION_COLOR.g, SELECTION_COLOR.b, 0.7);
        ctx.set_line_width(HANDLE_BORDER);
        ctx.rectangle(
            ex - edge_half,
            ey - edge_half,
            edge_handle_size,
            edge_handle_size,
        );
        let _ = ctx.stroke();
    }

    let _ = ctx.restore();
}

/// Computes selection handle rectangles for hit testing.
/// Returns handles in order: TL, TR, BL, BR, Top, Bottom, Left, Right
#[allow(dead_code)]
pub fn selection_handle_rects(bounds: &Rect) -> [Rect; 8] {
    let x = bounds.x;
    let y = bounds.y;
    let w = bounds.width;
    let h = bounds.height;
    let size = HANDLE_SIZE as i32;
    let half = size / 2;
    let edge_size = (HANDLE_SIZE * 0.75) as i32;
    let edge_half = edge_size / 2;

    [
        // Corner handles
        Rect::new(x - half, y - half, size, size).unwrap_or(Rect {
            x,
            y,
            width: size,
            height: size,
        }),
        Rect::new(x + w - half, y - half, size, size).unwrap_or(Rect {
            x: x + w,
            y,
            width: size,
            height: size,
        }),
        Rect::new(x - half, y + h - half, size, size).unwrap_or(Rect {
            x,
            y: y + h,
            width: size,
            height: size,
        }),
        Rect::new(x + w - half, y + h - half, size, size).unwrap_or(Rect {
            x: x + w,
            y: y + h,
            width: size,
            height: size,
        }),
        // Edge handles
        Rect::new(x + w / 2 - edge_half, y - edge_half, edge_size, edge_size).unwrap_or(Rect {
            x: x + w / 2,
            y,
            width: edge_size,
            height: edge_size,
        }),
        Rect::new(
            x + w / 2 - edge_half,
            y + h - edge_half,
            edge_size,
            edge_size,
        )
        .unwrap_or(Rect {
            x: x + w / 2,
            y: y + h,
            width: edge_size,
            height: edge_size,
        }),
        Rect::new(x - edge_half, y + h / 2 - edge_half, edge_size, edge_size).unwrap_or(Rect {
            x,
            y: y + h / 2,
            width: edge_size,
            height: edge_size,
        }),
        Rect::new(
            x + w - edge_half,
            y + h / 2 - edge_half,
            edge_size,
            edge_size,
        )
        .unwrap_or(Rect {
            x: x + w,
            y: y + h / 2,
            width: edge_size,
            height: edge_size,
        }),
    ]
}
