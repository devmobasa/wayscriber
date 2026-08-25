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

/// Token for a raster backdrop that cannot go stale: an export snapshot, a
/// region capture, or a persisted session image. Those pixels are immutable
/// for the lifetime of the render, so they need no generation of their own.
pub const IMMUTABLE_RASTER_SOURCE_TOKEN: u64 = 0;

/// Whether the canvas under a loupe is a complete set of pixels, and where
/// those pixels came from.
///
/// A raster source carries the provenance identity of the capture the backend
/// validated, so a stale Frozen/Zoom generation is a *different* value rather
/// than one silently reused. That identity is what lets the backend decide in
/// one place what may be painted and what the loupe may sample, and lets the
/// toolbar report a reason drawn from the same answer.
///
/// It has no role in scratch storage: retained snapshot surfaces are rewritten
/// in full before every use, so they cannot carry a previous capture's pixels
/// (see [`SpotlightMagnifierScratch`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotlightMagnifierSource {
    /// An opaque board fills every pixel itself; nothing else is needed.
    CompleteSolid,
    /// Captured desktop pixels with a current, provenance-valid token.
    CompleteRaster { source_token: u64 },
    /// A transparent board with no usable captured pixels underneath.
    IncompleteTransparent,
}

impl SpotlightMagnifierSource {
    /// Resolves availability from the two facts every surface can answer: the
    /// provenance token of its raster backdrop *if that backdrop is currently
    /// valid*, and whether an opaque board colour fills the rest.
    ///
    /// A raster backdrop whose provenance has gone stale passes `None` here and
    /// degrades to the solid/transparent answer, which is what keeps a stale
    /// capture from being magnified as though it were live.
    pub const fn from_backdrop(raster_token: Option<u64>, has_solid_color: bool) -> Self {
        match raster_token {
            Some(source_token) => Self::CompleteRaster { source_token },
            None if has_solid_color => Self::CompleteSolid,
            None => Self::IncompleteTransparent,
        }
    }

    /// Availability for a backdrop whose pixels cannot change under it.
    pub const fn immutable_raster() -> Self {
        Self::CompleteRaster {
            source_token: IMMUTABLE_RASTER_SOURCE_TOKEN,
        }
    }

    /// Whether the loupe has every pixel it needs to magnify faithfully.
    pub const fn is_complete(self) -> bool {
        !matches!(self, Self::IncompleteTransparent)
    }

    /// Provenance identity of the raster backdrop, when there is one.
    pub const fn raster_token(self) -> Option<u64> {
        match self {
            Self::CompleteRaster { source_token } => Some(source_token),
            Self::CompleteSolid | Self::IncompleteTransparent => None,
        }
    }

