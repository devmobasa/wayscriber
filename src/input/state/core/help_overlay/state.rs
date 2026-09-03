use crate::input::state::{HelpOverlayClick, HelpOverlayPressSource};

/// Visibility, navigation, and pointer bookkeeping for the help overlay.
#[derive(Debug, Default)]
pub struct HelpOverlayState {
    pub visible: bool,
    pub page: usize,
    pub search: String,
    pub search_cursor: usize,
    pub scroll: f64,
    pub scroll_max: f64,
    pub(crate) pending_presses: Vec<(HelpOverlayPressSource, HelpOverlayClick)>,
    pub(crate) consume_only_presses: Vec<HelpOverlayPressSource>,
    pub quick_mode: bool,
}
