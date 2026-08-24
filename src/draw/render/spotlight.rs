//! The spotlight compositing pass.
//!
//! Unlike every other shape, a spotlight does not paint itself — it darkens
//! everything *around* itself. So it cannot be a per-shape draw call; it is one
//! pass over the whole canvas.
//!
//! It runs *after* the committed shapes. Eraser strokes clear their path and
//! replay the original backdrop into it, so a dim layer painted before them
//! would be punched away and past erasures would show as bright trails. Running
//! last also means the committed canvas dims as a whole, while the live preview,
//! selection handles, click highlights, and UI drawn afterwards stay bright.

/// One elliptical opening left bright by the pass, in logical canvas coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpotlightRegion {
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
    pub magnification: f64,
}

/// How strongly the surrounding canvas is dimmed and how soft each edge is.
#[derive(Clone, Copy, Debug)]
pub struct SpotlightPass {
    /// Alpha of the dim layer outside every opening.
    pub dim_opacity: f64,
    /// Fraction of each radius spent fading out. 0.0 is a hard edge.
    pub feather: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotlightMagnifierSource {
    Complete,
    IncompleteTransparent,
}

impl SpotlightMagnifierSource {
    pub fn from_backdrop_presence(has_pixel_surface: bool, has_solid_color: bool) -> Self {
        if has_pixel_surface || has_solid_color {
            Self::Complete
        } else {
            Self::IncompleteTransparent
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotlightSnapshotStrategy {
    Regional,
    FullSurface,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpotlightMagnifierMetrics {
    pub regions: usize,
    pub copied_pixels: u64,
    pub strategy: SpotlightSnapshotStrategy,
    pub snapshot_time: std::time::Duration,
    pub paint_time: std::time::Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotlightMagnifierOutcome {
    NotNeeded,
    SourceUnavailable,
    Rendered(SpotlightMagnifierMetrics),
}

#[derive(Default)]
pub struct SpotlightMagnifierScratch {
    regional: Vec<cairo::ImageSurface>,
    full: Option<cairo::ImageSurface>,
}

#[derive(Clone, Copy, Debug)]
struct DeviceRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Clone)]
struct SpotlightSnapshot {
    surface: cairo::ImageSurface,
    origin_x: i32,
    origin_y: i32,
    region: SpotlightRegion,
}

/// Magnify the completed canvas under every loupe before the shared dim pass.
///
/// Every source snapshot is taken before any loupe is painted, so overlapping
/// regions cannot recursively sample one another. Small sets copy only their
/// clipped bounds; larger sets switch to one full-target copy, bounding retained
/// scratch storage to at most one target-sized ARGB image.
pub fn render_spotlight_magnification_pass(
    ctx: &cairo::Context,
    regions: &[SpotlightRegion],
    feather: f64,
    source: SpotlightMagnifierSource,
    target_size: (u32, u32),
    scratch: &mut SpotlightMagnifierScratch,
) -> Result<SpotlightMagnifierOutcome, cairo::Error> {
    let target = ctx.target();
    let image_dimensions = cairo::ImageSurface::try_from(target.clone())
        .ok()
        .map(|surface| (surface.width(), surface.height()));
    let target_width = image_dimensions
        .map(|dimensions| dimensions.0)
        .unwrap_or_else(|| i32::try_from(target_size.0).unwrap_or(i32::MAX));
    let target_height = image_dimensions
        .map(|dimensions| dimensions.1)
        .unwrap_or_else(|| i32::try_from(target_size.1).unwrap_or(i32::MAX));
    if target_width <= 0 || target_height <= 0 {
        return Ok(SpotlightMagnifierOutcome::NotNeeded);
    }

    let has_magnification = regions
        .iter()
        .any(|region| crate::draw::spotlight_magnification_is_active(region.magnification));
    if !has_magnification {
        return Ok(SpotlightMagnifierOutcome::NotNeeded);
    }
    if source == SpotlightMagnifierSource::IncompleteTransparent {
        return Ok(SpotlightMagnifierOutcome::SourceUnavailable);
    }

    let active: Vec<(SpotlightRegion, DeviceRect)> = regions
        .iter()
        .copied()
        .filter_map(|mut region| {
            region.magnification =
                crate::draw::normalize_spotlight_magnification(region.magnification);
            crate::draw::spotlight_magnification_is_active(region.magnification)
                .then(|| device_rect_for_region(ctx, region, target_width, target_height))
                .flatten()
                .map(|rect| (region, rect))
        })
        .collect();
    if active.is_empty() {
        return Ok(SpotlightMagnifierOutcome::NotNeeded);
    }

    let target_pixels = (target_width as u64).saturating_mul(target_height as u64);
    let regional_pixels = active.iter().fold(0u64, |total, (_, rect)| {
        total.saturating_add((rect.width as u64).saturating_mul(rect.height as u64))
    });
    let strategy = if regional_pixels <= target_pixels / 2 {
        SpotlightSnapshotStrategy::Regional
    } else {
        SpotlightSnapshotStrategy::FullSurface
    };

    let snapshot_start = std::time::Instant::now();
    target.flush();
    let snapshots = match strategy {
        SpotlightSnapshotStrategy::Regional => {
            scratch.full = None;
            scratch.regional.truncate(active.len());
            let mut snapshots = Vec::with_capacity(active.len());
            for (index, (region, rect)) in active.iter().copied().enumerate() {
                let surface = ensure_regional_surface(scratch, index, rect.width, rect.height)?;
                copy_target_rect(&target, &surface, rect)?;
                snapshots.push(SpotlightSnapshot {
                    surface,
                    origin_x: rect.x,
                    origin_y: rect.y,
                    region,
                });
            }
            snapshots
        }
        SpotlightSnapshotStrategy::FullSurface => {
            scratch.regional.clear();
            let surface = ensure_full_surface(scratch, target_width, target_height)?;
            copy_target_rect(
                &target,
                &surface,
                DeviceRect {
                    x: 0,
                    y: 0,
                    width: target_width,
                    height: target_height,
                },
            )?;
            active
                .iter()
                .map(|(region, _)| SpotlightSnapshot {
                    surface: surface.clone(),
                    origin_x: 0,
                    origin_y: 0,
                    region: *region,
                })
                .collect()
        }
    };

    let snapshot_time = snapshot_start.elapsed();
    let paint_start = std::time::Instant::now();
    for snapshot in &snapshots {
        paint_snapshot(ctx, snapshot, feather)?;
    }
    let paint_time = paint_start.elapsed();

    Ok(SpotlightMagnifierOutcome::Rendered(
        SpotlightMagnifierMetrics {
            regions: snapshots.len(),
            copied_pixels: match strategy {
                SpotlightSnapshotStrategy::Regional => regional_pixels,
                SpotlightSnapshotStrategy::FullSurface => target_pixels,
            },
            strategy,
            snapshot_time,
            paint_time,
        },
    ))
}

fn device_rect_for_region(
    ctx: &cairo::Context,
    region: SpotlightRegion,
    target_width: i32,
    target_height: i32,
) -> Option<DeviceRect> {
    let rx = region.rx.abs().max(1.0);
    let ry = region.ry.abs().max(1.0);
    let corners = [
        ctx.user_to_device(region.cx - rx, region.cy - ry),
        ctx.user_to_device(region.cx + rx, region.cy - ry),
        ctx.user_to_device(region.cx - rx, region.cy + ry),
        ctx.user_to_device(region.cx + rx, region.cy + ry),
    ];
    if corners
        .iter()
        .any(|(x, y)| !x.is_finite() || !y.is_finite())
    {
        return None;
    }
    let min_x = corners
        .iter()
        .map(|point| point.0)
        .fold(f64::INFINITY, f64::min)
        .floor() as i64
        - 1;
    let min_y = corners
        .iter()
        .map(|point| point.1)
        .fold(f64::INFINITY, f64::min)
        .floor() as i64
        - 1;
    let max_x = corners
        .iter()
        .map(|point| point.0)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil() as i64
        + 1;
    let max_y = corners
        .iter()
        .map(|point| point.1)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil() as i64
        + 1;
    let x0 = min_x.clamp(0, i64::from(target_width));
    let y0 = min_y.clamp(0, i64::from(target_height));
    let x1 = max_x.clamp(0, i64::from(target_width));
    let y1 = max_y.clamp(0, i64::from(target_height));
    let width = i32::try_from(x1 - x0).ok()?;
    let height = i32::try_from(y1 - y0).ok()?;
    (width > 0 && height > 0).then_some(DeviceRect {
        x: i32::try_from(x0).ok()?,
        y: i32::try_from(y0).ok()?,
        width,
        height,
    })
}

fn ensure_regional_surface(
    scratch: &mut SpotlightMagnifierScratch,
    index: usize,
    width: i32,
    height: i32,
) -> Result<cairo::ImageSurface, cairo::Error> {
    let reusable = scratch
        .regional
        .get(index)
        .filter(|surface| surface.width() == width && surface.height() == height)
        .cloned();
    let surface = match reusable {
        Some(surface) => surface,
        None => cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?,
    };
    if index < scratch.regional.len() {
        scratch.regional[index] = surface.clone();
    } else {
        scratch.regional.push(surface.clone());
    }
    Ok(surface)
}

fn ensure_full_surface(
    scratch: &mut SpotlightMagnifierScratch,
    width: i32,
    height: i32,
) -> Result<cairo::ImageSurface, cairo::Error> {
    if let Some(surface) = scratch
        .full
        .as_ref()
        .filter(|surface| surface.width() == width && surface.height() == height)
    {
        return Ok(surface.clone());
    }
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;
    scratch.full = Some(surface.clone());
    Ok(surface)
}

fn copy_target_rect(
    target: &cairo::Surface,
    destination: &cairo::ImageSurface,
    rect: DeviceRect,
) -> Result<(), cairo::Error> {
    let copy = cairo::Context::new(destination)?;
    copy.set_operator(cairo::Operator::Source);
    copy.set_source_surface(target, -f64::from(rect.x), -f64::from(rect.y))?;
    copy.paint()?;
    destination.flush();
    Ok(())
}

fn paint_snapshot(
    ctx: &cairo::Context,
    snapshot: &SpotlightSnapshot,
    feather: f64,
) -> Result<(), cairo::Error> {
    let region = snapshot.region;
    let rx = region.rx.abs().max(1.0);
    let ry = region.ry.abs().max(1.0);
    let center = ctx.user_to_device(region.cx, region.cy);
    let axis_x = ctx.user_to_device(region.cx + rx, region.cy);
    let axis_y = ctx.user_to_device(region.cx, region.cy + ry);
    let x_axis = (axis_x.0 - center.0, axis_x.1 - center.1);
    let y_axis = (axis_y.0 - center.0, axis_y.1 - center.1);
    let determinant = x_axis.0 * y_axis.1 - y_axis.0 * x_axis.1;
    if !determinant.is_finite() || determinant.abs() <= f64::EPSILON {
        return Ok(());
    }

    ctx.save()?;
    let painted = (|| {
        // Establish the ellipse under the caller's transform. Cairo stores the
        // resulting clip in device space, so sampling can then use an identity CTM.
        ctx.new_path();
        ctx.save()?;
        ctx.translate(region.cx, region.cy);
        ctx.scale(rx, ry);
        ctx.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
        ctx.restore()?;
        ctx.clip();
        ctx.identity_matrix();

        let magnification = crate::draw::normalize_spotlight_magnification(region.magnification);
        let pattern = cairo::SurfacePattern::create(&snapshot.surface);
        pattern.set_extend(cairo::Extend::Pad);
        pattern.set_filter(cairo::Filter::Bilinear);
        pattern.set_matrix(cairo::Matrix::new(
            1.0 / magnification,
            0.0,
            0.0,
            1.0 / magnification,
            center.0 * (1.0 - 1.0 / magnification) - f64::from(snapshot.origin_x),
            center.1 * (1.0 - 1.0 / magnification) - f64::from(snapshot.origin_y),
        ));
        ctx.set_source(&pattern)?;

        let feather = feather.clamp(0.0, 0.9);
        if feather <= f64::EPSILON {
            return ctx.paint();
        }

        let inv_xx = y_axis.1 / determinant;
        let inv_xy = -y_axis.0 / determinant;
        let inv_yx = -x_axis.1 / determinant;
        let inv_yy = x_axis.0 / determinant;
        let mask = cairo::RadialGradient::new(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
        let solid_until = (1.0 - feather).clamp(0.0, 1.0);
        mask.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 1.0);
        mask.add_color_stop_rgba(solid_until, 1.0, 1.0, 1.0, 1.0);
        mask.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0);
        mask.set_extend(cairo::Extend::Pad);
        mask.set_matrix(cairo::Matrix::new(
            inv_xx,
            inv_yx,
            inv_xy,
            inv_yy,
            -(inv_xx * center.0 + inv_xy * center.1),
            -(inv_yx * center.0 + inv_yy * center.1),
        ));
        ctx.mask(&mask)
    })();
    let restored = ctx.restore();
    painted.and(restored)
}

/// Every spotlight opening on a frame, in the order the shapes were added.
///
/// The pass needs all regions at once, so each surface that renders a frame —
/// the live canvas, exports, and board thumbnails — collects them the same way.
pub fn spotlight_regions_for_frame(frame: &crate::draw::Frame) -> Vec<SpotlightRegion> {
    frame
        .shapes
        .iter()
        .filter_map(|drawn| match &drawn.shape {
            crate::draw::Shape::Spotlight {
                cx,
                cy,
                rx,
                ry,
                magnification,
            } => Some(SpotlightRegion {
                cx: f64::from(*cx),
                cy: f64::from(*cy),
                rx: f64::from(*rx),
                ry: f64::from(*ry),
                magnification: crate::draw::normalize_spotlight_magnification(*magnification),
            }),
            _ => None,
        })
        .collect()
}

/// Dims the current clip except inside `regions`.
///
/// The dim layer is built in its own group before the openings are punched out
/// of it. That isolation matters: punching directly onto the canvas would clear
/// the board background or captured backdrop underneath, leaving a transparent
/// hole instead of revealing what the spotlight is supposed to reveal.
pub fn render_spotlight_pass(
    ctx: &cairo::Context,
    regions: &[SpotlightRegion],
    pass: SpotlightPass,
) {
    if regions.is_empty() {
        return;
    }
    let dim = pass.dim_opacity.clamp(0.0, 1.0);
    if dim <= f64::EPSILON {
        return;
    }
    let feather = pass.feather.clamp(0.0, 0.9);

    let _ = ctx.save();
    ctx.push_group();

    ctx.set_source_rgba(0.0, 0.0, 0.0, dim);
    let _ = ctx.paint();

    ctx.set_operator(cairo::Operator::DestOut);
    for region in regions {
        punch_opening(ctx, *region, feather);
    }

    let _ = ctx.pop_group_to_source();
    ctx.set_operator(cairo::Operator::Over);
    let _ = ctx.paint();
    let _ = ctx.restore();
}

/// Erases one feathered ellipse from the dim layer currently being built.
fn punch_opening(ctx: &cairo::Context, region: SpotlightRegion, feather: f64) {
    let rx = region.rx.max(1.0);
    let ry = region.ry.max(1.0);

    let _ = ctx.save();
    ctx.translate(region.cx, region.cy);
    ctx.scale(rx, ry);

    // Radius 1.0 in the scaled space, so one gradient serves any aspect ratio.
    let gradient = cairo::RadialGradient::new(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    let solid_until = (1.0 - feather).clamp(0.0, 1.0);
    gradient.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 1.0);
    gradient.add_color_stop_rgba(solid_until, 0.0, 0.0, 0.0, 1.0);
    gradient.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    let _ = ctx.set_source(&gradient);

    ctx.new_sub_path();
    ctx.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
    let _ = ctx.fill();
    let _ = ctx.restore();
}

/// Outline drawn for a selected spotlight, since the shape itself is invisible.
pub fn render_spotlight_outline(
    ctx: &cairo::Context,
    region: SpotlightRegion,
    color: crate::draw::Color,
    thick: f64,
) {
    let rx = region.rx.max(1.0);
    let ry = region.ry.max(1.0);

    // Build the ellipse under a scaled transform, then drop the transform before
    // stroking so the line width is not scaled with it. Cairo keeps the path
    // across restore, but not the source or line width — so both are set after
    // the restore, or the stroke would inherit whatever the caller left behind.
    ctx.new_path();
    let _ = ctx.save();
    ctx.translate(region.cx, region.cy);
    ctx.scale(rx, ry);
    ctx.new_sub_path();
    ctx.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
    let _ = ctx.restore();

    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    let _ = ctx.stroke();
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{Context, ImageSurface};

    fn surface_with_context(width: i32, height: i32) -> (ImageSurface, Context) {
        let surface = ImageSurface::create(cairo::Format::ARgb32, width, height)
            .expect("test surface allocation");
        let ctx = Context::new(&surface).expect("test cairo context");
        (surface, ctx)
    }

    fn alpha_at(surface: &mut ImageSurface, x: i32, y: i32) -> u8 {
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().expect("surface data");
        data[y as usize * stride + x as usize * 4 + 3]
    }

    fn rgb_at(surface: &mut ImageSurface, x: i32, y: i32) -> (u8, u8, u8) {
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().expect("surface data");
        let idx = y as usize * stride + x as usize * 4;
        (data[idx + 2], data[idx + 1], data[idx])
    }

    const CENTERED: SpotlightRegion = SpotlightRegion {
        cx: 100.0,
        cy: 100.0,
        rx: 40.0,
        ry: 30.0,
        magnification: 1.0,
    };

    #[test]
    fn magnifier_source_requires_pixels_or_a_solid_color() {
        assert_eq!(
            SpotlightMagnifierSource::from_backdrop_presence(false, false),
            SpotlightMagnifierSource::IncompleteTransparent
        );
        assert_eq!(
            SpotlightMagnifierSource::from_backdrop_presence(true, false),
            SpotlightMagnifierSource::Complete
        );
        assert_eq!(
            SpotlightMagnifierSource::from_backdrop_presence(false, true),
            SpotlightMagnifierSource::Complete
        );
    }

    #[test]
    fn no_regions_leaves_the_canvas_untouched() {
        let (mut surface, ctx) = surface_with_context(60, 60);
        render_spotlight_pass(
            &ctx,
            &[],
            SpotlightPass {
                dim_opacity: 0.6,
                feather: 0.3,
            },
        );
        drop(ctx);
        assert_eq!(
            alpha_at(&mut surface, 30, 30),
            0,
            "a frame with no spotlights must not dim anything"
        );
    }

    #[test]
    fn zero_dim_opacity_is_a_no_op() {
        let (mut surface, ctx) = surface_with_context(60, 60);
        render_spotlight_pass(
            &ctx,
            &[SpotlightRegion {
                cx: 30.0,
                cy: 30.0,
                rx: 10.0,
                ry: 10.0,
                magnification: 1.0,
            }],
            SpotlightPass {
                dim_opacity: 0.0,
                feather: 0.3,
            },
        );
        drop(ctx);
        assert_eq!(alpha_at(&mut surface, 5, 5), 0);
    }

    #[test]
    fn opening_is_transparent_while_the_surround_is_dimmed() {
        let (mut surface, ctx) = surface_with_context(200, 200);
        render_spotlight_pass(
            &ctx,
            &[CENTERED],
            SpotlightPass {
                dim_opacity: 0.6,
                feather: 0.3,
            },
        );
        drop(ctx);

        assert_eq!(
            alpha_at(&mut surface, 100, 100),
            0,
            "the opening must be fully transparent so live content shows through"
        );
        let surround = alpha_at(&mut surface, 5, 5);
        assert!(
            surround > 140,
            "surround should carry the dim layer, got alpha {surround}"
        );
    }

    #[test]
    fn feathering_produces_a_gradual_edge() {
        let (mut surface, ctx) = surface_with_context(200, 200);
        render_spotlight_pass(
            &ctx,
            &[CENTERED],
            SpotlightPass {
                dim_opacity: 0.8,
                feather: 0.5,
            },
        );
        drop(ctx);

        // Walking out along +x from the centre, alpha must rise monotonically.
        let samples: Vec<u8> = (0..=40)
            .step_by(8)
            .map(|dx| alpha_at(&mut surface, 100 + dx, 100))
            .collect();
        for pair in samples.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "feathered edge should not brighten outward: {samples:?}"
            );
        }
        assert!(
            samples[0] < samples[samples.len() - 1],
            "edge should actually fade: {samples:?}"
        );
    }

