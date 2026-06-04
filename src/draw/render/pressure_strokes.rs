use crate::draw::Color;

const PRESSURE_STROKE_MAX_SAMPLE_SPACING: f64 = 4.0;
const PRESSURE_STROKE_MAX_SUBDIVISIONS: usize = 128;
const PRESSURE_STROKE_MIN_WIDTH: f64 = 0.1;
const PRESSURE_STROKE_EPSILON: f64 = 0.000_001;

#[derive(Clone, Copy, Debug)]
struct PressureStrokeSample {
    x: f64,
    y: f64,
    radius: f64,
}

#[derive(Clone, Copy, Debug)]
struct PressureStrokeSegment {
    left_start: (f64, f64),
    left_end: (f64, f64),
    right_end: (f64, f64),
    right_start: (f64, f64),
}

impl PressureStrokeSegment {
    fn from_samples(start: PressureStrokeSample, end: PressureStrokeSample) -> Option<Self> {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length = (dx * dx + dy * dy).sqrt();
        if length <= PRESSURE_STROKE_EPSILON {
            return None;
        }

        let nx = -dy / length;
        let ny = dx / length;
        Some(Self {
            left_start: (start.x + nx * start.radius, start.y + ny * start.radius),
            left_end: (end.x + nx * end.radius, end.y + ny * end.radius),
            right_end: (end.x - nx * end.radius, end.y - ny * end.radius),
            right_start: (start.x - nx * start.radius, start.y - ny * start.radius),
        })
    }

    #[cfg(test)]
    fn is_valid(&self) -> bool {
        let finite_points = [
            self.left_start,
            self.left_end,
            self.right_end,
            self.right_start,
        ]
        .into_iter()
        .all(|(x, y)| x.is_finite() && y.is_finite());

        finite_points
            && polygon_area(&[
                self.left_start,
                self.left_end,
                self.right_end,
                self.right_start,
            ])
            .abs()
                > PRESSURE_STROKE_EPSILON
    }
}

fn pressure_stroke_samples(
    points: &[(i32, i32)],
    thicknesses: &[f32],
) -> Vec<PressureStrokeSample> {
    let len = points.len().min(thicknesses.len());
    let mut samples = Vec::with_capacity(len);

    for i in 0..len {
        let (x, y) = points[i];
        let width = (thicknesses[i] as f64).max(PRESSURE_STROKE_MIN_WIDTH);
        let current = PressureStrokeSample {
            x: x as f64,
            y: y as f64,
            radius: width / 2.0,
        };

        let Some(previous) = samples.last().copied() else {
            samples.push(current);
            continue;
        };

        let dx = current.x - previous.x;
        let dy = current.y - previous.y;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance <= PRESSURE_STROKE_EPSILON {
            samples.pop();
            samples.push(current);
            continue;
        }

        let subdivisions = ((distance / PRESSURE_STROKE_MAX_SAMPLE_SPACING).ceil() as usize)
            .clamp(1, PRESSURE_STROKE_MAX_SUBDIVISIONS);
        for step in 1..=subdivisions {
            let t = step as f64 / subdivisions as f64;
            samples.push(PressureStrokeSample {
                x: previous.x + dx * t,
                y: previous.y + dy * t,
                radius: previous.radius + (current.radius - previous.radius) * t,
            });
        }
    }

    samples
}

#[cfg(test)]
fn pressure_stroke_segments(
    points: &[(i32, i32)],
    thicknesses: &[f32],
) -> Vec<PressureStrokeSegment> {
    let samples = pressure_stroke_samples(points, thicknesses);
    pressure_stroke_segments_from_samples(&samples)
}

#[cfg(test)]
fn pressure_stroke_segments_from_samples(
    samples: &[PressureStrokeSample],
) -> Vec<PressureStrokeSegment> {
    samples
        .windows(2)
        .filter_map(|window| PressureStrokeSegment::from_samples(window[0], window[1]))
        .collect()
}

