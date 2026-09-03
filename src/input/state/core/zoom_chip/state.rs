use crate::config::ZoomChipDisplay;
use crate::ui::{ZoomChipButtonKind, ZoomChipLayout, ZoomChipPress};

/// Display policy, cached geometry, and pointer interaction state for the zoom chip.
#[derive(Debug)]
pub struct ZoomChipState {
    pub display: ZoomChipDisplay,
    pub hover: Option<ZoomChipButtonKind>,
    pub layout: Option<ZoomChipLayout>,
    pub(in crate::input::state) press_pending: ZoomChipPress,
}

impl Default for ZoomChipState {
    fn default() -> Self {
        Self {
            display: ZoomChipDisplay::Always,
            hover: None,
            layout: None,
            press_pending: ZoomChipPress::None,
        }
    }
}