    /// Stable, user-facing reason a loupe cannot preview right now.
    pub const fn unavailable_reason(self) -> Option<&'static str> {
        match self {
            Self::IncompleteTransparent => Some("Freeze screen to preview"),
            Self::CompleteSolid | Self::CompleteRaster { .. } => None,
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
    /// No region asked for magnification, so no snapshot was taken.
    NotNeeded,
    /// The surface underneath has no complete pixel source to sample.
    SourceUnavailable,
    /// Scratch storage for the snapshot could not be allocated.
    AllocationFailed,
    Rendered(SpotlightMagnifierMetrics),
}

/// Retained snapshot surfaces, reused across frames whenever their size still
/// fits.
///
/// Deliberately not keyed on which capture the pixels came from: every
/// retained surface is rewritten in full by [`copy_target_rect`], which paints
/// the whole destination with [`cairo::Operator::Source`], so a surface can
/// never serve pixels from the capture that filled it last. Size checks are
/// the only reuse condition that has to hold, and invalidating on capture
/// identity would only force reallocation on every Freeze, Zoom, or recapture.
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
///
/// `fallback_target_size` is the pixel size to assume when the Cairo target is
/// not an image surface — a vector PDF page, say. `None` when the caller has no
/// raster size to offer, which skips the pass rather than guessing one.
pub fn render_spotlight_magnification_pass(
    ctx: &cairo::Context,
    regions: &[SpotlightRegion],
    feather: f64,
    source: SpotlightMagnifierSource,
    fallback_target_size: Option<(u32, u32)>,
    scratch: &mut SpotlightMagnifierScratch,
) -> Result<SpotlightMagnifierOutcome, cairo::Error> {
    let target = ctx.target();
    let image_dimensions = cairo::ImageSurface::try_from(target.clone())
        .ok()
        .map(|surface| (surface.width(), surface.height()))
        .or_else(|| {
            let (width, height) = fallback_target_size?;
            Some((i32::try_from(width).ok()?, i32::try_from(height).ok()?))
        });
    let Some((target_width, target_height)) = image_dimensions else {
        return Ok(SpotlightMagnifierOutcome::NotNeeded);
    };
    if target_width <= 0 || target_height <= 0 {
        return Ok(SpotlightMagnifierOutcome::NotNeeded);
    }

    let has_magnification = regions
        .iter()
        .any(|region| crate::draw::spotlight_magnification_is_active(region.magnification));
    if !has_magnification {
        return Ok(SpotlightMagnifierOutcome::NotNeeded);
    }
    if !source.is_complete() {
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
    // Regional copies win while their combined area is at most half of the
    // target. Above that point one full copy bounds allocation count and copy
    // bookkeeping without retaining both strategies at once. The crossover is
    // pinned by `the_snapshot_strategy_crosses_over_at_half_the_target_area`;
    // it is a bookkeeping bound, not a measured one — see the perf note in
    // `docs/temp/spotlight-magnifier.md`.
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
                let Some(surface) =
                    ensure_regional_surface(scratch, index, rect.width, rect.height)
                else {
                    return Ok(SpotlightMagnifierOutcome::AllocationFailed);
                };
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
            let Some(surface) = ensure_full_surface(scratch, target_width, target_height) else {
                return Ok(SpotlightMagnifierOutcome::AllocationFailed);
            };
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

/// `None` means the allocation was refused, which the pass reports as
/// [`SpotlightMagnifierOutcome::AllocationFailed`] rather than an error: a
/// loupe that cannot allocate degrades to the ordinary bright opening.
fn ensure_regional_surface(
    scratch: &mut SpotlightMagnifierScratch,
    index: usize,
    width: i32,
    height: i32,
) -> Option<cairo::ImageSurface> {
    let reusable = scratch
        .regional
        .get(index)
        .filter(|surface| surface.width() == width && surface.height() == height)
        .cloned();
    let surface = match reusable {
        Some(surface) => surface,
        None => cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).ok()?,
    };
    if index < scratch.regional.len() {
        scratch.regional[index] = surface.clone();
    } else {
        scratch.regional.push(surface.clone());
    }
    Some(surface)
}

fn ensure_full_surface(
    scratch: &mut SpotlightMagnifierScratch,
    width: i32,
    height: i32,
) -> Option<cairo::ImageSurface> {
    if let Some(surface) = scratch
        .full
        .as_ref()
        .filter(|surface| surface.width() == width && surface.height() == height)
    {
        return Some(surface.clone());
    }
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).ok()?;
    scratch.full = Some(surface.clone());
    Some(surface)
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

    crate::draw::with_saved_state(ctx, || {
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
    })
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
    fn magnifier_source_requires_valid_pixels_or_a_solid_color() {
        assert_eq!(
            SpotlightMagnifierSource::from_backdrop(None, false),
            SpotlightMagnifierSource::IncompleteTransparent
        );
        assert_eq!(
            SpotlightMagnifierSource::from_backdrop(Some(9), false),
            SpotlightMagnifierSource::CompleteRaster { source_token: 9 }
        );
        assert_eq!(
            SpotlightMagnifierSource::from_backdrop(None, true),
            SpotlightMagnifierSource::CompleteSolid
        );
        // A raster backdrop wins over the board colour: it is the layer the
        // loupe actually samples.
        assert_eq!(
            SpotlightMagnifierSource::from_backdrop(Some(3), true),
            SpotlightMagnifierSource::CompleteRaster { source_token: 3 }
        );
    }

    #[test]
    fn stale_raster_provenance_degrades_instead_of_magnifying_old_pixels() {
        // A transparent board whose capture went stale passes `None` here.
        // Reporting it complete would magnify pixels that no longer match the
        // desktop underneath.
        let stale_transparent = SpotlightMagnifierSource::from_backdrop(None, false);
        assert!(!stale_transparent.is_complete());
        assert_eq!(
            stale_transparent.unavailable_reason(),
            Some("Freeze screen to preview")
        );

        // The same staleness on a solid board still has every pixel it needs.
        let stale_solid = SpotlightMagnifierSource::from_backdrop(None, true);
        assert!(stale_solid.is_complete());
        assert_eq!(stale_solid.unavailable_reason(), None);
        assert_eq!(stale_solid.raster_token(), None);
    }

    #[test]
    fn a_recapture_is_a_different_source_than_the_capture_it_replaced() {
        let before = SpotlightMagnifierSource::from_backdrop(Some(1), false);
        let after = SpotlightMagnifierSource::from_backdrop(Some(2), false);
        assert_ne!(before, after, "a new capture must not compare equal");
        assert_eq!(before.raster_token(), Some(1));
        assert!(SpotlightMagnifierSource::immutable_raster().is_complete());
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
            SpotlightMagnifierSource::CompleteSolid,
            Some((100, 100)),
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
            Some((32, 32)),
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
            Some((32, 32)),
            &mut SpotlightMagnifierScratch::default(),
        )
        .expect("unavailable pass");
        drop(ctx);

        assert_eq!(outcome, SpotlightMagnifierOutcome::SourceUnavailable);
        assert_eq!(rgb_at(&mut surface, 16, 16), (255, 0, 0));
    }