    #[test]
    fn hard_edge_when_feather_is_zero() {
        let (mut surface, ctx) = surface_with_context(200, 200);
        render_spotlight_pass(
            &ctx,
            &[CENTERED],
            SpotlightPass {
                dim_opacity: 0.8,
                feather: 0.0,
            },
        );
        drop(ctx);

        assert_eq!(alpha_at(&mut surface, 100, 100), 0);
        // Just inside the 40px x-radius stays clear; just outside is fully dim.
        assert!(alpha_at(&mut surface, 135, 100) < 40);
        assert!(alpha_at(&mut surface, 145, 100) > 180);
    }

    #[test]
    fn background_beneath_the_opening_survives_the_pass() {
        // The board-mode and export case: the pass must not clear what is under it.
        let (mut surface, ctx) = surface_with_context(200, 200);
        ctx.set_source_rgb(0.0, 1.0, 0.0);
        let _ = ctx.paint();

        render_spotlight_pass(
            &ctx,
            &[CENTERED],
            SpotlightPass {
                dim_opacity: 0.6,
                feather: 0.0,
            },
        );
        drop(ctx);

        let inside = rgb_at(&mut surface, 100, 100);
        assert_eq!(
            inside,
            (0, 255, 0),
            "background under the opening must be untouched, got {inside:?}"
        );
        assert_eq!(
            alpha_at(&mut surface, 100, 100),
            255,
            "the opening must stay opaque where a background exists"
        );

        let outside = rgb_at(&mut surface, 5, 5);
        assert!(
            outside.1 < 130,
            "surround should be darkened, got {outside:?}"
        );
    }

