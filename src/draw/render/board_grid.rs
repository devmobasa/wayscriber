//! Procedural board paper shared by canvas paint, eraser replay, and exports.

use cairo::{Context, RecordingSurface, Rectangle, SurfacePattern};

use crate::domain::{BoardGrid, BoardGridKind, Color};

use super::primitives::with_saved_state;

/// A compact, world-anchored tile. Screen targets reuse a small raster source
/// during erasing; vector targets retain the recording. Neither resource
/// escapes into board state or persisted snapshots.
pub struct BoardPaper {
    pattern: SurfacePattern,
}

impl BoardPaper {
    pub fn for_context(
        color: Color,
        grid: BoardGrid,
        target: &Context,
    ) -> Result<Self, cairo::Error> {
        let device_scale = board_paper_device_scale(target);
        let origin = target.device_to_user(0.0, 0.0)?;
        if !device_scale.is_finite() || device_scale <= 0.0 {
            return Err(cairo::Error::InvalidMatrix);
        }
        let (width, height) = tile_size(grid);
        let bounds = Rectangle::new(0.0, 0.0, width, height);
        let raster = target.target().type_() == cairo::SurfaceType::Image;
        // Cap a procedural tile at 4x density (under 4.5 MiB at max spacing).
        // This also bounds allocations under unusually large zoom transforms.
        let (source_width, source_height) = if raster {
            (
                (width * device_scale.min(4.0)).ceil().max(1.0),
                (height * device_scale.min(4.0)).ceil().max(1.0),
            )
        } else {
            (1.0, 1.0)
        };
        let surface: cairo::Surface = if raster {
            cairo::ImageSurface::create(
                cairo::Format::ARgb32,
                source_width as i32,
                source_height as i32,
            )?
            .as_ref()
            .clone()
        } else {
            // Fractional recording extents are rounded by Cairo when repeating
            // and can leave translucent seams. Record in an integer unit cell.
            RecordingSurface::create(
                cairo::Content::ColorAlpha,
                Some(Rectangle::new(0.0, 0.0, 1.0, 1.0)),
            )?
            .as_ref()
            .clone()
        };
        let ctx = Context::new(&surface)?;
        ctx.scale(source_width / width, source_height / height);
        paint_geometry(&ctx, color, grid, device_scale, bounds)?;
        let pattern = SurfacePattern::create(&surface);
        pattern.set_extend(cairo::Extend::Repeat);
        let mut matrix = cairo::Matrix::identity();
        matrix.scale(source_width / width, source_height / height);
        // Cairo recording replay has a finite coordinate range. Whole-cell
        // translation preserves world phase while keeping its source near zero.
        matrix.set_x0(-(origin.0 / width).floor() * source_width);
        matrix.set_y0(-(origin.1 / height).floor() * source_height);
        pattern.set_matrix(matrix);
        Ok(Self { pattern })
    }

    /// The context must already map board-world coordinates to the target.
    /// Paint does not touch its current path or leak source/operator state.
    pub fn paint(&self, ctx: &Context) -> Result<(), cairo::Error> {
        with_saved_state(ctx, || {
            ctx.set_operator(cairo::Operator::Over);
            ctx.set_source(&self.pattern)?;
            ctx.paint()
        })
    }

    pub fn pattern(&self) -> &cairo::Pattern {
        self.pattern.as_ref()
    }
}

/// Smallest target-axis scale, including Cairo surface device scaling.
pub fn board_paper_device_scale(ctx: &Context) -> f64 {
    let matrix = ctx.matrix();
    let (dx, dy) = ctx.target().device_scale();
    (matrix.xx() * dx)
        .hypot(matrix.yx() * dy)
        .min((matrix.xy() * dx).hypot(matrix.yy() * dy))
}

fn tile_size(grid: BoardGrid) -> (f64, f64) {
    let s = f64::from(grid.spacing());
    match grid.kind {
        BoardGridKind::None => (1.0, 1.0),
        BoardGridKind::Cartesian => (s, s),
        BoardGridKind::Isometric | BoardGridKind::IsometricDots => (3.0_f64.sqrt() * s, s),
    }
}