    /// Red field with a 20x20 yellow square centred at (50, 50).
    ///
    /// At 4x that square covers 80x80, which is wider than the 60px loupe — so
    /// every pixel inside the opening samples yellow while every pixel outside
    /// it must stay red. That separates "sampled at 4x" from "clipped to the
    /// ellipse" in one image.
    fn marked_surface() -> (ImageSurface, Context) {
        let (surface, ctx) = surface_with_context(100, 100);
        ctx.set_source_rgb(1.0, 0.0, 0.0);
        ctx.paint().unwrap();
        ctx.set_source_rgb(1.0, 1.0, 0.0);
        ctx.rectangle(40.0, 40.0, 20.0, 20.0);
        ctx.fill().unwrap();
        (surface, ctx)
    }

    const FOUR_X: SpotlightRegion = SpotlightRegion {
        cx: 50.0,
        cy: 50.0,
        rx: 30.0,
        ry: 30.0,
        magnification: 4.0,
    };

    #[test]
    fn four_x_samples_a_quarter_of_the_distance_from_its_center() {
        let (mut surface, ctx) = marked_surface();
        let outcome = render_spotlight_magnification_pass(
            &ctx,
            &[FOUR_X],
            0.0,
            SpotlightMagnifierSource::CompleteSolid,
            Some((100, 100)),
            &mut SpotlightMagnifierScratch::default(),
        )
        .expect("4x loupe renders");
        drop(ctx);

        assert!(matches!(outcome, SpotlightMagnifierOutcome::Rendered(_)));
        assert_eq!(rgb_at(&mut surface, 50, 50), (255, 255, 0), "center");
        // source = 50 + 25/4 = 56.25, inside the square. At 1x or 2x this
        // pixel would still be red, so the assertion is specific to 4x.
        assert_eq!(rgb_at(&mut surface, 75, 50), (255, 255, 0), "near the rim");
    }

