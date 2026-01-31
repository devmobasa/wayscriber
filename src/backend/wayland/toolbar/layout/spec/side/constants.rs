use super::super::ToolbarLayoutSpec;

impl ToolbarLayoutSpec {
    pub(in crate::backend::wayland::toolbar) const SIDE_WIDTH: u32 = 260;

    pub(in crate::backend::wayland::toolbar) const SIDE_START_X: f64 = 16.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_TOP_PADDING: f64 = 12.0;

    // Three-row header layout
    pub(in crate::backend::wayland::toolbar) const SIDE_HEADER_ROW1_HEIGHT: f64 = 30.0; // Drag + pin/close
    pub(in crate::backend::wayland::toolbar) const SIDE_HEADER_ROW2_HEIGHT: f64 = 28.0; // Mode controls row
    pub(in crate::backend::wayland::toolbar) const SIDE_HEADER_ROW3_HEIGHT: f64 = 24.0; // Board row
    pub(in crate::backend::wayland::toolbar) const SIDE_HEADER_BOTTOM_GAP: f64 = 8.0;

    // Drag handle (compact size like before)
    pub(in crate::backend::wayland::toolbar) const SIDE_HEADER_DRAG_SIZE: f64 = 18.0;

    // Utility buttons (pin, more, close)
    pub(in crate::backend::wayland::toolbar) const SIDE_HEADER_BUTTON_SIZE: f64 = 22.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_HEADER_BUTTON_MARGIN_RIGHT: f64 = 12.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_HEADER_BUTTON_GAP: f64 = 6.0;

    // Segmented control height
    pub(in crate::backend::wayland::toolbar) const SIDE_SEGMENT_HEIGHT: f64 = 22.0;

    pub(in crate::backend::wayland::toolbar) const SIDE_MODE_ICONS_WIDTH: f64 = 72.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_MODE_LAYOUT_WIDTH: f64 = 120.0;

    // Board chip (inline style)
    pub(in crate::backend::wayland::toolbar) const SIDE_BOARD_CHIP_HEIGHT: f64 = 22.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_BOARD_CHEVRON_SIZE: f64 = 20.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_CONTENT_PADDING_X: f64 = 32.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_CARD_INSET: f64 = 6.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_PICKER_OFFSET_Y: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_PICKER_EXTRA_HEIGHT: f64 = 30.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_SLIDER_ROW_OFFSET: f64 = 26.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_NUDGE_SIZE: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_ACTION_BUTTON_HEIGHT_ICON: f64 = 32.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_ACTION_BUTTON_HEIGHT_TEXT: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_ACTION_BUTTON_GAP: f64 = 6.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_ACTION_CONTENT_GAP_TEXT: f64 = 5.0;

    pub(in crate::backend::wayland::toolbar) const SIDE_SECTION_GAP: f64 = 12.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_SECTION_TOGGLE_OFFSET_Y: f64 = 22.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_SECTION_LABEL_HEIGHT: f64 = 28.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_PICKER_INPUT_HEIGHT: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_SECTION_BOTTOM_PADDING: f64 = 8.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_SWATCH: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_SWATCH_GAP: f64 = 6.0;
    // Preview row constants (between gradient picker and swatches)
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_PREVIEW_SIZE: f64 = 28.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_PREVIEW_GAP_TOP: f64 = 10.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_PREVIEW_GAP_BOTTOM: f64 = 8.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_HEX_INPUT_HEIGHT: f64 = 20.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_HEX_INPUT_WIDTH: f64 = 70.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_COLOR_EXPAND_ICON_SIZE: f64 = 8.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_BOARD_COLOR_DOT_SIZE: f64 = 14.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_SECTION_LABEL_OFFSET_Y: f64 = 12.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_SECTION_LABEL_OFFSET_TALL: f64 = 14.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_FONT_BUTTON_HEIGHT: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_FONT_BUTTON_GAP: f64 = 8.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_NUDGE_ICON_SIZE: f64 = 14.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_SLIDER_VALUE_WIDTH: f64 = 40.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_TRACK_HEIGHT: f64 = 8.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_TRACK_KNOB_RADIUS: f64 = 7.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_DELAY_SLIDER_HEIGHT: f64 = 6.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_DELAY_SLIDER_KNOB_RADIUS: f64 = 6.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_DELAY_SLIDER_HIT_PADDING: f64 = 4.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_DELAY_SLIDER_UNDO_OFFSET_Y: f64 = 16.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_DELAY_SLIDER_REDO_OFFSET_Y: f64 = 32.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_ACTION_ICON_SIZE: f64 = 18.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_STEP_SLIDER_TOP_PADDING: f64 = 4.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_SLIDER_CARD_HEIGHT: f64 = 52.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_ERASER_MODE_CARD_HEIGHT: f64 = 44.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_TOGGLE_CARD_HEIGHT: f64 = 44.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_TOGGLE_CARD_HEIGHT_WITH_RESET: f64 =
        Self::SIDE_TOGGLE_CARD_HEIGHT + Self::SIDE_TOGGLE_HEIGHT + Self::SIDE_TOGGLE_GAP;
    pub(in crate::backend::wayland::toolbar) const SIDE_FONT_CARD_HEIGHT: f64 = 50.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_DELAY_SECTION_HEIGHT: f64 = 55.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_TOGGLE_HEIGHT: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_TOGGLE_GAP: f64 = 6.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_CUSTOM_SECTION_HEIGHT: f64 = 120.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_STEP_HEADER_HEIGHT: f64 = 20.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_PRESET_CARD_HEIGHT: f64 = 100.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_PRESET_SLOT_SIZE: f64 = 40.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_PRESET_SLOT_GAP: f64 = 8.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_PRESET_ROW_OFFSET_Y: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_PRESET_ACTION_GAP: f64 = 6.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_PRESET_ACTION_HEIGHT: f64 = 20.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_PRESET_ACTION_BUTTON_GAP: f64 = 4.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_FOOTER_PADDING: f64 = 10.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_SETTINGS_BUTTON_HEIGHT: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const SIDE_SETTINGS_BUTTON_GAP: f64 = 6.0;
}
