use super::super::base::{InputState, PasteAnchor};
use super::types::{ContextMenuState, MenuCommand};
use crate::domain::Action;
use crate::draw::ShapeId;
use crate::input::state::{Toast, ToastPriority};
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD};
use log::info;

impl InputState {
    fn hovered_context_menu_shape(&self) -> Option<ShapeId> {
        if let ContextMenuState::Open {
            hovered_shape_id: Some(shape_id),
            ..
        } = &self.context_menu.state
        {
            Some(*shape_id)
        } else {
            None
        }
    }

    fn context_menu_paste_anchor(&self) -> PasteAnchor {
        if let ContextMenuState::Open { anchor, .. } = self.context_menu.state {
            let (x, y) = self.canvas_coords_for_screen(anchor.0, anchor.1);
            PasteAnchor::Pointer { x, y }
        } else {
            self.paste_anchor()
        }
    }

    fn board_picker_page_context_change_affects_panel(&self, board_index: usize) -> bool {
        self.board_picker_page_panel_board_index() == Some(board_index)
    }

    fn context_menu_board_target_index(&self) -> Option<usize> {
        let id = self.context_menu.board_target.as_deref()?;
        self.boards
            .board_states()
            .iter()
            .position(|board| board.spec.id == id)
    }

    /// Picker row actions act on the selected row, so select the target first.
    fn select_board_picker_row_for(&mut self, board_index: usize) -> bool {
        let Some(row) = self.board_picker_row_for_board(board_index) else {
            return false;
        };
        self.board_picker_set_selected(row);
        true
    }

    fn select_hovered_context_menu_shape_with(&mut self, measurer: &crate::draw::TextMeasurer) {
        if let Some(hovered_shape) = self.hovered_context_menu_shape() {
            self.set_selection_with(measurer, vec![hovered_shape]);
            self.close_context_menu();
        } else {
            self.close_context_menu();
        }
    }

    pub fn execute_menu_command(&mut self, command: MenuCommand) {
        let measurer = crate::draw::TextMeasurer::default();
        let ui_engine = crate::ui_text::UiTextEngine::default();
        self.execute_menu_command_with_resources(
            crate::input::state::InputTextResources {
                measurer: &measurer,
                ui_engine: &ui_engine,
            },
            command,
        );
    }

