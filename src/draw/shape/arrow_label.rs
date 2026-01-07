use crate::draw::FontDescriptor;
use crate::util::Rect;

use super::text::{text_bounds_from_metrics, text_layout_metrics};

pub(crate) const ARROW_LABEL_BACKGROUND: bool = true;

const LABEL_OFFSET_SCALE: f64 = 0.6;
const LABEL_OFFSET_MIN: f64 = 6.0;
const LABEL_THICKNESS_SCALE: f64 = 0.4;
const LABEL_ALONG_RATIO: f64 = 0.5;

pub(crate) struct ArrowLabelLayout {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) bounds: Rect,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn arrow_label_layout(
    tip_x: i32,
    tip_y: i32,
    tail_x: i32,
    tail_y: i32,
    thick: f64,
    label_text: &str,
    label_size: f64,
    font_descriptor: &FontDescriptor,
) -> Option<ArrowLabelLayout> {
    if label_text.is_empty() {
        return None;
    }

    let dx = (tip_x - tail_x) as f64;
    let dy = (tip_y - tail_y) as f64;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f64::EPSILON {
        return None;
    }

    let ux = dx / len;
    let uy = dy / len;
    let nx = -uy;
    let ny = ux;

    let along = len * LABEL_ALONG_RATIO;
    let offset =
        (label_size * LABEL_OFFSET_SCALE).max(LABEL_OFFSET_MIN) + thick * LABEL_THICKNESS_SCALE;

    let anchor_x = tail_x as f64 + ux * along + nx * offset;
    let anchor_y = tail_y as f64 + uy * along + ny * offset;

    let metrics = text_layout_metrics(label_text, label_size, font_descriptor, None)?;
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