    #[test]
    fn multiple_openings_are_all_punched() {
        let (mut surface, ctx) = surface_with_context(300, 120);
        render_spotlight_pass(
            &ctx,
            &[
                SpotlightRegion {
                    cx: 70.0,
                    cy: 60.0,
                    rx: 30.0,
                    ry: 30.0,
                    magnification: 1.0,
                },
                SpotlightRegion {
                    cx: 220.0,
                    cy: 60.0,
                    rx: 30.0,
                    ry: 30.0,
                    magnification: 1.0,
                },
            ],
            SpotlightPass {
                dim_opacity: 0.7,
                feather: 0.0,
            },
        );
        drop(ctx);

        assert_eq!(alpha_at(&mut surface, 70, 60), 0);
        assert_eq!(alpha_at(&mut surface, 220, 60), 0);
        assert!(
            alpha_at(&mut surface, 145, 60) > 150,
            "the gap between two spotlights stays dimmed"
        );
    }

    #[test]
    fn selection_outline_ignores_whatever_styling_the_caller_left_behind() {
        let (mut surface, ctx) = surface_with_context(200, 200);
        // A caller mid-render leaves a source and width set; the outline must
        // use its own, not inherit these.
        ctx.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        ctx.set_line_width(1.0);

        render_spotlight_outline(
            &ctx,
            CENTERED,
            crate::draw::Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            6.0,
        );
        drop(ctx);

        // On the ellipse's right edge (cx + rx = 140, cy = 100).
        assert!(
            alpha_at(&mut surface, 140, 100) > 0,
            "the outline must actually stroke"
        );
        let (r, g, _b) = rgb_at(&mut surface, 140, 100);
        assert!(
            r > 200 && g < 60,
            "outline should use its own red, not the caller's green: rgb ({r}, {g}, _)"
        );
    }

