use super::{RadialMenuLayout, RadialMenuState};
use crate::config::RadialMenuMouseBinding;

/// Lifecycle, layout, and configured pointer trigger for the radial menu.
#[derive(Debug)]
pub struct RadialMenuPanel {
    pub state: RadialMenuState,
    pub layout: Option<RadialMenuLayout>,
    pub mouse_binding: RadialMenuMouseBinding,
}

impl Default for RadialMenuPanel {
    fn default() -> Self {
        Self {
            state: RadialMenuState::Hidden,
            layout: None,
            mouse_binding: RadialMenuMouseBinding::Middle,
        }
    }
}
