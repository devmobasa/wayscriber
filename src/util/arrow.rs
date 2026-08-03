/// Half-width of the shaft at the tail, as a fraction of its half-width where it
/// meets the arrowhead. The taper is what makes an arrow read as directional
/// rather than as a plain line with a triangle stuck on the end, and a strong one
/// gives the shaft a drawn, brush-like swell into the head.
///
/// [`MIN_TAIL_HALF_WIDTH`] is what keeps this from thinning a hairline stroke
/// into nothing, so the ratio can stay aggressive without breaking thin pens.
const TAIL_TAPER_RATIO: f64 = 0.25;

/// Head length as a multiple of stroke width.
///
/// The head has to grow with the stroke, or a thick arrow ends up with a stub
/// head that reads as a thin line with a nub on the end. Kept modest, though:
/// past roughly three times the stroke the head starts to overpower the shaft.
/// `arrow.length` stays meaningful as the floor, so hairline strokes still get a
/// visible head without inheriting a head sized for a thick one.
const HEAD_LENGTH_PER_THICKNESS: f64 = 3.0;

/// Floor for the tapered tail so thin arrows keep a visible tail instead of
/// fading into sub-pixel coverage.
const MIN_TAIL_HALF_WIDTH: f64 = 0.55;

/// Where the shaft joins the head, as a fraction of the distance from the tip
/// back to the arrowhead base.
///
/// At `1.0` the shaft meets the base flush and the whole rear of the head is one
/// straight line across, which reads as a triangle parked on a stick. Pulling
/// the join forward bevels each rear edge inward so the head wedges into the
/// shaft instead.
///
/// This has to stay shallow. Around `0.8` the bevel deepens into a real gap
/// between barb and shaft and the head stops reading as part of the same arrow —
/// the silhouette splits into a triangle balanced on a neck.
const HEAD_SWEEP_RATIO: f64 = 0.90;

/// Arrowhead triangle geometry used by hit-testing and dirty-region bounds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ArrowheadTriangle {
    pub tip: (f64, f64),
    pub left: (f64, f64),
    pub right: (f64, f64),
}

/// The arrow's filled outline: a tapered shaft fused into the arrowhead.
///
/// Points are in path order starting at the tail's left edge, so the renderer
/// can walk them straight into a single closed Cairo path.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ArrowOutline {
    pub points: [(f64, f64); 7],
}

/// The shaft axis plus the arrowhead sizing every arrow consumer needs.
///
/// Both the head triangle and the full outline derive from this, so the
/// renderer, hit-testing, and dirty-region bounds cannot drift apart.
struct ArrowAxis {
    tip: (f64, f64),
    tail: (f64, f64),
    /// Unit vector pointing from the tip toward the tail.
    toward_tail: (f64, f64),
    /// Unit vector perpendicular to the shaft.
    perp: (f64, f64),
    /// Distance from the tip back to the arrowhead base.
    head_length: f64,
    /// Half-width of the arrowhead at its base.
    head_half_base: f64,
}

fn arrow_axis(
    tip_x: i32,
    tip_y: i32,
    tail_x: i32,
    tail_y: i32,
    thick: f64,
    arrow_length: f64,
    arrow_angle: f64,
) -> Option<ArrowAxis> {
    let tip_x = tip_x as f64;
    let tip_y = tip_y as f64;
    let tail_x = tail_x as f64;
    let tail_y = tail_y as f64;

    let dir_x = tail_x - tip_x;
    let dir_y = tail_y - tip_y;
    let line_length = (dir_x * dir_x + dir_y * dir_y).sqrt();
    if line_length < 1.0 {
        return None;
    }

    // Direction from tip toward tail.
    let ux = dir_x / line_length;
    let uy = dir_y / line_length;

    // Keep heads visible for thick strokes but avoid oversized heads on short lines.
    let scaled_length = arrow_length.max(thick * HEAD_LENGTH_PER_THICKNESS);
    let head_length = scaled_length.min(line_length * 0.4);

    let angle_rad = arrow_angle.to_radians();
    let half_base_from_angle = head_length * angle_rad.tan();
    let head_half_base = half_base_from_angle.max(thick * 0.6);

    Some(ArrowAxis {
        tip: (tip_x, tip_y),
        tail: (tail_x, tail_y),
        toward_tail: (ux, uy),
        perp: (-uy, ux),
        head_length,
        head_half_base,
    })
}