    #[test]
    fn selection_outline_leaves_no_dimming_behind() {
        let (mut surface, ctx) = surface_with_context(200, 200);
        render_spotlight_outline(
            &ctx,
            CENTERED,
            crate::draw::Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            4.0,
        );
        drop(ctx);
        assert_eq!(
            alpha_at(&mut surface, 100, 100),
            0,
            "the outline is a stroke only; it must not fill or dim"
        );
    }

    #[test]
    fn degenerate_radii_do_not_panic_or_dim_everything() {
        let (mut surface, ctx) = surface_with_context(80, 80);
        render_spotlight_pass(
            &ctx,
            &[SpotlightRegion {
                cx: 40.0,
                cy: 40.0,
                rx: 0.0,
                ry: 0.0,
                magnification: 1.0,
            }],
            SpotlightPass {
                dim_opacity: 0.6,
                feather: 0.3,
            },
        );
        drop(ctx);
        assert!(alpha_at(&mut surface, 5, 5) > 140);
    }

    #[test]
    fn two_x_loupe_samples_half_the_distance_from_its_center() {
        let (mut surface, ctx) = surface_with_context(100, 100);
        ctx.set_source_rgb(1.0, 0.0, 0.0);
        ctx.rectangle(0.0, 0.0, 70.0, 100.0);
        ctx.fill().unwrap();
        ctx.set_source_rgb(0.0, 0.0, 1.0);
        ctx.rectangle(70.0, 0.0, 30.0, 100.0);
        ctx.fill().unwrap();

        let outcome = render_spotlight_magnification_pass(
            &ctx,
            &[SpotlightRegion {
                cx: 50.0,
                cy: 50.0,
                rx: 30.0,
                ry: 30.0,
                magnification: 2.0,
            }],
            0.0,
            SpotlightMagnifierSource::Complete,
            (100, 100),
            &mut SpotlightMagnifierScratch::default(),
        )
        .expect("loupe should render");
        drop(ctx);

        assert!(matches!(outcome, SpotlightMagnifierOutcome::Rendered(_)));
        assert_eq!(rgb_at(&mut surface, 75, 50), (255, 0, 0));
        assert_eq!(rgb_at(&mut surface, 85, 50), (0, 0, 255));
    }

