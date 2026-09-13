use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, ContextMenuKind, MenuCommand};
use crate::domain::Action;
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
            "Paste",
            self.shortcut_for_action(Action::PasteSelection),
            false,
            Some(MenuCommand::Paste),
        ));
        entries.push(ContextMenuEntry::new(
            clear_label,
            self.shortcut_for_action(Action::ClearCanvas),
            clear_disabled,
            Some(MenuCommand::ClearAll),
        ));
        if self.boards.pan_enabled() && !self.board_is_transparent() {
            let reset_disabled = self.boards.active_frame().view_offset() == (0, 0);
            entries.push(ContextMenuEntry::new(
                "Reset Canvas Position",
                Some("Space+Drag"),
                reset_disabled,
                Some(MenuCommand::ResetCanvasPosition),
            ));
        }
        // Parent rows show their submenu's current state in the shortcut column.
        entries.push(
            ContextMenuEntry::new("Zoom", Some(self.zoom_summary()), false, None)
                .with_submenu(ContextMenuKind::Zoom),
        );
        entries.push(ContextMenuEntry::new(
            "Toggle Highlight (tool + click)",
            self.shortcut_for_action(Action::ToggleHighlightTool),
            false,
            Some(MenuCommand::ToggleHighlightTool),
        ));
        entries.push(
            ContextMenuEntry::new("Boards", Some(self.boards_summary()), false, None)
                .with_submenu(ContextMenuKind::Boards),
        );
        entries.push(
            ContextMenuEntry::new("Pages", Some(self.pages_summary()), false, None)
                .with_submenu(ContextMenuKind::Pages),
        );

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
                    Some(MenuCommand::SwitchToWhiteboard),
                ));
            }
            if has_blackboard {
                entries.push(ContextMenuEntry::new(
                    "Switch to Blackboard",
                    self.shortcut_for_action(Action::ToggleBlackboard),
                    false,
                    Some(MenuCommand::SwitchToBlackboard),
                ));
            }
        } else {
            entries.push(ContextMenuEntry::new(
                "Return to Transparent",
                self.shortcut_for_action(Action::ReturnToTransparent),
                false,
                Some(MenuCommand::ReturnToTransparent),
            ));
            if current_id == BOARD_ID_WHITEBOARD && has_blackboard {
                entries.push(ContextMenuEntry::new(
                    "Switch to Blackboard",
                    self.shortcut_for_action(Action::ToggleBlackboard),
                    false,
                    Some(MenuCommand::SwitchToBlackboard),
                ));
            } else if current_id == BOARD_ID_BLACKBOARD && has_whiteboard {
                entries.push(ContextMenuEntry::new(
                    "Switch to Whiteboard",
                    self.shortcut_for_action(Action::ToggleWhiteboard),
                    false,
                    Some(MenuCommand::SwitchToWhiteboard),
                ));
            }
        }

        entries.push(ContextMenuEntry::new(
            "Command Palette",
            self.shortcut_for_action(Action::ToggleCommandPalette),
            false,
            Some(MenuCommand::OpenCommandPalette),
        ));
        entries.push(ContextMenuEntry::new(
            "Radial Menu",
            self.shortcut_for_action(Action::ToggleRadialMenu),
            false,
            Some(MenuCommand::OpenRadialMenu),
        ));
        self.push_chrome_recovery_entries(&mut entries);
        entries.push(ContextMenuEntry::new(
            "Help",
            self.shortcut_for_action(Action::ToggleHelp),
            false,
            Some(MenuCommand::ToggleHelp),
        ));
        entries.push(ContextMenuEntry::new(
            "Open Config File",
            None::<String>,
            false,
            Some(MenuCommand::OpenConfigFile),
        ));
        entries
    }
}
