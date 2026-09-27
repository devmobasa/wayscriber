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
    bounds
        .map(|rect| {
            (
                rect.x as f64 + rect.width as f64 + PANEL_ANCHOR_GAP,
                (rect.y as f64 - PANEL_ANCHOR_GAP).max(PANEL_MARGIN),
            )
        })
        .unwrap_or_else(|| {
            let (px, py) = pointer;
            (
                px as f64 + PANEL_POINTER_OFFSET,
                py as f64 - PANEL_POINTER_OFFSET,
            )
        })
}
