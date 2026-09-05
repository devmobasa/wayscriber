use super::blur::{render_black_out_rect, render_blur_placeholder};
use super::highlight::render_click_highlight;
use super::image::render_image_shape;
use super::pressure_strokes::render_packed_freehand_pressure_borrowed;
use super::primitives::{render_arrow, render_ellipse, render_line, render_polygon, render_rect};
use super::strokes::{render_freehand_borrowed, render_marker_stroke_borrowed};
use super::text::{render_sticky_note_with_measurer, render_text_over_with_halo_with_measurer};
use crate::draw::Color;
use crate::draw::shape::{
    ARROW_LABEL_BACKGROUND, ArrowLabel, ArrowStyle, Shape, StepMarkerLabel, arrow_label_ends,
    arrow_label_layout_with, step_marker_outline_thickness, step_marker_radius_with,
};

#[derive(Clone, Copy)]
struct ShapeTextOptions {
    known_background_luminance: Option<f64>,
    halo_enabled: bool,
}

struct ArrowRenderSpec<'a> {
    start: (i32, i32),
    end: (i32, i32),
    color: Color,
    thickness: f64,
    arrow_length: f64,
    arrow_angle: f64,
    head_at_end: bool,
    style: ArrowStyle,
    bend: f64,
    label: Option<&'a ArrowLabel>,
}

struct StepMarkerRenderSpec<'a> {
    center: (i32, i32),
    color: Color,
    label: &'a StepMarkerLabel,
}

/// Renders a single shape to a Cairo context.
///
/// Dispatches to the appropriate internal rendering function based on shape type.
/// Handles all shape variants: Freehand, Line, Rect, Ellipse, Arrow, and Text.
///
/// This convenience entry point uses temporary drawing caches. Repeated painting
/// should use [`super::RenderCtx`] with a persistent [`super::RenderCaches`] owner.
///
/// # Arguments
/// * `ctx` - Cairo drawing context to render to
/// * `shape` - The shape to render
pub fn render_shape(ctx: &cairo::Context, shape: &Shape) {
    render_shape_with_halo(ctx, shape, true);
}

/// [`render_shape`], with explicit control over text outlines within the shape.
pub fn render_shape_with_halo(ctx: &cairo::Context, shape: &Shape, text_halo_enabled: bool) {
    render_shape_over_with_halo(ctx, shape, None, text_halo_enabled);
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
    render_shape_over_with_halo(ctx, shape, known_background_luminance, true);
}

/// [`render_shape_over`], with explicit control over text outlines.
pub fn render_shape_over_with_halo(
    ctx: &cairo::Context,
    shape: &Shape,
    known_background_luminance: Option<f64>,
    text_halo_enabled: bool,
) {
    super::RenderCtx::new(ctx, &mut super::RenderCaches::default()).render_shape_over_with_halo(
        shape,
        known_background_luminance,
        text_halo_enabled,
    );
}