    #[test]
    fn magnified_pixels_stay_inside_the_elliptical_opening() {
        let (mut surface, ctx) = marked_surface();
        render_spotlight_magnification_pass(
            &ctx,
            &[FOUR_X],
            0.0,
            SpotlightMagnifierSource::CompleteSolid,
            Some((100, 100)),
            &mut SpotlightMagnifierScratch::default(),
        )
        .expect("4x loupe renders");
        drop(ctx);

        // The magnified square would reach x = 90 unclipped; the opening ends
        // at x = 80, so everything past it must still be the original red.
        assert_eq!(rgb_at(&mut surface, 85, 50), (255, 0, 0), "outside on x");
        assert_eq!(rgb_at(&mut surface, 50, 85), (255, 0, 0), "outside on y");
    }

    #[test]
    fn feather_cross_fades_the_magnified_image_back_to_the_canvas() {
        let sample_green_near_the_rim = |feather: f64| {
            let (mut surface, ctx) = marked_surface();
            render_spotlight_magnification_pass(
                &ctx,
                &[FOUR_X],
                feather,
                SpotlightMagnifierSource::CompleteSolid,
                Some((100, 100)),
                &mut SpotlightMagnifierScratch::default(),
            )
            .expect("loupe renders");
            drop(ctx);
            rgb_at(&mut surface, 78, 50).1
        };

        // Yellow has full green, the underlying red none. A hard edge keeps the
        // magnified yellow right up to the rim; a wide feather has faded most
        // of it back into the canvas by the same point.
        assert_eq!(sample_green_near_the_rim(0.0), 255, "hard edge");
        assert!(
            sample_green_near_the_rim(0.6) < 128,
            "a feathered rim must blend back toward the unmagnified canvas"
        );
    }

    #[test]
    fn a_loupe_clipped_by_the_surface_edge_still_renders() {
        let (mut surface, ctx) = marked_surface();
        let outcome = render_spotlight_magnification_pass(
            &ctx,
            &[SpotlightRegion {
                cx: 5.0,
                cy: 5.0,
                rx: 20.0,
                ry: 20.0,
                magnification: 3.0,
            }],
            0.0,
            SpotlightMagnifierSource::CompleteSolid,
            Some((100, 100)),
            &mut SpotlightMagnifierScratch::default(),
        )
        .expect("edge loupe renders");
        drop(ctx);

        assert!(matches!(outcome, SpotlightMagnifierOutcome::Rendered(_)));
        // The centre samples itself at any magnification, so a snapshot padded
        // against the clamped edge must still land the centre pixel on red.
        assert_eq!(rgb_at(&mut surface, 5, 5), (255, 0, 0));
    }

    #[test]
    fn a_fully_offscreen_loupe_needs_no_snapshot() {
        let (mut surface, ctx) = marked_surface();
        let outcome = render_spotlight_magnification_pass(
            &ctx,
            &[SpotlightRegion {
                cx: -400.0,
                cy: -400.0,
                rx: 30.0,
                ry: 30.0,
                magnification: 4.0,
            }],
            0.0,
            SpotlightMagnifierSource::CompleteSolid,
            Some((100, 100)),
            &mut SpotlightMagnifierScratch::default(),
        )
        .expect("offscreen loupe");
        drop(ctx);

        assert_eq!(outcome, SpotlightMagnifierOutcome::NotNeeded);
        assert_eq!(rgb_at(&mut surface, 50, 50), (255, 255, 0), "canvas intact");
    }

