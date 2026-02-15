use super::super::base::InputState;
use super::types::{ContextMenuKind, ContextMenuState, MenuCommand};
use crate::draw::ShapeId;

impl InputState {
    /// Closes the currently open context menu.
    pub fn close_context_menu(&mut self) {
        if let Some(layout) = self.context_menu_layout.take() {
            self.mark_context_menu_region(layout);
        }
        self.context_menu_state = ContextMenuState::Hidden;
        self.context_menu_page_target = None;
        self.pending_menu_hover_recalc = false;
        self.needs_redraw = true;
    }

    /// Returns true if a context menu is currently visible.
    pub fn is_context_menu_open(&self) -> bool {
        matches!(self.context_menu_state, ContextMenuState::Open { .. })
    }

    /// Opens a context menu at the given anchor for the provided kind.
    pub fn open_context_menu(
        &mut self,
        anchor: (i32, i32),
        shape_ids: Vec<ShapeId>,
        kind: ContextMenuKind,
        hovered_shape_id: Option<ShapeId>,
    ) {
        if !self.context_menu_enabled {
            return;
        }
        self.close_properties_panel();
        self.context_menu_page_target = None;
        if let Some(layout) = self.context_menu_layout.take() {
            self.mark_context_menu_region(layout);
        }
        self.context_menu_state = ContextMenuState::Open {
            anchor,
            shape_ids,
            kind,
            hover_index: None,
            keyboard_focus: None,
            hovered_shape_id,
        };
        self.pending_menu_hover_recalc = true;
    }

    pub fn open_page_context_menu(
        &mut self,
        anchor: (i32, i32),
        board_index: usize,
        page_index: usize,
    ) {
        if !self.context_menu_enabled {
            return;
        }
        self.open_context_menu(anchor, Vec::new(), ContextMenuKind::Page, None);
        self.context_menu_page_target = Some(super::super::board_picker::BoardPickerPageTarget {
            board_index,
            page_index,
        });
        self.pending_menu_hover_recalc = false;
        self.set_context_menu_focus(None);
        self.focus_first_context_menu_entry();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub fn toggle_context_menu_via_keyboard(&mut self) {
        if !self.context_menu_enabled {
            return;
        }
        if self.is_context_menu_open() {
            self.close_context_menu();
            return;
        }

        let selection = self.selected_shape_ids().to_vec();
        if selection.is_empty() {
            let anchor = self.keyboard_canvas_menu_anchor();
            self.update_pointer_position_synthetic(anchor.0, anchor.1);
            self.open_context_menu(anchor, Vec::new(), ContextMenuKind::Canvas, None);
            self.pending_menu_hover_recalc = false;
            self.set_context_menu_focus(None);
            self.focus_first_context_menu_entry();
        } else {
            let focus_edit = selection.len() == 1
                && self
                    .boards
                    .active_frame()
                    .shape(selection[0])
                    .map(|shape| {
                        matches!(
                            shape.shape,
                            crate::draw::Shape::Text { .. } | crate::draw::Shape::StickyNote { .. }
                        )
                    })
                    .unwrap_or(false);
            let anchor = self.keyboard_shape_menu_anchor(&selection);
            self.update_pointer_position_synthetic(anchor.0, anchor.1);
            self.open_context_menu(anchor, selection, ContextMenuKind::Shape, None);
            self.pending_menu_hover_recalc = false;
            if !focus_edit || !self.focus_context_menu_command(MenuCommand::EditText) {
                self.focus_first_context_menu_entry();
            }
        }
        if self.is_context_menu_open() {
            self.pending_onboarding_usage.used_context_menu_keyboard = true;
        }
        self.needs_redraw = true;
    }

    fn keyboard_canvas_menu_anchor(&self) -> (i32, i32) {
        let padding = 16;
        let x = padding;
        let y = match &self.context_menu_layout {
            Some(layout) => (layout.origin_y + layout.height + padding as f64) as i32,
            None => padding,
        };
        (x, y)
    }

    fn keyboard_shape_menu_anchor(&self, ids: &[ShapeId]) -> (i32, i32) {
        if let Some(bounds) = self.selection_bounding_box(ids) {
            (bounds.x + bounds.width / 2, bounds.y + bounds.height / 2)
        } else {
            let (px, py) = self.last_pointer_position;
            (px, py)
        }
    }

    pub fn set_context_menu_enabled(&mut self, enabled: bool) {
        self.context_menu_enabled = enabled;
        if !enabled && self.is_context_menu_open() {
            self.close_context_menu();
        }
    }

    pub fn context_menu_enabled(&self) -> bool {
        self.context_menu_enabled
    }
}
