use crate::draw::{ArrowStyle, FontDescriptor};
use crate::util::Rect;

use super::text::{text_bounds_from_metrics, text_layout_metrics};
use super::text_cache::{TextMeasurer, with_legacy_measurer};

pub(crate) const ARROW_LABEL_BACKGROUND: bool = true;

/// The tip/tail pair an arrow's label is placed against.
///
/// For every style but [`ArrowStyle::Double`] this is just the arrow's own
/// tip and tail, which is what puts the number on a consistent side of the
/// shaft as the arrow is redrawn.
///
/// `Double` has a head on both ends, so `head_at_end` describes nothing about
/// it — the documented contract is that the setting has no effect on a double
/// arrow, and the outline it produces is the same polygon either way. Letting
/// the flag choose the label's side anyway would mean toggling Arrow Head
/// mirrors the number across a shaft that did not move, taking the label's
/// bounds and its hit area with it. Pinning `Double` to the `head_at_end`
/// reading keeps the contract and leaves every already-drawn double arrow
/// (the flag defaults to true) exactly where it is.
pub(crate) fn arrow_label_ends(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    head_at_end: bool,
    style: ArrowStyle,
) -> (i32, i32, i32, i32) {
    if head_at_end || style == ArrowStyle::Double {
        (x2, y2, x1, y1)
    } else {
        (x1, y1, x2, y2)
    }
}

const LABEL_OFFSET_SCALE: f64 = 0.6;
const LABEL_OFFSET_MIN: f64 = 6.0;
const LABEL_THICKNESS_SCALE: f64 = 0.4;
const LABEL_ALONG_RATIO: f64 = 0.5;

pub(crate) struct ArrowLabelLayout {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) bounds: Rect,
}

/// Places an arrow's auto-number label beside the middle of its shaft.
///
/// `bend` is the bend the shaft actually draws — callers gate the stored value
/// through [`ArrowStyle::effective_bend`] first. A bent shaft leaves the chord,
/// so a label anchored to the chord's midpoint would float in the gap the arrow
/// was drawn to route around; the anchor follows the arc instead, and sits on
/// the outside of the curve where there is room for it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn arrow_label_layout(
    tip_x: i32,
    tip_y: i32,
    tail_x: i32,
    tail_y: i32,
    thick: f64,
    bend: f64,
    label_text: &str,
    label_size: f64,
    font_descriptor: &FontDescriptor,
) -> Option<ArrowLabelLayout> {
    with_legacy_measurer(|measurer| {
        arrow_label_layout_with(
            measurer,
            tip_x,
            tip_y,
            tail_x,
            tail_y,
            thick,
            bend,
            label_text,
            label_size,
            font_descriptor,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn arrow_label_layout_with(
    measurer: &TextMeasurer,
    tip_x: i32,
    tip_y: i32,
    tail_x: i32,
    tail_y: i32,
    thick: f64,
    bend: f64,
    label_text: &str,
    label_size: f64,
    font_descriptor: &FontDescriptor,
) -> Option<ArrowLabelLayout> {
    if label_text.is_empty() {
        return None;
    }

    let dx = f64::from(tip_x) - f64::from(tail_x);
    let dy = f64::from(tip_y) - f64::from(tail_y);
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f64::EPSILON {
        return None;
    }

    let ux = dx / len;
    let uy = dy / len;

    let along = len * LABEL_ALONG_RATIO;
    let offset =
        (label_size * LABEL_OFFSET_SCALE).max(LABEL_OFFSET_MIN) + thick * LABEL_THICKNESS_SCALE;

    let bend = crate::util::clamp_arrow_bend(bend);
    // Left normal of the tail-to-tip direction, matching the convention the
    // arc's control point is offset along.
    let (left_x, left_y) = (uy, -ux);
    let (base_x, base_y, nx, ny) = if bend == 0.0 {
        // Straight shaft: the historical placement, one side of the chord.
        (
            tail_x as f64 + ux * along,
            tail_y as f64 + uy * along,
            -uy,
            ux,
        )
    } else {
        // The arc's own midpoint sits half the control point's offset out.
        let bulge = bend * len / 2.0;
        let side = bend.signum();
        (
            tail_x as f64 + ux * along + left_x * bulge,
            tail_y as f64 + uy * along + left_y * bulge,
            left_x * side,
            left_y * side,
        )
    };

    let anchor_x = base_x + nx * offset;
    let anchor_y = base_y + ny * offset;

    let metrics = text_layout_metrics(measurer, label_text, label_size, font_descriptor, None)?;
    let center_offset_x = metrics.ink_x + metrics.ink_width / 2.0;
    let center_offset_y = metrics.ink_y + metrics.ink_height / 2.0;

    let baseline_x = (anchor_x - center_offset_x).round();
    let baseline_y = (anchor_y - center_offset_y + metrics.baseline).round();

    let bounds = text_bounds_from_metrics(
        baseline_x,
        baseline_y,
        &metrics,
        label_size,
        ARROW_LABEL_BACKGROUND,
        None,
    )?;

    Some(ArrowLabelLayout {
        x: baseline_x as i32,
        y: baseline_y as i32,
        bounds,
    })
}