pub(super) fn render_shape_with_cache(
    measurer: &crate::draw::TextMeasurer,
    images: &mut super::image::ImageSurfaceCache,
    ctx: &cairo::Context,
    shape: &Shape,
    known_background_luminance: Option<f64>,
    text_halo_enabled: bool,
) {
    let text_options = ShapeTextOptions {
        known_background_luminance,
        halo_enabled: text_halo_enabled,
    };
    match shape {
        Shape::Freehand {
            points,
            color,
            thick,
        } => {
            render_freehand_borrowed(ctx, points, *color, *thick);
        }
        Shape::FreehandPressure { points, color } => {
            render_packed_freehand_pressure_borrowed(ctx, points, 0.0, *color);
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
            render_arrow_shape(
                measurer,
                ctx,
                ArrowRenderSpec {
                    start: (*x1, *y1),
                    end: (*x2, *y2),
                    color: *color,
                    thickness: *thick,
                    arrow_length: *arrow_length,
                    arrow_angle: *arrow_angle,
                    head_at_end: *head_at_end,
                    style: *style,
                    bend: *bend,
                    label: label.as_ref(),
                },
                text_options,
            );
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
            render_text_over_with_halo_with_measurer(
                measurer,
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
                text_halo_enabled,
            );
        }
        Shape::StepMarker { x, y, color, label } => {
            render_step_marker_shape(
                measurer,
                ctx,
                StepMarkerRenderSpec {
                    center: (*x, *y),
                    color: *color,
                    label,
                },
                text_options,
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
            render_sticky_note_with_measurer(
                measurer,
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
            render_image_shape(images, ctx, *x, *y, *w, *h, data);
        }
    }
}

fn render_arrow_shape(
    measurer: &crate::draw::TextMeasurer,
    ctx: &cairo::Context,
    arrow: ArrowRenderSpec<'_>,
    text: ShapeTextOptions,
) {
    // Only the label needs these: `render_arrow` reads `head_at_end` itself.
    // `Double` deliberately ignores the flag here, matching the outline it
    // draws either way.
    let (tip_x, tip_y, tail_x, tail_y) = arrow_label_ends(
        arrow.start.0,
        arrow.start.1,
        arrow.end.0,
        arrow.end.1,
        arrow.head_at_end,
        arrow.style,
    );
    render_arrow(
        ctx,
        arrow.start.0,
        arrow.start.1,
        arrow.end.0,
        arrow.end.1,
        arrow.color,
        arrow.thickness,
        arrow.arrow_length,
        arrow.arrow_angle,
        arrow.head_at_end,
        arrow.style,
        arrow.bend,
    );
    let Some(label) = arrow.label else {
        return;
    };
    let label_text = label.value.to_string();
    let Some(layout) = arrow_label_layout_with(
        measurer,
        tip_x,
        tip_y,
        tail_x,
        tail_y,
        arrow.thickness,
        arrow.style.effective_bend(arrow.bend),
        &label_text,
        label.size,
        &label.font_descriptor,
    ) else {
        return;
    };
    render_text_over_with_halo_with_measurer(
        measurer,
        ctx,
        layout.x,
        layout.y,
        &label_text,
        arrow.color,
        label.size,
        &label.font_descriptor,
        ARROW_LABEL_BACKGROUND,
        None,
        text.known_background_luminance,
        text.halo_enabled,
    );
}

fn render_step_marker_shape(
    measurer: &crate::draw::TextMeasurer,
    ctx: &cairo::Context,
    marker: StepMarkerRenderSpec<'_>,
    text: ShapeTextOptions,
) {
    let label_text = marker.label.value.to_string();
    let radius = step_marker_radius_with(
        measurer,
        marker.label.value,
        marker.label.size,
        &marker.label.font_descriptor,
    );
    let outline_thickness = step_marker_outline_thickness(marker.label.size);
    let alpha = marker.color.a.clamp(0.0, 1.0);
    let fill_color = Color {
        a: (alpha * 0.9).clamp(0.0, 1.0),
        ..marker.color
    };
    let brightness = super::color_luminance(marker.color);
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
        f64::from(marker.center.0),
        f64::from(marker.center.1),
        radius,
        outline_thickness,
        fill_color,
        outline_color,
        1.0,
    );
    let font_desc = marker
        .label
        .font_descriptor
        .to_pango_string(marker.label.size);
    let Some(metrics) = measurer.measure(&label_text, &font_desc, marker.label.size, None) else {
        return;
    };
    let center_offset_x = metrics.ink_x + metrics.ink_width / 2.0;
    let center_offset_y = metrics.ink_y + metrics.ink_height / 2.0;
    let baseline_x = (f64::from(marker.center.0) - center_offset_x).round() as i32;
    let baseline_y =
        (f64::from(marker.center.1) - center_offset_y + metrics.baseline).round() as i32;
    render_text_over_with_halo_with_measurer(
        measurer,
        ctx,
        baseline_x,
        baseline_y,
        &label_text,
        text_color,
        marker.label.size,
        &marker.label.font_descriptor,
        false,
        None,
        text.known_background_luminance,
        text.halo_enabled,
    );
}

#[cfg(test)]
mod tests {
    use super::render_shape_with_halo;
    use crate::draw::{ArrowLabel, ArrowStyle, Color, FontDescriptor, Shape, StepMarkerLabel};

    fn rendered_pixels(shape: &Shape, text_halo_enabled: bool) -> Vec<u8> {
        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, 500, 220).expect("shape surface");
        {
            let ctx = cairo::Context::new(&surface).expect("shape context");
            ctx.set_source_rgb(1.0, 1.0, 1.0);
            ctx.paint().expect("white backdrop");
            render_shape_with_halo(&ctx, shape, text_halo_enabled);
        }
        surface.flush();
        surface.data().expect("shape pixels").to_vec()
    }

    #[test]
    fn labelled_shapes_honour_the_text_halo_setting() {
        let red = Color::new(0.96, 0.2, 0.25, 1.0);
        let font_descriptor = FontDescriptor::default();
        let cases = [
            (
                "arrow label",
                Shape::Arrow {
                    x1: 60,
                    y1: 110,
                    x2: 440,
                    y2: 110,
                    color: red,
                    thick: 4.0,
                    arrow_length: 20.0,
                    arrow_angle: 30.0,
                    head_at_end: true,
                    style: ArrowStyle::Standard,
                    bend: 0.0,
                    label: Some(ArrowLabel {
                        value: 7,
                        size: 36.0,
                        font_descriptor: font_descriptor.clone(),
                    }),
                },
            ),
            (
                "step-marker label",
                Shape::StepMarker {
                    x: 250,
                    y: 110,
                    color: red,
                    label: StepMarkerLabel {
                        value: 8,
                        size: 36.0,
                        font_descriptor,
                    },
                },
            ),
        ];

        for (name, shape) in cases {
            assert!(
                rendered_pixels(&shape, true) != rendered_pixels(&shape, false),
                "{name} must forward the halo setting to its text renderer",
            );
        }
    }
}