fn fill_pressure_segment(ctx: &cairo::Context, segment: &PressureStrokeSegment) {
    ctx.new_path();
    ctx.move_to(segment.left_start.0, segment.left_start.1);
    ctx.line_to(segment.left_end.0, segment.left_end.1);
    ctx.line_to(segment.right_end.0, segment.right_end.1);
    ctx.line_to(segment.right_start.0, segment.right_start.1);
    ctx.close_path();
    ctx.fill().ok();
}

fn fill_pressure_sample(ctx: &cairo::Context, sample: &PressureStrokeSample) {
    ctx.new_path();
    ctx.arc(
        sample.x,
        sample.y,
        sample.radius,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    ctx.fill().ok();
}

fn fill_pressure_geometry(ctx: &cairo::Context, samples: &[PressureStrokeSample]) {
    for window in samples.windows(2) {
        if let Some(segment) = PressureStrokeSegment::from_samples(window[0], window[1]) {
            fill_pressure_segment(ctx, &segment);
        }
    }

    for sample in samples {
        fill_pressure_sample(ctx, sample);
    }
}

fn pressure_stroke_preview_samples(
    points: &[(i32, i32)],
    thicknesses: &[f32],
) -> Vec<PressureStrokeSample> {
    let len = points.len().min(thicknesses.len());
    let mut samples = Vec::with_capacity(len);

    for i in 0..len {
        let (x, y) = points[i];
        let width = (thicknesses[i] as f64).max(PRESSURE_STROKE_MIN_WIDTH);
        samples.push(PressureStrokeSample {
            x: x as f64,
            y: y as f64,
            radius: width / 2.0,
        });
    }

    samples
}

/// Render a fast variable-thickness pressure stroke preview.
///
/// This intentionally trades final-stroke repair quality for low latency while
/// the stylus is still down. Committed pressure strokes use the mask renderer.
pub(crate) fn render_freehand_pressure_preview_borrowed(
    ctx: &cairo::Context,
    points: &[(i32, i32)],
    thicknesses: &[f32],
    color: Color,
) {
    let samples = pressure_stroke_preview_samples(points, thicknesses);
    if samples.is_empty() {
        return;
    }

    if color.a >= 1.0 {
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        fill_pressure_geometry(ctx, &samples);
        return;
    }

    let _ = ctx.save();
    ctx.push_group_with_content(cairo::Content::Alpha);
    ctx.set_operator(cairo::Operator::Over);
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    fill_pressure_geometry(ctx, &samples);

    if let Ok(mask) = ctx.pop_group() {
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        ctx.mask(&mask).ok();
    }
    let _ = ctx.restore();
}

/// Render a variable-thickness freehand stroke (pressure sensitive).
pub fn render_freehand_pressure_borrowed(
    ctx: &cairo::Context,
    points: &[(i32, i32)],
    thicknesses: &[f32],
    color: Color,
) {
    if points.is_empty() || thicknesses.is_empty() {
        return;
    }

    let samples = pressure_stroke_samples(points, thicknesses);
    if samples.is_empty() {
        return;
    }

    let _ = ctx.save();
    ctx.push_group_with_content(cairo::Content::Alpha);
    ctx.set_operator(cairo::Operator::Over);
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    fill_pressure_geometry(ctx, &samples);

    if let Ok(mask) = ctx.pop_group() {
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        ctx.mask(&mask).ok();
    }
    let _ = ctx.restore();
}

#[cfg(test)]
fn polygon_area(points: &[(f64, f64)]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }

    let mut area = 0.0;
    for i in 0..points.len() {
        let (x1, y1) = points[i];
        let (x2, y2) = points[(i + 1) % points.len()];
        area += x1 * y2 - x2 * y1;
    }
    area / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{Context, ImageSurface};

    fn surface_with_context(width: i32, height: i32) -> (ImageSurface, Context) {
        let surface = ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
        let ctx = Context::new(&surface).unwrap();
        (surface, ctx)
    }

    fn alpha_at(surface: &mut ImageSurface, x: i32, y: i32) -> u8 {
        let stride = surface.stride() as usize;
        let offset = y as usize * stride + x as usize * 4 + 3;
        surface.data().unwrap()[offset]
    }

    fn has_alpha(surface: &mut ImageSurface) -> bool {
        surface
            .data()
            .unwrap()
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0)
    }

    #[test]
    fn pressure_stroke_preview_renders_nonblank_output() {
        let points = [(20, 140), (100, 50), (220, 50), (320, 110)];
        let thicknesses = [32.0, 18.0, 5.0, 1.0];
        let (mut surface, ctx) = surface_with_context(360, 180);

        render_freehand_pressure_preview_borrowed(
            &ctx,
            &points,
            &thicknesses,
            Color {
                r: 1.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
        );
        drop(ctx);

        assert!(has_alpha(&mut surface));
    }

    #[test]
    fn pressure_stroke_preview_covers_backtracking_turnaround() {
        let points = [(40, 80), (160, 80), (100, 80)];
        let thicknesses = [30.0, 30.0, 30.0];
        let (mut surface, ctx) = surface_with_context(200, 140);

        render_freehand_pressure_preview_borrowed(
            &ctx,
            &points,
            &thicknesses,
            Color {
                r: 1.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
        );
        drop(ctx);

        assert!(
            alpha_at(&mut surface, 160, 80) > 160,
            "expected preview to cover backtracking turnaround"
        );
    }

    #[test]
    fn pressure_stroke_preview_translucent_overlap_applies_alpha_once() {
        let points = [(40, 80), (160, 80), (100, 80)];
        let thicknesses = [30.0, 30.0, 30.0];
        let expected_alpha = (0.35_f64 * 255.0).round() as i32;
        let (mut surface, ctx) = surface_with_context(200, 140);

        render_freehand_pressure_preview_borrowed(
            &ctx,
            &points,
            &thicknesses,
            Color {
                r: 1.0,
                g: 1.0,
                b: 0.0,
                a: 0.35,
            },
        );
        drop(ctx);

        let alpha = alpha_at(&mut surface, 100, 80) as i32;
        assert!(
            (expected_alpha - 8..=expected_alpha + 8).contains(&alpha),
            "expected preview overlap alpha near {expected_alpha}, got {alpha}"
        );
    }

    #[test]
    fn pressure_stroke_sparse_drop_geometry_is_valid_and_centerline_covered() {
        let points = [(20, 140), (100, 50), (220, 50), (320, 110)];
        let thicknesses = [32.0, 18.0, 5.0, 1.0];
        let segments = pressure_stroke_segments(&points, &thicknesses);
        assert!(segments.len() > points.len() - 1);
        assert!(segments.iter().all(PressureStrokeSegment::is_valid));

        let (mut surface, ctx) = surface_with_context(360, 180);
        render_freehand_pressure_borrowed(
            &ctx,
            &points,
            &thicknesses,
            Color {
                r: 1.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
        );
        drop(ctx);

        for window in points.windows(2) {
            let (x0, y0) = window[0];
            let (x1, y1) = window[1];
            for step in 1..20 {
                let t = step as f64 / 20.0;
                let x = (x0 as f64 + (x1 - x0) as f64 * t).round() as i32;
                let y = (y0 as f64 + (y1 - y0) as f64 * t).round() as i32;
                assert!(
                    alpha_at(&mut surface, x, y) > 160,
                    "expected pressure stroke to cover centerline at ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn pressure_stroke_translucent_overlap_applies_alpha_once() {
        let points = [(40, 80), (160, 80), (40, 80)];
        let thicknesses = [30.0, 30.0, 30.0];
        let expected_alpha = (0.35_f64 * 255.0).round() as i32;

        let (mut surface, ctx) = surface_with_context(200, 140);
        render_freehand_pressure_borrowed(
            &ctx,
            &points,
            &thicknesses,
            Color {
                r: 1.0,
                g: 1.0,
                b: 0.0,
                a: 0.35,
            },
        );
        drop(ctx);

        let alpha = alpha_at(&mut surface, 100, 80) as i32;
        assert!(
            (expected_alpha - 8..=expected_alpha + 8).contains(&alpha),
            "expected overlap alpha near {expected_alpha}, got {alpha}"
        );
    }
}
