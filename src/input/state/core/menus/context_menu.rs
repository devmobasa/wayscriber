use super::super::board_picker::BoardPickerPageTarget;
use super::{ContextMenuKind, ContextMenuLayout, ContextMenuState};
use crate::draw::ShapeId;

/// Lifecycle, target, and cached layout for the context menu.
#[derive(Debug)]
pub struct ContextMenuPanel {
    pub(in crate::input::state) state: ContextMenuState,
    pub(in crate::input::state) page_target: Option<BoardPickerPageTarget>,
    pub(in crate::input::state) enabled: bool,
    pub(in crate::input::state) layout: Option<ContextMenuLayout>,
}

impl ContextMenuPanel {
    pub fn state(&self) -> &ContextMenuState {
        &self.state
    }

    pub fn is_open(&self) -> bool {
        matches!(self.state, ContextMenuState::Open { .. })
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn layout(&self) -> Option<&ContextMenuLayout> {
        self.layout.as_ref()
    }

    pub(crate) fn clear_layout(&mut self) {
        self.layout = None;
    }

    pub(crate) fn close(&mut self) -> Option<ContextMenuLayout> {
        let layout = self.layout.take();
        self.state = ContextMenuState::Hidden;
        self.page_target = None;
        layout
    }

    pub(crate) fn open(
        &mut self,
        anchor: (i32, i32),
        shape_ids: Vec<ShapeId>,
        kind: ContextMenuKind,
        hovered_shape_id: Option<ShapeId>,
    ) -> Option<ContextMenuLayout> {
        let layout = self.layout.take();
        self.page_target = None;
        self.state = ContextMenuState::Open {
            anchor,
            shape_ids,
            kind,
            hover_index: None,
            keyboard_focus: None,
            hovered_shape_id,
        };
        layout
    }

    pub(crate) fn set_page_target(&mut self, board_index: usize, page_index: usize) {
        self.page_target = Some(BoardPickerPageTarget {
            board_index,
            page_index,
        });
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) -> bool {
        self.enabled = enabled;
        !enabled && self.is_open()
    }
}

impl Default for ContextMenuPanel {
    fn default() -> Self {
        Self {
            state: ContextMenuState::Hidden,
            page_target: None,
            enabled: true,
            layout: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_replaces_transient_target_and_closing_clears_it() {
        let mut panel = ContextMenuPanel::default();
        panel.set_page_target(2, 3);

        assert!(
            panel
                .open((10, 20), Vec::new(), ContextMenuKind::Canvas, None)
                .is_none()
        );
        assert!(panel.is_open());
        assert!(panel.page_target.is_none());

        panel.set_page_target(4, 5);
        assert!(panel.close().is_none());
        assert!(!panel.is_open());
        assert!(panel.page_target.is_none());
    }

    #[test]
    fn disabling_an_open_panel_requests_canonical_close() {
        let mut panel = ContextMenuPanel::default();
        panel.open((0, 0), Vec::new(), ContextMenuKind::Canvas, None);

        assert!(panel.set_enabled(false));
        assert!(!panel.is_enabled());
        assert!(panel.is_open(), "the root still owns close side effects");
    }
}