impl ArrowAxis {
    /// Offsets a point on the shaft axis sideways by `half_width * side`.
    fn offset(&self, point: (f64, f64), side: f64, half_width: f64) -> (f64, f64) {
        (
            point.0 + self.perp.0 * half_width * side,
            point.1 + self.perp.1 * half_width * side,
        )
    }

    fn base(&self) -> (f64, f64) {
        (
            self.tip.0 + self.toward_tail.0 * self.head_length,
            self.tip.1 + self.toward_tail.1 * self.head_length,
        )
    }

    /// Point on the shaft axis where the shaft joins the head.
    ///
    /// Sits just forward of [`ArrowAxis::base`], so each rear edge bevels
    /// inward instead of running straight across. Stays inside the head
    /// triangle, which is what lets hit-testing and dirty-region bounds keep
    /// using that triangle alone.
    fn notch(&self) -> (f64, f64) {
        let along = self.head_length * HEAD_SWEEP_RATIO;
        (
            self.tip.0 + self.toward_tail.0 * along,
            self.tip.1 + self.toward_tail.1 * along,
        )
    }
}

/// Calculates arrowhead triangle points matching the renderer's geometry model.
///
/// This helper must remain in sync with `render_arrow` so dirty-region bounds and
/// hit-testing stay aligned with the visual arrowhead.
#[allow(clippy::too_many_arguments)]
pub(crate) fn calculate_arrowhead_triangle_custom(
    tip_x: i32,
    tip_y: i32,
    tail_x: i32,
    tail_y: i32,
    thick: f64,
    arrow_length: f64,
    arrow_angle: f64,
) -> Option<ArrowheadTriangle> {
    let axis = arrow_axis(
        tip_x,
        tip_y,
        tail_x,
        tail_y,
        thick,
        arrow_length,
        arrow_angle,
    )?;
    let base = axis.base();

    Some(ArrowheadTriangle {
        tip: axis.tip,
        left: axis.offset(base, 1.0, axis.head_half_base),
        right: axis.offset(base, -1.0, axis.head_half_base),
    })
}

/// Calculates the arrow's single filled outline: tapered shaft plus arrowhead.
///
/// The tail is narrower than the shoulder where the shaft meets the head, and
/// both are emitted as one closed polygon so there is no seam to show through a
/// semi-transparent color and no width step at the shoulders.
///
/// The shoulders sit slightly forward of the head base, so the rear of the head
/// bevels into the shaft rather than running straight across it. The outline
/// stays within the head triangle that hit-testing and dirty-region bounds use.
#[allow(clippy::too_many_arguments)]
pub(crate) fn calculate_arrow_outline(
    tip_x: i32,
    tip_y: i32,
    tail_x: i32,
    tail_y: i32,
    thick: f64,
    arrow_length: f64,
    arrow_angle: f64,
) -> Option<ArrowOutline> {
    let axis = arrow_axis(
        tip_x,
        tip_y,
        tail_x,
        tail_y,
        thick,
        arrow_length,
        arrow_angle,
    )?;
    let base = axis.base();
    let notch = axis.notch();

    // The shaft never pokes outside the head it feeds into, which has already
    // narrowed by `HEAD_SWEEP_RATIO` by the time it reaches the notch. The
    // `thick * 0.6` floor on the head keeps that product above `thick / 2`, so
    // the shaft still joins at its full width.
    let shoulder_half = (thick / 2.0).min(axis.head_half_base * HEAD_SWEEP_RATIO);
    let tail_half = (shoulder_half * TAIL_TAPER_RATIO)
        .max(MIN_TAIL_HALF_WIDTH)
        .min(shoulder_half);

    Some(ArrowOutline {
        points: [
            axis.offset(axis.tail, 1.0, tail_half),
            axis.offset(notch, 1.0, shoulder_half),
            axis.offset(base, 1.0, axis.head_half_base),
            axis.tip,
            axis.offset(base, -1.0, axis.head_half_base),
            axis.offset(notch, -1.0, shoulder_half),
            axis.offset(axis.tail, -1.0, tail_half),
        ],
    })
}