    #[test]
    fn one_x_loupe_skips_all_snapshot_work() {
        let (mut surface, ctx) = surface_with_context(32, 32);
        ctx.set_source_rgb(0.0, 1.0, 0.0);
        ctx.paint().unwrap();

        let outcome = render_spotlight_magnification_pass(
            &ctx,
            &[SpotlightRegion {
                cx: 16.0,
                cy: 16.0,
                rx: 12.0,
                ry: 12.0,
                magnification: 1.0,
            }],
            0.35,
            SpotlightMagnifierSource::IncompleteTransparent,
            (32, 32),
            &mut SpotlightMagnifierScratch::default(),
        )
        .expect("1x pass");
        drop(ctx);

        assert_eq!(outcome, SpotlightMagnifierOutcome::NotNeeded);
        assert_eq!(rgb_at(&mut surface, 16, 16), (0, 255, 0));
    }

    #[test]
    fn incomplete_source_reports_unavailable_without_changing_pixels() {
        let (mut surface, ctx) = surface_with_context(32, 32);
        ctx.set_source_rgb(1.0, 0.0, 0.0);
        ctx.paint().unwrap();

        let outcome = render_spotlight_magnification_pass(
            &ctx,
            &[SpotlightRegion {
                cx: 16.0,
                cy: 16.0,
                rx: 12.0,
                ry: 12.0,
                magnification: 3.0,
            }],
            0.35,
            SpotlightMagnifierSource::IncompleteTransparent,
            (32, 32),
            &mut SpotlightMagnifierScratch::default(),
        )
        .expect("unavailable pass");
        drop(ctx);

        assert_eq!(outcome, SpotlightMagnifierOutcome::SourceUnavailable);
        assert_eq!(rgb_at(&mut surface, 16, 16), (255, 0, 0));
    }

