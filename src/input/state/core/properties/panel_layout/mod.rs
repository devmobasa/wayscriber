use crate::util::Rect;

const PANEL_TITLE_FONT: f64 = 15.0;
const PANEL_BODY_FONT: f64 = 13.0;
const PANEL_LINE_HEIGHT: f64 = 18.0;
const PANEL_ROW_HEIGHT: f64 = 22.0;
const PANEL_PADDING_X: f64 = 16.0;
const PANEL_PADDING_Y: f64 = 12.0;
const PANEL_COLUMN_GAP: f64 = 16.0;
const PANEL_SECTION_GAP: f64 = 8.0;
const PANEL_MARGIN: f64 = 12.0;
const PANEL_INFO_OFFSET: f64 = 12.0;
const PANEL_ANCHOR_GAP: f64 = 12.0;
const PANEL_POINTER_OFFSET: f64 = 16.0;

mod focus;
mod interaction;
mod layout;

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
