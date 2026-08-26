use crate::draw::ArrowStyle;

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
    let (shoulder_half, tail_half) = axis.shaft_half_widths(thick);

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

/// Where the dart notch of [`ArrowStyle::Pointy`] sits, as a fraction of the
/// distance from the tip back to the head base.
///
/// This is [`HEAD_SWEEP_RATIO`] deliberately pulled past the depth that
/// constant's comment warns about: for a dart, the barbs sweeping clear of the
/// shaft *is* the shape. It still has a floor — below roughly `0.4` the barbs
/// grow long enough to read as two separate spikes with a stick between them
/// rather than as one head.
const POINTY_NOTCH_RATIO: f64 = 0.55;

/// Samples per pixel of chord length used to walk a curved shaft.
///
/// One sample per eight pixels, floored and capped by the two constants below,
/// so a short arc still gets enough points to hide its facets and a
/// screen-wide one does not pay for hundreds it cannot show.
const CURVE_SAMPLES_PER_PX: f64 = 1.0 / 8.0;
const MIN_CURVE_SEGMENTS: usize = 12;
const MAX_CURVE_SEGMENTS: usize = 64;

/// Default bend for a newly drawn [`ArrowStyle::Curved`] arrow.
///
/// A curved arrow created with `bend` at zero would draw exactly like a
/// standard one, so picking the style would appear to do nothing. This is the
/// smallest arc that reads as deliberate rather than as a wobble.
pub(crate) const DEFAULT_ARROW_BEND: f64 = 0.25;

/// Cap on `bend`, as a fraction of the chord length.
///
/// The arc's furthest point sits `bend / 2` chord-lengths off the chord, so
/// this puts the bulge half a chord out — already a quarter-circle. Past it the
/// arrow stops pointing at anything recognizable.
pub(crate) const MAX_ARROW_BEND: f64 = 1.0;

/// Clamps a bend to the range the geometry is defined over.
pub(crate) fn clamp_arrow_bend(bend: f64) -> f64 {
    if bend.is_finite() {
        bend.clamp(-MAX_ARROW_BEND, MAX_ARROW_BEND)
    } else {
        0.0
    }
}

/// The bend that carries a curved arrow's arc through a scale transform.
///
/// `bend` is stored as a fraction of the chord, so a *uniform* scale needs no
/// work: chord and bulge grow together. A non-uniform one does. Dragging the
/// bottom handle of a horizontal curved arrow leaves the chord alone and so
/// leaves the bulge alone, and the arc — the only part of that arrow with any
/// height — refuses to follow the pointer.
///
/// The fix is to scale the arc itself. A quadratic Bezier maps through an
/// affine transform by mapping its control point, and the control point's
/// offset from the chord midpoint maps through the transform's linear part
/// alone, so the anchor never enters: the caller passes the same `scale_x` and
/// `scale_y` it used on the endpoints. Projecting the scaled offset back onto
/// the new chord's normal drops any component that has rotated to lie *along*
/// the chord, which is what keeps the arc symmetric — the single-scalar `bend`
/// has nowhere to put a lopsided arc anyway.
///
/// Endpoints are whichever pair the caller holds; they need not be tail-first.
/// Naming them backwards flips the normal on both sides of the projection, and
/// the two sign flips cancel.
pub(crate) fn scaled_arrow_bend(
    old_start: (f64, f64),
    old_end: (f64, f64),
    new_start: (f64, f64),
    new_end: (f64, f64),
    bend: f64,
    scale_x: f64,
    scale_y: f64,
) -> f64 {
    let bend = clamp_arrow_bend(bend);
    if bend == 0.0 || !scale_x.is_finite() || !scale_y.is_finite() {
        return bend;
    }
    let Some((old_perp, old_chord)) = chord_normal(old_start, old_end) else {
        return bend;
    };
    let Some((new_perp, new_chord)) = chord_normal(new_start, new_end) else {
        return bend;
    };
    let offset = (
        old_perp.0 * bend * old_chord * scale_x,
        old_perp.1 * bend * old_chord * scale_y,
    );
    clamp_arrow_bend((offset.0 * new_perp.0 + offset.1 * new_perp.1) / new_chord)
}

/// Left normal of `start` -> `end` and the chord length, or `None` when the two
/// points are too close together for the direction to mean anything.
///
/// "Left" is in screen coordinates, where y grows downward, and matches
/// `ArrowAxis::perp`. Every consumer of a signed bend measures against this one
/// normal, so the sign documented on `Shape::Arrow::bend` means the same thing
/// to the renderer, the bend handle, and a resize.
pub(crate) fn chord_normal(start: (f64, f64), end: (f64, f64)) -> Option<((f64, f64), f64)> {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let chord = (dx * dx + dy * dy).sqrt();
    if !chord.is_finite() || chord < MIN_CHORD_LENGTH {
        return None;
    }
    Some(((dy / chord, -dx / chord), chord))
}

