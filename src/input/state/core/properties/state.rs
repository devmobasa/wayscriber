use super::{PropertiesPanelLayout, ShapePropertiesPanel};

/// Lifecycle, cached geometry, and deferred refresh state for the properties panel.
#[derive(Debug, Default)]
pub struct PropertiesPanelState {
    pub(super) panel: Option<ShapePropertiesPanel>,
    pub(crate) layout: Option<PropertiesPanelLayout>,
    pub(super) pending_hover_recalc: bool,
    pub(in crate::input::state::core) needs_refresh: bool,
}
