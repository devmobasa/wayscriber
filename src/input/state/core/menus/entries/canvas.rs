use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, MenuCommand};
use crate::config::Action;
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD};

impl InputState {
    pub(super) fn canvas_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let mut entries = Vec::new();
        let frame = self.boards.active_frame();
        let mut has_locked = false;
        let mut has_unlocked = false;
        for shape in &frame.shapes {
            if shape.locked {
                has_locked = true;
            } else {
                has_unlocked = true;
            }
            if has_locked && has_unlocked {
                break;
            }
        }
        let clear_label = if has_locked {
            "Clear Unlocked"
        } else {
            "Clear All"
        };
        let clear_disabled = !has_unlocked;
        entries.push(ContextMenuEntry::new(
            clear_label,
            self.shortcut_for_action(Action::ClearCanvas),
            false,
            clear_disabled,
            Some(MenuCommand::ClearAll),
        ));
        entries.push(ContextMenuEntry::new(
            "Toggle Highlight (tool + click)",
            self.shortcut_for_action(Action::ToggleHighlightTool),
            false,
            false,
            Some(MenuCommand::ToggleHighlightTool),
        ));
        entries.push(ContextMenuEntry::new(
            "Boards",
            None::<String>,
            true,
            false,
            Some(MenuCommand::OpenBoardsMenu),
        ));
        entries.push(ContextMenuEntry::new(
            "Pages",
            None::<String>,
            true,
            false,
            Some(MenuCommand::OpenPagesMenu),
        ));

        // Quick board switching options
        let current_id = self.board_id();
        let has_whiteboard = self.boards.has_board(BOARD_ID_WHITEBOARD);
        let has_blackboard = self.boards.has_board(BOARD_ID_BLACKBOARD);

        if current_id == BOARD_ID_TRANSPARENT {
            if has_whiteboard {
                entries.push(ContextMenuEntry::new(
                    "Switch to Whiteboard",
                    self.shortcut_for_action(Action::ToggleWhiteboard),
                    false,
                    false,
                    Some(MenuCommand::SwitchToWhiteboard),
                ));
            }
            if has_blackboard {
                entries.push(ContextMenuEntry::new(
                    "Switch to Blackboard",
                    self.shortcut_for_action(Action::ToggleBlackboard),
                    false,
                    false,
                    Some(MenuCommand::SwitchToBlackboard),
                ));
            }
        } else {
            entries.push(ContextMenuEntry::new(
                "Return to Transparent",
                self.shortcut_for_action(Action::ReturnToTransparent),
                false,
                false,
                Some(MenuCommand::ReturnToTransparent),
            ));
            if current_id == BOARD_ID_WHITEBOARD && has_blackboard {
                entries.push(ContextMenuEntry::new(
                    "Switch to Blackboard",
                    self.shortcut_for_action(Action::ToggleBlackboard),
                    false,
                    false,
                    Some(MenuCommand::SwitchToBlackboard),
                ));
            } else if current_id == BOARD_ID_BLACKBOARD && has_whiteboard {
                entries.push(ContextMenuEntry::new(
                    "Switch to Whiteboard",
                    self.shortcut_for_action(Action::ToggleWhiteboard),
                    false,
                    false,
                    Some(MenuCommand::SwitchToWhiteboard),
                ));
            }
        }

        entries.push(ContextMenuEntry::new(
            "Command Palette",
            self.shortcut_for_action(Action::ToggleCommandPalette),
            false,
            false,
            Some(MenuCommand::OpenCommandPalette),
        ));
        entries.push(ContextMenuEntry::new(
            "Radial Menu",
            self.shortcut_for_action(Action::ToggleRadialMenu),
            false,
            false,
            Some(MenuCommand::OpenRadialMenu),
        ));
        entries.push(ContextMenuEntry::new(
            "Help",
            self.shortcut_for_action(Action::ToggleHelp),
            false,
            false,
            Some(MenuCommand::ToggleHelp),
        ));
        entries.push(ContextMenuEntry::new(
            "Open Config File",
            None::<String>,
            false,
            false,
            Some(MenuCommand::OpenConfigFile),
        ));
        entries
    }
}