/// Shortest chord a bend is defined over.
///
/// Below this the normal is dominated by the endpoints' own rounding to whole
/// pixels, so a recomputed bend would be noise.
const MIN_CHORD_LENGTH: f64 = 1.0;

/// The shaft's centre line, from tail to tip.
///
/// Straight styles carry the chord itself and allocate nothing; only
/// [`ArrowStyle::Curved`] pays for the sampled polyline. Bounds and
/// hit-testing both read this rather than walking the curve themselves, which
/// is what keeps them from drifting apart from the renderer.
#[derive(Debug, Clone)]
pub(crate) enum ArrowSpine {
    Straight([(f64, f64); 2]),
    Curved(Vec<(f64, f64)>),
}

impl ArrowSpine {
    pub(crate) fn points(&self) -> &[(f64, f64)] {
        match self {
            Self::Straight(points) => points.as_slice(),
            Self::Curved(points) => points.as_slice(),
        }
    }
}

/// The non-filled geometry every arrow consumer other than the renderer needs:
/// the head triangles to test and bound, plus the shaft's centre line.
#[derive(Debug, Clone)]
pub(crate) struct ArrowSkeleton {
    /// Head at the tip. Aimed along the curve's end tangent when the style bends.
    pub head: ArrowheadTriangle,
    /// Second head at the tail. `Some` only for [`ArrowStyle::Double`].
    pub tail_head: Option<ArrowheadTriangle>,
    pub spine: ArrowSpine,
}

/// One point on a sampled quadratic Bezier, with the direction of travel there.
#[derive(Debug, Clone, Copy)]
struct CurveSample {
    point: (f64, f64),
    /// Unit tangent pointing from the tail toward the tip.
    tangent: (f64, f64),
    /// Arc length from the tail, approximated along the sampled polyline.
    arc: f64,
}

impl ArrowAxis {
    /// Unit vector pointing from the tail toward the tip.
    fn toward_tip(&self) -> (f64, f64) {
        (-self.toward_tail.0, -self.toward_tail.1)
    }

    /// Point `along` pixels from the tail toward the tip, on the chord.
    fn along_from_tail(&self, along: f64) -> (f64, f64) {
        let forward = self.toward_tip();
        (
            self.tail.0 + forward.0 * along,
            self.tail.1 + forward.1 * along,
        )
    }

    /// Half-width where the shaft meets the head, and at the tapered tail.
    ///
    /// Shared by every style so the taper tuning stays in one place; `Double`
    /// takes only the shoulder value and keeps it the whole way.
    fn shaft_half_widths(&self, thick: f64) -> (f64, f64) {
        // The shaft never pokes outside the head it feeds into, which has already
        // narrowed by `HEAD_SWEEP_RATIO` by the time it reaches the notch. The
        // `thick * 0.6` floor on the head keeps that product above `thick / 2`, so
        // the shaft still joins at its full width.
        let shoulder_half = (thick / 2.0).min(self.head_half_base * HEAD_SWEEP_RATIO);
        let tail_half = (shoulder_half * TAIL_TAPER_RATIO)
            .max(MIN_TAIL_HALF_WIDTH)
            .min(shoulder_half);
        (shoulder_half, tail_half)
    }

    /// Head triangle at the tip, aimed straight down the chord.
    fn head_triangle(&self) -> ArrowheadTriangle {
        let base = self.base();
        ArrowheadTriangle {
            tip: self.tip,
            left: self.offset(base, 1.0, self.head_half_base),
            right: self.offset(base, -1.0, self.head_half_base),
        }
    }

    /// Head triangle at the tail end, for [`ArrowStyle::Double`].
    fn tail_head_triangle(&self) -> ArrowheadTriangle {
        let base = self.along_from_tail(self.head_length);
        ArrowheadTriangle {
            tip: self.tail,
            left: self.offset(base, 1.0, self.head_half_base),
            right: self.offset(base, -1.0, self.head_half_base),
        }
    }

    /// Control point of the shaft's quadratic Bezier.
    ///
    /// Sits on the chord's perpendicular bisector, offset by `bend` chord
    /// lengths toward the left of the tail-to-tip direction. `perp` is already
    /// that left normal, so a positive bend bulges left and a negative one
    /// right, matching the sign convention documented on `Shape::Arrow::bend`.
    fn curve_control(&self, bend: f64) -> (f64, f64) {
        let chord_len = self.chord_length();
        let mid = (
            (self.tip.0 + self.tail.0) / 2.0,
            (self.tip.1 + self.tail.1) / 2.0,
        );
        let offset = clamp_arrow_bend(bend) * chord_len;
        (mid.0 + self.perp.0 * offset, mid.1 + self.perp.1 * offset)
    }