    #[test]
    fn overlapping_loupes_sample_the_same_pre_loupe_canvas() {
        fn gradient_surface() -> (ImageSurface, cairo::Context) {
            let (surface, ctx) = surface_with_context(100, 100);
            for x in 0..100 {
                let channel = f64::from(x) / 99.0;
                ctx.set_source_rgb(channel, channel, channel);
                ctx.rectangle(f64::from(x), 0.0, 1.0, 100.0);
                ctx.fill().unwrap();
            }
            (surface, ctx)
        }

        let second = SpotlightRegion {
            cx: 60.0,
            cy: 50.0,
            rx: 30.0,
            ry: 30.0,
            magnification: 2.0,
        };
        let (mut expected_surface, expected_ctx) = gradient_surface();
        render_spotlight_magnification_pass(
            &expected_ctx,
            &[second],
            0.0,
            SpotlightMagnifierSource::Complete,
            (100, 100),
            &mut SpotlightMagnifierScratch::default(),
        )
        .unwrap();
        drop(expected_ctx);
        let expected = rgb_at(&mut expected_surface, 50, 50);

        let (mut actual_surface, actual_ctx) = gradient_surface();
        render_spotlight_magnification_pass(
            &actual_ctx,
            &[
                SpotlightRegion {
                    cx: 40.0,
                    cy: 50.0,
                    rx: 30.0,
                    ry: 30.0,
                    magnification: 2.0,
                },
                second,
            ],
            0.0,
            SpotlightMagnifierSource::Complete,
            (100, 100),
            &mut SpotlightMagnifierScratch::default(),
        )
        .unwrap();
        drop(actual_ctx);

        assert_eq!(rgb_at(&mut actual_surface, 50, 50), expected);
    }

    #[test]
    fn magnification_respects_the_callers_scaled_canvas_transform() {
        let (mut surface, ctx) = surface_with_context(200, 200);
        ctx.set_source_rgb(1.0, 0.0, 0.0);
        ctx.rectangle(0.0, 0.0, 140.0, 200.0);
        ctx.fill().unwrap();
        ctx.set_source_rgb(0.0, 0.0, 1.0);
        ctx.rectangle(140.0, 0.0, 60.0, 200.0);
        ctx.fill().unwrap();
        ctx.scale(2.0, 2.0);

        render_spotlight_magnification_pass(
            &ctx,
            &[SpotlightRegion {
                cx: 50.0,
                cy: 50.0,
                rx: 30.0,
                ry: 30.0,
                magnification: 2.0,
            }],
            0.0,
            SpotlightMagnifierSource::Complete,
            (200, 200),
            &mut SpotlightMagnifierScratch::default(),
        )
        .unwrap();
        drop(ctx);

        assert_eq!(rgb_at(&mut surface, 150, 100), (255, 0, 0));
        assert_eq!(rgb_at(&mut surface, 175, 100), (0, 0, 255));
    }
}
