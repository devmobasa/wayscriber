use super::StatusHudRebuildInputs;
use crate::ui::{StatusHudLayout, StatusHudSegmentKind};

/// Cached geometry and pointer interaction state for the status HUD.
#[derive(Debug, Default)]
pub struct StatusHudState {
    pub hover: Option<StatusHudSegmentKind>,
    pub layout: Option<StatusHudLayout>,
    pub(super) rebuild_inputs: Option<StatusHudRebuildInputs>,
    pub(in crate::input::state) press_pending: bool,
}