    fn chord_length(&self) -> f64 {
        let dx = self.tail.0 - self.tip.0;
        let dy = self.tail.1 - self.tip.1;
        (dx * dx + dy * dy).sqrt()
    }

    /// Walks the shaft's Bezier from tail to tip.
    ///
    /// This is the only place the curve is evaluated. The renderer, the
    /// dirty-region bounds, and hit-testing all consume its output, so a curved
    /// arrow cannot end up drawn along one path and tested along another.
    fn sample_curve(&self, bend: f64) -> Vec<CurveSample> {
        let control = self.curve_control(bend);
        let segments = ((self.chord_length() * CURVE_SAMPLES_PER_PX).round() as usize)
            .clamp(MIN_CURVE_SEGMENTS, MAX_CURVE_SEGMENTS);

        let mut samples = Vec::with_capacity(segments + 1);
        let mut arc = 0.0;
        let mut previous: Option<(f64, f64)> = None;

        for step in 0..=segments {
            let t = step as f64 / segments as f64;
            let inv = 1.0 - t;
            let point = (
                inv * inv * self.tail.0 + 2.0 * t * inv * control.0 + t * t * self.tip.0,
                inv * inv * self.tail.1 + 2.0 * t * inv * control.1 + t * t * self.tip.1,
            );

            // B'(t) = 2(1-t)(C - A) + 2t(T - C). Degenerate only if the control
            // point lands exactly on both endpoints, which means a zero-length
            // arrow, and `arrow_axis` has already rejected those.
            let raw = (
                2.0 * inv * (control.0 - self.tail.0) + 2.0 * t * (self.tip.0 - control.0),
                2.0 * inv * (control.1 - self.tail.1) + 2.0 * t * (self.tip.1 - control.1),
            );
            let len = (raw.0 * raw.0 + raw.1 * raw.1).sqrt();
            let tangent = if len > f64::EPSILON {
                (raw.0 / len, raw.1 / len)
            } else {
                self.toward_tip()
            };

            if let Some(prev) = previous {
                let dx = point.0 - prev.0;
                let dy = point.1 - prev.1;
                arc += (dx * dx + dy * dy).sqrt();
            }
            previous = Some(point);

            samples.push(CurveSample {
                point,
                tangent,
                arc,
            });
        }

        samples
    }
}

/// Left normal of a unit tangent, matching [`ArrowAxis::perp`]'s convention.
fn left_normal(tangent: (f64, f64)) -> (f64, f64) {
    (tangent.1, -tangent.0)
}

fn offset_by(point: (f64, f64), normal: (f64, f64), half_width: f64) -> (f64, f64) {
    (
        point.0 + normal.0 * half_width,
        point.1 + normal.1 * half_width,
    )
}

/// Head triangle plus its shaft-join notch for a curved arrow's tip.
///
/// Aimed along the tangent at `t = 1`, not along the chord. On a strongly bent
/// arrow those differ by tens of degrees, and a head aimed down the chord
/// visibly points somewhere the arrow does not.
fn curved_head(axis: &ArrowAxis, end_tangent: (f64, f64)) -> (ArrowheadTriangle, (f64, f64)) {
    let backward = (-end_tangent.0, -end_tangent.1);
    let normal = left_normal(end_tangent);
    let base = (
        axis.tip.0 + backward.0 * axis.head_length,
        axis.tip.1 + backward.1 * axis.head_length,
    );
    let notch_along = axis.head_length * HEAD_SWEEP_RATIO;
    let notch = (
        axis.tip.0 + backward.0 * notch_along,
        axis.tip.1 + backward.1 * notch_along,
    );

    (
        ArrowheadTriangle {
            tip: axis.tip,
            left: offset_by(base, normal, axis.head_half_base),
            right: offset_by(base, normal, -axis.head_half_base),
        },
        notch,
    )
}

/// Head triangles and shaft centre line for one arrow, in the form bounds and
/// hit-testing consume.
///
/// Straight styles cost no allocation here; `Curved` pays for one sampled
/// polyline, which both callers then share.
#[allow(clippy::too_many_arguments)]
pub(crate) fn calculate_arrow_skeleton(
    tip_x: i32,
    tip_y: i32,
    tail_x: i32,
    tail_y: i32,
    thick: f64,
    arrow_length: f64,
    arrow_angle: f64,
    style: ArrowStyle,
    bend: f64,
) -> Option<ArrowSkeleton> {
    let axis = arrow_axis(
        tip_x,
        tip_y,
        tail_x,
        tail_y,
        thick,
        arrow_length,
        arrow_angle,
    )?;

    if style.is_curved() {
        let samples = axis.sample_curve(bend);
        let end_tangent = samples.last()?.tangent;
        let (head, _) = curved_head(&axis, end_tangent);
        return Some(ArrowSkeleton {
            head,
            tail_head: None,
            spine: ArrowSpine::Curved(samples.into_iter().map(|sample| sample.point).collect()),
        });
    }

    Some(ArrowSkeleton {
        head: axis.head_triangle(),
        tail_head: match style {
            ArrowStyle::Double => Some(axis.tail_head_triangle()),
            _ => None,
        },
        spine: ArrowSpine::Straight([axis.tail, axis.tip]),
    })
}

