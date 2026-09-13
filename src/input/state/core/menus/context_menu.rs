use std::time::Instant;

use super::super::board_picker::BoardPickerPageTarget;
use super::{ContextMenuKind, ContextMenuLayout, ContextMenuState, SubmenuSide};
use crate::draw::ShapeId;

/// Where the pointer was at the previous hover update, and when. A move from
/// here toward an open submenu keeps it open while the pointer crosses other
/// rows, for as long as the sample is fresh.
#[derive(Debug, Clone, Copy)]
pub(in crate::input::state) struct AimSample {
    pub(in crate::input::state) point: (f64, f64),
    pub(in crate::input::state) at: Instant,
}

/// A hover change waiting for the pointer to rest. At `due` the row under the
/// pointer decides which submenu is open.
#[derive(Debug, Clone, Copy)]
pub(in crate::input::state) struct PendingHover {
    pub(in crate::input::state) due: Instant,
}

/// Lifecycle, target, and cached layout for the context menu.
#[derive(Debug)]
pub struct ContextMenuPanel {
    pub(in crate::input::state) state: ContextMenuState,
    pub(in crate::input::state) page_target: Option<BoardPickerPageTarget>,
    /// Board id for a picker row menu; ids survive row reordering.
    pub(in crate::input::state) board_target: Option<String>,
    pub(in crate::input::state) enabled: bool,
    pub(in crate::input::state) layout: Option<ContextMenuLayout>,
    pub(in crate::input::state) submenu_layout: Option<ContextMenuLayout>,
    /// Which side of the menu submenus open on for this layout.
    pub(in crate::input::state) submenu_side: SubmenuSide,
    pub(in crate::input::state) aim: Option<AimSample>,
    pub(in crate::input::state) pending_hover: Option<PendingHover>,
    /// A parent row collapsed by a click keeps its submenu shut while the
    /// pointer stays on it.
    pub(in crate::input::state) hover_open_suppressed: Option<usize>,
    /// An outside left press dismissed the menu but still owns its release.
    dismissal_release_pending: bool,
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

    pub fn submenu_layout(&self) -> Option<&ContextMenuLayout> {
        self.submenu_layout.as_ref()
    }

    pub fn submenu_side(&self) -> SubmenuSide {
        self.submenu_side
    }

    pub(crate) fn clear_layout(&mut self) {
        self.layout = None;
        self.submenu_layout = None;
    }

    pub(in crate::input::state) fn set_dismissal_release_pending(&mut self) {
        self.dismissal_release_pending = true;
    }

    pub(in crate::input::state) fn take_dismissal_release_pending(&mut self) -> bool {
        std::mem::take(&mut self.dismissal_release_pending)
    }

    /// Closes the menu and any submenu. The frame damage history repaints the
    /// area they covered.
    pub(crate) fn close(&mut self) {
        self.clear_layout();
        self.state = ContextMenuState::Hidden;
        self.page_target = None;
        self.board_target = None;
        self.reset_hover_timing();
    }

    pub(crate) fn open(
        &mut self,
        anchor: (i32, i32),
        shape_ids: Vec<ShapeId>,
        kind: ContextMenuKind,
        hovered_shape_id: Option<ShapeId>,
    ) {
        self.clear_layout();
        self.page_target = None;
        self.board_target = None;
        self.reset_hover_timing();
        self.state = ContextMenuState::Open {
            anchor,
            shape_ids,
            kind,
            hover_index: None,
            keyboard_focus: None,
            hovered_shape_id,
            submenu: None,
        };
    }

    fn reset_hover_timing(&mut self) {
        self.aim = None;
        self.pending_hover = None;
        self.hover_open_suppressed = None;
    }

    pub(crate) fn set_page_target(&mut self, board_index: usize, page_index: usize) {
        self.page_target = Some(BoardPickerPageTarget {
            board_index,
            page_index,
        });
    }

    pub(crate) fn set_board_target(&mut self, board_id: String) {
        self.board_target = Some(board_id);
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
            board_target: None,
            enabled: true,
            layout: None,
            submenu_layout: None,
            submenu_side: SubmenuSide::Right,
            aim: None,
            pending_hover: None,
            hover_open_suppressed: None,
            dismissal_release_pending: false,
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
        panel.set_board_target("whiteboard".to_string());

        panel.open((10, 20), Vec::new(), ContextMenuKind::Canvas, None);
        assert!(panel.is_open());
        assert!(panel.page_target.is_none());
        assert!(panel.board_target.is_none());

        panel.set_page_target(4, 5);
        panel.set_board_target("blackboard".to_string());
        panel.hover_open_suppressed = Some(1);
        panel.close();
        assert!(!panel.is_open());
        assert!(panel.page_target.is_none());
        assert!(panel.board_target.is_none());
        assert!(panel.hover_open_suppressed.is_none());
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