/// Also used by the full-viewport recording performance comparison. Work is
/// bounded by the requested region, never by distance from the world origin.
fn paint_geometry(
    ctx: &Context,
    color: Color,
    grid: BoardGrid,
    device_scale: f64,
    bounds: Rectangle,
) -> Result<(), cairo::Error> {
    let path = ctx.copy_path()?;
    let result = with_saved_state(ctx, || {
        ctx.new_path();
        ctx.rectangle(bounds.x(), bounds.y(), bounds.width(), bounds.height());
        ctx.clip();
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        ctx.paint()?;
        if grid.kind == BoardGridKind::None {
            return Ok(());
        }
        let s = f64::from(grid.spacing());
        let fade = (s * device_scale / 3.0).clamp(0.0, 1.0);
        let ink = if super::perceived_luminance(color.r, color.g, color.b) > 0.5 {
            0.0
        } else {
            1.0
        };
        let alpha = if grid.kind == BoardGridKind::IsometricDots {
            0.28
        } else {
            0.18
        };
        ctx.set_source_rgba(ink, ink, ink, alpha * fade);
        ctx.set_line_width(1.0);
        ctx.set_line_cap(cairo::LineCap::Butt);
        ctx.set_line_join(cairo::LineJoin::Miter);
        ctx.set_dash(&[], 0.0);
        match grid.kind {
            BoardGridKind::None => {}
            BoardGridKind::Cartesian => cartesian_path(ctx, s, bounds),
            BoardGridKind::Isometric => isometric_path(ctx, s, bounds),
            BoardGridKind::IsometricDots => isometric_dots(ctx, s, bounds),
        }
        if grid.kind == BoardGridKind::IsometricDots {
            ctx.fill()
        } else {
            ctx.stroke()
        }
    });
    ctx.new_path();
    ctx.append_path(&path);
    result
}

fn indices(min: f64, max: f64, spacing: f64) -> std::ops::RangeInclusive<i64> {
    (min / spacing).floor() as i64..=(max / spacing).ceil() as i64
}

fn cartesian_path(ctx: &Context, s: f64, b: Rectangle) {
    for n in indices(b.x() - 1.0, b.x() + b.width() + 1.0, s) {
        let x = n as f64 * s;
        ctx.move_to(x, b.y() - 1.0);
        ctx.line_to(x, b.y() + b.height() + 1.0);
    }
    for n in indices(b.y() - 1.0, b.y() + b.height() + 1.0, s) {
        let y = n as f64 * s;
        ctx.move_to(b.x() - 1.0, y);
        ctx.line_to(b.x() + b.width() + 1.0, y);
    }
}

fn isometric_path(ctx: &Context, s: f64, b: Rectangle) {
    let column = 3.0_f64.sqrt() * s / 2.0;
    for n in indices(b.x() - 1.0, b.x() + b.width() + 1.0, column) {
        let x = n as f64 * column;
        ctx.move_to(x, b.y() - 1.0);
        ctx.line_to(x, b.y() + b.height() + 1.0);
    }
    let left = b.x() - 2.0;
    let right = b.x() + b.width() + 2.0;
    for slope in [-1.0 / 3.0_f64.sqrt(), 1.0 / 3.0_f64.sqrt()] {
        let min = b.y() - (slope * left).max(slope * right) - 2.0;
        let max = b.y() + b.height() - (slope * left).min(slope * right) + 2.0;
        for n in indices(min, max, s) {
            let intercept = n as f64 * s;
            ctx.move_to(left, slope * left + intercept);
            ctx.line_to(right, slope * right + intercept);
        }
    }
}

fn isometric_dots(ctx: &Context, s: f64, b: Rectangle) {
    let column = 3.0_f64.sqrt() * s / 2.0;
    for i in indices(b.x() - 1.25, b.x() + b.width() + 1.25, column) {
        let x = i as f64 * column;
        let shift = i.rem_euclid(2) as f64 * s / 2.0;
        for j in indices(b.y() - shift - 1.25, b.y() + b.height() - shift + 1.25, s) {
            ctx.new_sub_path();
            ctx.arc(x, j as f64 * s + shift, 1.25, 0.0, std::f64::consts::TAU);
        }
    }
}

#[cfg(test)]
mod performance;
#[cfg(test)]
mod tests;
