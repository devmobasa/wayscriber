use super::{ColorPickerPopupAction, ColorPickerPopupLayout, ColorPickerPopupState};

/// Modal state, cached geometry, and press identity for the color picker popup.
#[derive(Debug, Default)]
pub struct ColorPickerPopupPanel {
    pub state: ColorPickerPopupState,
    pub layout: Option<ColorPickerPopupLayout>,
    pub(crate) generation: u64,
    pub(crate) pressed_action: Option<ColorPickerPopupAction>,
}