    pub(crate) fn execute_menu_command_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        command: MenuCommand,
    ) {
        match command {
            MenuCommand::Copy => {
                self.handle_action_with_resources(resources, Action::CopySelection);
                self.close_context_menu();
            }
            MenuCommand::Paste => {
                let anchor = self.context_menu_paste_anchor();
                self.request_clipboard_paste_at_anchor(anchor);
                info!("Requested clipboard paste");
                self.close_context_menu();
            }
            MenuCommand::Delete => {
                self.delete_selection_with(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::Duplicate => {
                self.duplicate_selection_with(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::SelectHoveredShape => {
                self.select_hovered_context_menu_shape_with(resources.measurer);
            }
            MenuCommand::MoveToFront => {
                self.move_selection_to_front_with(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::MoveToBack => {
                self.move_selection_to_back_with(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::Lock => {
                self.set_selection_locked_with(resources.measurer, true);
                self.close_context_menu();
            }
            MenuCommand::Unlock => {
                self.set_selection_locked_with(resources.measurer, false);
                self.close_context_menu();
            }
            MenuCommand::Properties => {
                if self.show_properties_panel_with(resources.measurer) {
                    self.close_context_menu();
                }
            }
            MenuCommand::EditText => {
                if self.edit_selected_text_with(resources.measurer) {
                    self.close_context_menu();
                }
            }
            MenuCommand::ClearAll => {
                self.clear_all();
                self.close_context_menu();
            }
            MenuCommand::ResetCanvasPosition => {
                self.reset_active_canvas_position();
                self.close_context_menu();
            }
            MenuCommand::OpenZoomMenu
            | MenuCommand::OpenPagesMenu
            | MenuCommand::OpenBoardsMenu
            | MenuCommand::OpenPageMoveMenu => {
                // Beside the parent row that holds it, keeping this menu open;
                // on its own when no such row is open.
                self.open_menu_for_command(&command);
            }
            MenuCommand::ZoomIn => {
                self.request_zoom_action(crate::input::ZoomAction::In);
                self.close_context_menu();
            }
            MenuCommand::ZoomOut => {
                self.request_zoom_action(crate::input::ZoomAction::Out);
                self.close_context_menu();
            }
            MenuCommand::ResetZoom => {
                self.request_zoom_action(crate::input::ZoomAction::Reset);
                self.close_context_menu();
            }
            MenuCommand::ToggleHighlightTool => {
                // Through the action, not the primitive: the action is what
                // queues the durable click-highlight change, and the other
                // chrome commands in this menu already route the same way.
                self.handle_action_with_resources(resources, Action::ToggleHighlightTool);
                self.close_context_menu();
            }
            MenuCommand::PagePrev => {
                self.page_prev_with_measurer(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::PageNext => {
                self.page_next_with_measurer(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::PageNew => {
                self.page_new_with_measurer(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::PageDuplicate => {
                self.page_duplicate_with_measurer(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::PageDelete => {
                if matches!(
                    self.page_delete_with_measurer(resources.measurer),
                    crate::draw::PageDeleteOutcome::Cleared
                ) {
                    self.push_toast(
                        ToastPriority::Info,
                        "ui",
                        Toast::info("Cleared the last page."),
                    );
                }
                self.close_context_menu();
            }
            MenuCommand::PageRename => {
                if let Some(target) = self.context_menu.page_target {
                    self.board_picker_start_page_rename(target.board_index, target.page_index);
                }
                self.close_context_menu();
            }
            MenuCommand::PageDuplicateFromContext => {
                if let Some(target) = self.context_menu.page_target {
                    let affects_panel =
                        self.board_picker_page_context_change_affects_panel(target.board_index);
                    if self.duplicate_page_in_board_with_measurer(
                        resources.measurer,
                        target.board_index,
                        target.page_index,
                    ) && affects_panel
                    {
                        self.board_picker_reconcile_page_nav_after_page_change();
                    }
                }
                self.close_context_menu();
            }
            MenuCommand::PageDeleteFromContext => {
                if let Some(target) = self.context_menu.page_target {
                    let affects_panel =
                        self.board_picker_page_context_change_affects_panel(target.board_index);
                    let outcome = self.delete_page_in_board_with_measurer(
                        resources.measurer,
                        target.board_index,
                        target.page_index,
                    );
                    if !matches!(outcome, crate::draw::PageDeleteOutcome::Pending) && affects_panel
                    {
                        self.board_picker_reconcile_page_nav_after_page_change();
                    }
                }
                self.close_context_menu();
            }
            MenuCommand::PageMoveToBoard { id } => {
                if let Some(target) = self.context_menu.page_target {
                    let source_board = target.board_index;
                    let page_index = target.page_index;
                    let affects_panel =
                        self.board_picker_page_context_change_affects_panel(source_board);
                    if let Some(target_index) = self
                        .boards
                        .board_states()
                        .iter()
                        .position(|board| board.spec.id == id)
                    {
                        let moved = self.move_page_between_boards_with_activation_with_measurer(
                            resources.measurer,
                            source_board,
                            page_index,
                            target_index,
                            false,
                            true,
                        );
                        if moved && affects_panel {
                            self.board_picker_reconcile_page_nav_after_page_change();
                        }
                    }
                }
                self.close_context_menu();
            }
            MenuCommand::SwitchToPage(index) => {
                self.switch_to_page_with_measurer(resources.measurer, index);
                self.close_context_menu();
            }
            MenuCommand::OpenBoardPicker => {
                self.close_context_menu();
                // A menu opened from the picker itself leaves it open.
                if !self.is_board_picker_open() {
                    self.open_board_picker_with_measurer(resources.measurer);
                }
            }
            MenuCommand::BoardPrev => {
                self.switch_board_prev_with_measurer(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::BoardNext => {
                self.switch_board_next_with_measurer(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::BoardNew => {
                self.create_board_with_measurer(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::BoardDuplicate => {
                self.duplicate_board_with_measurer(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::BoardDelete => {
                self.delete_active_board_with_measurer(resources.measurer);
                self.close_context_menu();
            }
            MenuCommand::BoardEditPaper => {
                self.close_context_menu();
                let active = self.boards.active_index();
                self.board_picker_edit_board_paper_with_measurer(resources.measurer, active);
            }
            MenuCommand::BoardEditPaperFromContext => {
                let target = self.context_menu_board_target_index();
                self.close_context_menu();
                if let Some(board_index) = target {
                    self.board_picker_edit_board_paper_with_measurer(
                        resources.measurer,
                        board_index,
                    );
                }
            }
            MenuCommand::BoardRenameFromContext => {
                let target = self.context_menu_board_target_index();
                self.close_context_menu();
                if let Some(board_index) = target
                    && self.select_board_picker_row_for(board_index)
                {
                    self.board_picker_rename_selected_with_measurer(resources.measurer);
                }
            }
            MenuCommand::BoardTogglePinFromContext => {
                let target = self.context_menu_board_target_index();
                self.close_context_menu();
                if let Some(board_index) = target
                    && self.select_board_picker_row_for(board_index)
                {
                    self.board_picker_toggle_pin_selected();
                }
            }
            MenuCommand::SwitchToBoard { id } => {
                self.switch_board_with_measurer(resources.measurer, &id);
                self.close_context_menu();
            }
            MenuCommand::SwitchToWhiteboard => {
                self.switch_board_with_measurer(resources.measurer, BOARD_ID_WHITEBOARD);
                self.close_context_menu();
            }
            MenuCommand::SwitchToBlackboard => {
                self.switch_board_with_measurer(resources.measurer, BOARD_ID_BLACKBOARD);
                self.close_context_menu();
            }
            MenuCommand::ReturnToTransparent => {
                self.switch_board_with_measurer(resources.measurer, BOARD_ID_TRANSPARENT);
                self.close_context_menu();
            }
            MenuCommand::OpenRadialMenu => {
                self.close_context_menu();
                self.handle_action_with_resources(resources, Action::ToggleRadialMenu);
            }
            MenuCommand::ToggleHelp => {
                self.toggle_help_overlay();
                self.close_context_menu();
            }
            MenuCommand::ShowToolbar => {
                self.close_context_menu();
                self.handle_action_with_resources(resources, Action::ToggleToolbar);
            }
            MenuCommand::ShowStatusBar => {
                self.close_context_menu();
                self.handle_action_with_resources(resources, Action::ToggleStatusBar);
            }
            MenuCommand::OpenCommandPalette => {
                self.close_context_menu();
                self.toggle_command_palette();
            }
            MenuCommand::OpenConfigFile => {
                self.open_config_file_default();
                self.close_context_menu();
            }
        }
    }
}
