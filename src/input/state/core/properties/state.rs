use super::{PropertiesPanelLayout, ShapePropertiesPanel};

/// Lifecycle, cached geometry, and deferred refresh state for the properties panel.
#[derive(Debug, Default)]
pub struct PropertiesPanelState {
    pub(super) panel: Option<ShapePropertiesPanel>,
    pub(in crate::input::state) layout: Option<PropertiesPanelLayout>,
    pub(super) pending_hover_recalc: bool,
    pub(super) needs_refresh: bool,
}

impl PropertiesPanelState {
    pub fn panel(&self) -> Option<&ShapePropertiesPanel> {
        self.panel.as_ref()
    }

    pub fn layout(&self) -> Option<&PropertiesPanelLayout> {
        self.layout.as_ref()
    }

    pub fn is_open(&self) -> bool {
        self.panel.is_some()
    }

    pub(super) fn open(&mut self, panel: ShapePropertiesPanel) {
        self.panel = Some(panel);
        self.layout = None;
        self.pending_hover_recalc = true;
        self.needs_refresh = false;
    }

    pub(super) fn close(&mut self) -> bool {
        if self.panel.take().is_none() {
            return false;
        }
        self.layout = None;
        self.pending_hover_recalc = false;
        self.needs_refresh = false;
        true
    }

    pub(super) fn clear_layout(&mut self) {
        self.layout = None;
        self.pending_hover_recalc = false;
    }

    pub(in crate::input::state::core) fn mark_needs_refresh(&mut self) {
        self.needs_refresh = true;
    }

    pub(super) fn needs_refresh(&self) -> bool {
        self.needs_refresh
    }

    pub(super) fn begin_refresh(&mut self) {
        self.needs_refresh = false;
    }

    pub(super) fn request_hover_recalc(&mut self) {
        self.pending_hover_recalc = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> ShapePropertiesPanel {
        ShapePropertiesPanel {
            title: "Properties".into(),
            anchor: (10.0, 20.0),
            anchor_rect: None,
            lines: Vec::new(),
            entries: Vec::new(),
            hover_index: None,
            keyboard_focus: None,
            multiple_selection: false,
        }
    }

    #[test]
    fn opening_and_closing_reset_deferred_panel_work() {
        let mut state = PropertiesPanelState::default();
        state.open(panel());
        assert!(state.is_open());
        assert!(state.pending_hover_recalc);

        state.mark_needs_refresh();
        assert!(state.close());
        assert!(!state.is_open());
        assert!(!state.pending_hover_recalc);
        assert!(!state.needs_refresh);
        assert!(!state.close());
    }
}
