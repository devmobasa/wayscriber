use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, MenuCommand};
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
            Some("E"),
            false,
            clear_disabled,
            Some(MenuCommand::ClearAll),
        ));
        entries.push(ContextMenuEntry::new(
            "Toggle Highlight (tool + click)",
            Some("Ctrl+Alt+H"),
            false,
            false,
            Some(MenuCommand::ToggleHighlightTool),
        ));
        entries.push(ContextMenuEntry::new(
            "Pages",
            None::<String>,
            true,
            false,
            Some(MenuCommand::OpenPagesMenu),
        ));

        let current_id = self.board_id();
        let has_whiteboard = self.boards.has_board(BOARD_ID_WHITEBOARD);
        let has_blackboard = self.boards.has_board(BOARD_ID_BLACKBOARD);

        if current_id == BOARD_ID_TRANSPARENT {
            if has_whiteboard {
                entries.push(ContextMenuEntry::new(
                    "Switch to Whiteboard",
                    Some("Ctrl+W"),
                    false,
                    false,
                    Some(MenuCommand::SwitchToWhiteboard),
                ));
            }
            if has_blackboard {
                entries.push(ContextMenuEntry::new(
                    "Switch to Blackboard",
                    Some("Ctrl+B"),
                    false,
                    false,
                    Some(MenuCommand::SwitchToBlackboard),
                ));
            }
        } else {
            entries.push(ContextMenuEntry::new(
                "Return to Transparent",
                Some("Ctrl+Shift+T"),
                false,
                false,
                Some(MenuCommand::ReturnToTransparent),
            ));
            if current_id == BOARD_ID_WHITEBOARD && has_blackboard {
                entries.push(ContextMenuEntry::new(
                    "Switch to Blackboard",
                    Some("Ctrl+B"),
                    false,
                    false,
                    Some(MenuCommand::SwitchToBlackboard),
                ));
            } else if current_id == BOARD_ID_BLACKBOARD && has_whiteboard {
                entries.push(ContextMenuEntry::new(
                    "Switch to Whiteboard",
                    Some("Ctrl+W"),
                    false,
                    false,
                    Some(MenuCommand::SwitchToWhiteboard),
                ));
            }
        }

        entries.push(ContextMenuEntry::new(
            "Help",
            Some("F10"),
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
