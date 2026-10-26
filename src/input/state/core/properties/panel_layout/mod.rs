use crate::util::Rect;

const PANEL_MARGIN: f64 = 12.0;
const PANEL_ANCHOR_GAP: f64 = 12.0;
const PANEL_POINTER_OFFSET: f64 = 16.0;

mod focus;
mod geometry;
mod interaction;
mod layout;
#[cfg(test)]
mod tests;

pub(super) fn selection_panel_anchor(bounds: Option<Rect>, pointer: (i32, i32)) -> (f64, f64) {
    bounds.map_or_else(
        || {
            let (px, py) = pointer;
            (
                f64::from(px) + PANEL_POINTER_OFFSET,
                f64::from(py) - PANEL_POINTER_OFFSET,
            )
        },
        |rect| {
            (
                f64::from(rect.x) + f64::from(rect.width) + PANEL_ANCHOR_GAP,
                (f64::from(rect.y) - PANEL_ANCHOR_GAP).max(PANEL_MARGIN),
            )
        },
    )
}
