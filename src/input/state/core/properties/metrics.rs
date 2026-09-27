//! Sizes shared by the properties panel's layout, hit-testing, and painting,
//! so what the pointer hits is exactly what the renderer draws.

pub(crate) const TITLE_FONT: f64 = 15.0;
pub(crate) const SUBTITLE_FONT: f64 = 12.0;
pub(crate) const BODY_FONT: f64 = 13.0;
/// Readouts beside a control: a switch's "On", a stepper's value.
pub(crate) const VALUE_FONT: f64 = 12.5;
/// Style names under the drawn arrow previews.
pub(crate) const CAPTION_FONT: f64 = 10.5;
pub(crate) const TOOLTIP_FONT: f64 = 12.0;
/// The keyboard hint strip at the bottom.
pub(crate) const FOOTER_FONT: f64 = 11.0;

pub(crate) const MIN_WIDTH: f64 = 272.0;
pub(crate) const PADDING_X: f64 = 16.0;
pub(crate) const PADDING_TOP: f64 = 12.0;
pub(crate) const PADDING_BOTTOM: f64 = 8.0;
/// Title baseline to subtitle baseline.
pub(crate) const SUBTITLE_STEP: f64 = 18.0;
/// Below the header text, before the divider.
pub(crate) const HEADER_GAP: f64 = 12.0;
/// Divider to the first row.
pub(crate) const ROWS_GAP: f64 = 6.0;
pub(crate) const COLUMN_GAP: f64 = 16.0;
/// Between columns of rows, when a tall selection needs more than one. Twice
/// the room a row's hover fill reaches past its content, so neighboring
/// columns' fills meet without overlapping.
pub(crate) const COLUMN_SPACING: f64 = 2.0 * (PADDING_X - ROW_INSET);
/// How far a row's hover fill sits inside the panel edge.
pub(crate) const ROW_INSET: f64 = 4.0;

/// A row whose control sits beside its label.
pub(crate) const ROW_HEIGHT: f64 = 34.0;
/// A row whose control sits under its label: space above the label, the label
/// line, the gap before the control, and space after it.
pub(crate) const BLOCK_TOP: f64 = 8.0;
pub(crate) const BLOCK_LABEL_LINE: f64 = 16.0;
pub(crate) const BLOCK_GAP: f64 = 8.0;
pub(crate) const BLOCK_BOTTOM: f64 = 10.0;

pub(crate) const SWATCH_SIZE: f64 = 20.0;
pub(crate) const SWATCH_GAP: f64 = 7.0;
pub(crate) const SWATCH_LINE_GAP: f64 = 8.0;
/// Swatches plus the trailing "more colors" button on one line.
pub(crate) const SWATCH_ITEMS_PER_LINE: usize = 9;
/// Most quick colors the panel shows; the full picker covers the rest.
pub(crate) const MAX_SWATCHES: usize = 17;

pub(crate) const STEP_BUTTON_WIDTH: f64 = 24.0;
pub(crate) const STEPPER_HEIGHT: f64 = 24.0;
pub(crate) const STEPPER_MIN_VALUE_WIDTH: f64 = 52.0;
pub(crate) const STEPPER_VALUE_PADDING: f64 = 16.0;
pub(crate) const PREVIEW_WIDTH: f64 = 26.0;
pub(crate) const PREVIEW_HEIGHT: f64 = 12.0;
pub(crate) const PREVIEW_GAP: f64 = 8.0;

pub(crate) const SWITCH_WIDTH: f64 = 34.0;
pub(crate) const SWITCH_HEIGHT: f64 = 20.0;
/// Between a switch and the readout to its left.
pub(crate) const SWITCH_VALUE_GAP: f64 = 8.0;

pub(crate) const SEGMENT_HEIGHT: f64 = 22.0;
pub(crate) const SEGMENT_PAD: f64 = 2.0;
pub(crate) const SEGMENT_ICON_WIDTH: f64 = 14.0;
pub(crate) const SEGMENT_ICON_GAP: f64 = 4.0;
pub(crate) const SEGMENT_TEXT_PADDING: f64 = 8.0;

pub(crate) const STYLE_BUTTON_HEIGHT: f64 = 44.0;
pub(crate) const STYLE_BUTTON_MIN_WIDTH: f64 = 52.0;
pub(crate) const STYLE_BUTTON_GAP: f64 = 6.0;

pub(crate) const LOCK_SIZE: f64 = 26.0;
/// The lock button's inset from the panel's top-right corner.
pub(crate) const LOCK_INSET: f64 = 8.0;

/// The actions area: a divider, the ordering row, and the Duplicate/Delete
/// row under it.
pub(crate) const ACTIONS_TOP_GAP: f64 = 12.0;
pub(crate) const ACTION_BUTTON_HEIGHT: f64 = 28.0;
pub(crate) const ACTION_ROW_GAP: f64 = 8.0;
pub(crate) const ACTION_BUTTON_GAP: f64 = 6.0;
pub(crate) const ACTIONS_BOTTOM: f64 = 6.0;
pub(crate) const ACTIONS_HEIGHT: f64 =
    ACTIONS_TOP_GAP + ACTION_BUTTON_HEIGHT * 2.0 + ACTION_ROW_GAP + ACTIONS_BOTTOM;
/// The "Order" label's column, where both rows of buttons start.
pub(crate) const ACTIONS_LABEL_WIDTH: f64 = 60.0;

pub(crate) const FOOTER_HEIGHT: f64 = 30.0;
/// Height of the empty-state line when the selection has no properties.
pub(crate) const EMPTY_HEIGHT: f64 = 30.0;

pub(crate) const TOOLTIP_PADDING_X: f64 = 8.0;
pub(crate) const TOOLTIP_PADDING_Y: f64 = 5.0;
pub(crate) const TOOLTIP_GAP: f64 = 6.0;

/// The panel's text style at `size`.
pub(crate) fn text_style(
    size: f64,
    weight: cairo::FontWeight,
) -> crate::ui_text::UiTextStyle<'static> {
    crate::ui_text::UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight,
        size,
    }
}
