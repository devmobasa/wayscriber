use super::super::base::{InputState, UiToastKind};
use super::types::{ContextMenuKind, ContextMenuState, MenuCommand};
use crate::draw::ShapeId;
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD};

impl InputState {
    fn hovered_context_menu_shape(&self) -> Option<ShapeId> {
        if let ContextMenuState::Open {
            hovered_shape_id: Some(shape_id),
            ..
        } = &self.context_menu_state
        {
            Some(*shape_id)
        } else {
            None
        }
    }

    pub fn execute_menu_command(&mut self, command: MenuCommand) {
        match command {
            MenuCommand::Delete => {
                self.delete_selection();
                self.close_context_menu();
            }
            MenuCommand::Duplicate => {
                self.duplicate_selection();
                self.close_context_menu();
            }
            MenuCommand::SelectHoveredShape => {
                if let Some(hovered_shape) = self.hovered_context_menu_shape() {
                    let previous_ids = self.selected_shape_ids().to_vec();
                    let previous_bounds = {
                        let frame = self.boards.active_frame();
                        previous_ids
                            .iter()
                            .filter_map(|id| {
                                frame
                                    .shape(*id)
                                    .and_then(|shape| shape.shape.bounding_box())
                            })
                            .collect::<Vec<_>>()
                    };

                    self.set_selection(vec![hovered_shape]);

                    for bounds in previous_bounds {
                        self.mark_selection_dirty_region(Some(bounds));
                    }
                    let hovered_bounds = {
                        let frame = self.boards.active_frame();
                        frame
                            .shape(hovered_shape)
                            .and_then(|shape| shape.shape.bounding_box())
                    };
                    self.mark_selection_dirty_region(hovered_bounds);

                    self.close_context_menu();
                } else {
                    self.close_context_menu();
                }
            }
            MenuCommand::MoveToFront => {
                self.move_selection_to_front();
                self.close_context_menu();
            }
            MenuCommand::MoveToBack => {
                self.move_selection_to_back();
                self.close_context_menu();
            }
            MenuCommand::Lock => {
                self.set_selection_locked(true);
                self.close_context_menu();
            }
            MenuCommand::Unlock => {
                self.set_selection_locked(false);
                self.close_context_menu();
            }
            MenuCommand::Properties => {
                if self.show_properties_panel() {
                    self.close_context_menu();
                }
            }
            MenuCommand::EditText => {
                if self.edit_selected_text() {
                    self.close_context_menu();
                }
            }
            MenuCommand::ClearAll => {
                self.clear_all();
                self.close_context_menu();
            }
            MenuCommand::ToggleHighlightTool => {
                self.toggle_all_highlights();
                self.close_context_menu();
            }
            MenuCommand::OpenPagesMenu => {
                let anchor = if let Some(layout) = self.context_menu_layout {
                    (
                        (layout.origin_x + layout.width + 8.0).round() as i32,
                        layout.origin_y.round() as i32,
                    )
                } else if let ContextMenuState::Open { anchor, .. } = &self.context_menu_state {
                    *anchor
                } else {
                    self.last_pointer_position
                };
                self.open_context_menu(anchor, Vec::new(), ContextMenuKind::Pages, None);
                self.pending_menu_hover_recalc = false;
                self.set_context_menu_focus(None);
                self.focus_first_context_menu_entry();
                self.needs_redraw = true;
            }
            MenuCommand::PagePrev => {
                self.page_prev();
                self.close_context_menu();
            }
            MenuCommand::PageNext => {
                self.page_next();
                self.close_context_menu();
            }
            MenuCommand::PageNew => {
                self.page_new();
                self.close_context_menu();
            }
            MenuCommand::PageDuplicate => {
                self.page_duplicate();
                self.close_context_menu();
            }
            MenuCommand::PageDelete => {
                if matches!(self.page_delete(), crate::draw::PageDeleteOutcome::Cleared) {
                    self.set_ui_toast(UiToastKind::Info, "Cleared the last page.");
                }
                self.close_context_menu();
            }
            MenuCommand::SwitchToWhiteboard => {
                self.switch_board(BOARD_ID_WHITEBOARD);
                self.close_context_menu();
            }
            MenuCommand::SwitchToBlackboard => {
                self.switch_board(BOARD_ID_BLACKBOARD);
                self.close_context_menu();
            }
            MenuCommand::ReturnToTransparent => {
                self.switch_board(BOARD_ID_TRANSPARENT);
                self.close_context_menu();
            }
            MenuCommand::ToggleHelp => {
                self.toggle_help_overlay();
                self.close_context_menu();
            }
            MenuCommand::OpenConfigFile => {
                self.open_config_file_default();
                self.close_context_menu();
            }
        }
    }
}