    #[test]
    fn degenerate_radii_under_magnification_do_not_panic() {
        let (mut surface, ctx) = marked_surface();
        let outcome = render_spotlight_magnification_pass(
            &ctx,
            &[SpotlightRegion {
                cx: 50.0,
                cy: 50.0,
                rx: 0.0,
                ry: 0.0,
                magnification: 4.0,
            }],
            0.35,
            SpotlightMagnifierSource::CompleteSolid,
            Some((100, 100)),
            &mut SpotlightMagnifierScratch::default(),
        )
        .expect("degenerate loupe");
        drop(ctx);

        assert!(matches!(outcome, SpotlightMagnifierOutcome::Rendered(_)));
        // A collapsed ellipse magnifies a pixel onto itself; the rest of the
        // canvas is untouched.
        assert_eq!(rgb_at(&mut surface, 90, 90), (255, 0, 0));
    }

    #[test]
    fn a_refused_allocation_reports_itself_instead_of_erroring() {
        // A recording surface is not an image surface, so the pass falls back
        // to the caller's size — here one Cairo will refuse to allocate.
        let recording = cairo::RecordingSurface::create(cairo::Content::ColorAlpha, None)
            .expect("recording surface");
        let ctx = Context::new(&recording).expect("recording context");

        let outcome = render_spotlight_magnification_pass(
            &ctx,
            &[SpotlightRegion {
                cx: 20_000.0,
                cy: 20_000.0,
                rx: 19_000.0,
                ry: 19_000.0,
                magnification: 2.0,
            }],
            0.0,
            SpotlightMagnifierSource::CompleteSolid,
            Some((40_000, 40_000)),
            &mut SpotlightMagnifierScratch::default(),
        )
        .expect("an oversized loupe must not surface a Cairo error");

        assert_eq!(outcome, SpotlightMagnifierOutcome::AllocationFailed);
    }

    #[test]
    fn a_target_with_no_known_size_skips_the_pass() {
        let recording = cairo::RecordingSurface::create(cairo::Content::ColorAlpha, None)
            .expect("recording surface");
        let ctx = Context::new(&recording).expect("recording context");

        let outcome = render_spotlight_magnification_pass(
            &ctx,
            &[FOUR_X],
            0.0,
            SpotlightMagnifierSource::CompleteSolid,
            None,
            &mut SpotlightMagnifierScratch::default(),
        )
        .expect("sizeless target");

        assert_eq!(outcome, SpotlightMagnifierOutcome::NotNeeded);
    }

    #[test]
    fn the_snapshot_strategy_crosses_over_at_half_the_target_area() {
        let strategy_for = |rx: f64, ry: f64| {
            let (_surface, ctx) = marked_surface();
            let outcome = render_spotlight_magnification_pass(
                &ctx,
                &[SpotlightRegion {
                    cx: 50.0,
                    cy: 50.0,
                    rx,
                    ry,
                    magnification: 2.0,
                }],
                0.0,
                SpotlightMagnifierSource::CompleteSolid,
                Some((100, 100)),
                &mut SpotlightMagnifierScratch::default(),
            )
            .expect("loupe renders");
            match outcome {
                SpotlightMagnifierOutcome::Rendered(metrics) => metrics.strategy,
                other => panic!("expected a rendered loupe, got {other:?}"),
            }
        };

        // Well under half of the 100x100 target: copy only the clipped bounds.
        assert_eq!(
            strategy_for(15.0, 15.0),
            SpotlightSnapshotStrategy::Regional
        );
        // Past the crossover documented above `strategy`: one full copy instead.
        assert_eq!(
            strategy_for(45.0, 45.0),
            SpotlightSnapshotStrategy::FullSurface
        );
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
            SpotlightMagnifierSource::CompleteSolid,
            Some((100, 100)),
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
            SpotlightMagnifierSource::CompleteSolid,
            Some((100, 100)),
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
            SpotlightMagnifierSource::CompleteSolid,
            Some((200, 200)),
            &mut SpotlightMagnifierScratch::default(),
        )
        .unwrap();
        drop(ctx);

        assert_eq!(rgb_at(&mut surface, 150, 100), (255, 0, 0));
        assert_eq!(rgb_at(&mut surface, 175, 100), (0, 0, 255));
    }
}