/// Calculates the arrow's single filled outline for any style.
///
/// Every style is one closed polygon, including `Double`: fusing the second
/// head into the shaft rather than filling it separately is what keeps a
/// semi-transparent color from painting a seam where the two overlap.
///
/// `ArrowStyle::Standard` returns exactly the points [`calculate_arrow_outline`]
/// produces, so sessions drawn before styles existed render unchanged.
#[allow(clippy::too_many_arguments)]
pub(crate) fn calculate_arrow_outline_styled(
    tip_x: i32,
    tip_y: i32,
    tail_x: i32,
    tail_y: i32,
    thick: f64,
    arrow_length: f64,
    arrow_angle: f64,
    style: ArrowStyle,
    bend: f64,
) -> Option<Vec<(f64, f64)>> {
    let axis = arrow_axis(
        tip_x,
        tip_y,
        tail_x,
        tail_y,
        thick,
        arrow_length,
        arrow_angle,
    )?;
    let (shoulder_half, tail_half) = axis.shaft_half_widths(thick);

    let points = match style {
        ArrowStyle::Standard => calculate_arrow_outline(
            tip_x,
            tip_y,
            tail_x,
            tail_y,
            thick,
            arrow_length,
            arrow_angle,
        )?
        .points
        .to_vec(),
        ArrowStyle::Pointy => {
            let base = axis.base();
            let notch_along = axis.head_length * POINTY_NOTCH_RATIO;
            let notch = (
                axis.tip.0 + axis.toward_tail.0 * notch_along,
                axis.tip.1 + axis.toward_tail.1 * notch_along,
            );
            vec![
                axis.offset(axis.tail, 1.0, tail_half),
                axis.offset(notch, 1.0, shoulder_half),
                axis.offset(base, 1.0, axis.head_half_base),
                axis.tip,
                axis.offset(base, -1.0, axis.head_half_base),
                axis.offset(notch, -1.0, shoulder_half),
                axis.offset(axis.tail, -1.0, tail_half),
            ]
        }
        ArrowStyle::Double => {
            // Both ends are heads, so there is no tail to taper into: the shaft
            // keeps its shoulder width the whole way and the silhouette stays
            // symmetric end to end.
            let base = axis.base();
            let notch = axis.notch();
            let tail_base = axis.along_from_tail(axis.head_length);
            let tail_notch = axis.along_from_tail(axis.head_length * HEAD_SWEEP_RATIO);
            vec![
                axis.tail,
                axis.offset(tail_base, 1.0, axis.head_half_base),
                axis.offset(tail_notch, 1.0, shoulder_half),
                axis.offset(notch, 1.0, shoulder_half),
                axis.offset(base, 1.0, axis.head_half_base),
                axis.tip,
                axis.offset(base, -1.0, axis.head_half_base),
                axis.offset(notch, -1.0, shoulder_half),
                axis.offset(tail_notch, -1.0, shoulder_half),
                axis.offset(tail_base, -1.0, axis.head_half_base),
            ]
        }
        ArrowStyle::Curved => {
            let samples = axis.sample_curve(bend);
            let end_tangent = samples.last()?.tangent;
            let (head, notch) = curved_head(&axis, end_tangent);
            let head_normal = left_normal(end_tangent);

            // The shaft stops where the head's rear bevel starts, so the two
            // meet at the notch exactly as they do on a straight arrow.
            let total_arc = samples.last()?.arc;
            let join_arc = (total_arc - axis.head_length * HEAD_SWEEP_RATIO).max(f64::EPSILON);

            let mut left = Vec::with_capacity(samples.len() + 5);
            let mut right = Vec::with_capacity(samples.len());
            for sample in samples.iter().take_while(|s| s.arc <= join_arc) {
                let progress = (sample.arc / join_arc).clamp(0.0, 1.0);
                let half_width = tail_half + (shoulder_half - tail_half) * progress;
                let normal = left_normal(sample.tangent);
                left.push(offset_by(sample.point, normal, half_width));
                right.push(offset_by(sample.point, normal, -half_width));
            }

            left.push(offset_by(notch, head_normal, shoulder_half));
            left.push(head.left);
            left.push(head.tip);
            left.push(head.right);
            left.push(offset_by(notch, head_normal, -shoulder_half));
            left.extend(right.into_iter().rev());
            left
        }
    };

    Some(points)
}
