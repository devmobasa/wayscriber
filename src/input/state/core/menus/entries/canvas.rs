use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, ContextMenuKind, MenuCommand};
use crate::config::action_short_label;
use crate::domain::Action;
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD};

impl InputState {
    /// The right-click menu on empty canvas, in groups: history, paste and
    /// capture, view and boards, the other surfaces, the destructive Clear,
    /// and the way out last.
    pub(super) fn canvas_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let mut entries = Vec::new();

        let frame = self.boards.active_frame();
        entries.push(ContextMenuEntry::new(
            "Undo",
            self.shortcut_for_action(Action::Undo),
            frame.undo_stack_len() == 0,
            Some(MenuCommand::Undo),
        ));
        entries.push(ContextMenuEntry::new(
            "Redo",
            self.shortcut_for_action(Action::Redo),
            frame.redo_stack_len() == 0,
            Some(MenuCommand::Redo),
        ));

        entries.push(self.paste_entry().with_separator());
        entries.push(ContextMenuEntry::new(
            "Capture Region…",
            self.shortcut_for_action(Action::CaptureRegionInteractive),
            false,
            Some(MenuCommand::CaptureRegion),
        ));

        self.push_canvas_view_entries(&mut entries);

        entries.push(
            ContextMenuEntry::new(
                "Command Palette",
                self.shortcut_for_action(Action::ToggleCommandPalette),
                false,
                Some(MenuCommand::OpenCommandPalette),
            )
            .with_separator(),
        );
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

        entries.push(self.clear_entry().with_separator());
        entries.push(self.exit_entry());
        entries
    }

    /// Clear All, or Clear Unlocked when locked shapes survive it.
    fn clear_entry(&self) -> ContextMenuEntry {
        let frame = self.boards.active_frame();
        let has_locked = frame.shapes.iter().any(|shape| shape.locked);
        let has_unlocked = frame.shapes.iter().any(|shape| !shape.locked);
        let label = if has_locked {
            "Clear Unlocked"
        } else {
            "Clear All"
        };
        ContextMenuEntry::new(
            label,
            self.shortcut_for_action(Action::ClearCanvas),
            !has_unlocked,
            Some(MenuCommand::ClearAll),
        )
    }

    /// Zoom, canvas position, highlight, and board/page switching.
    fn push_canvas_view_entries(&self, entries: &mut Vec<ContextMenuEntry>) {
        // Parent rows show their submenu's current state in the shortcut column.
        entries.push(
            ContextMenuEntry::new("Zoom", Some(self.zoom_summary()), false, None)
                .with_submenu(ContextMenuKind::Zoom)
                .with_separator(),
        );
        if self.boards.pan_enabled() && !self.board_is_transparent() {
            let reset_disabled = self.boards.active_frame().view_offset() == (0, 0);
            entries.push(ContextMenuEntry::new(
                "Reset Canvas Position",
                Some("Space+Drag"),
                reset_disabled,
                Some(MenuCommand::ResetCanvasPosition),
            ));
        }
        // The action's own short label, as on the toolbar's Highlight button.
        entries.push(ContextMenuEntry::new(
            action_short_label(Action::ToggleHighlightTool),
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
        self.push_board_switch_entries(entries);
    }

    /// Quick switches between the transparent overlay and the paper boards.
    fn push_board_switch_entries(&self, entries: &mut Vec<ContextMenuEntry>) {
        let current_id = self.board_id();
        let has_whiteboard = self.boards.has_board(BOARD_ID_WHITEBOARD);
        let has_blackboard = self.boards.has_board(BOARD_ID_BLACKBOARD);
        let whiteboard = || {
            ContextMenuEntry::new(
                "Switch to Whiteboard",
                self.shortcut_for_action(Action::ToggleWhiteboard),
                false,
                Some(MenuCommand::SwitchToWhiteboard),
            )
        };
        let blackboard = || {
            ContextMenuEntry::new(
                "Switch to Blackboard",
                self.shortcut_for_action(Action::ToggleBlackboard),
                false,
                Some(MenuCommand::SwitchToBlackboard),
            )
        };

        if current_id == BOARD_ID_TRANSPARENT {
            if has_whiteboard {
                entries.push(whiteboard());
            }
            if has_blackboard {
                entries.push(blackboard());
            }
            return;
        }

        entries.push(ContextMenuEntry::new(
            "Return to Transparent",
            self.shortcut_for_action(Action::ReturnToTransparent),
            false,
            Some(MenuCommand::ReturnToTransparent),
        ));
        if current_id == BOARD_ID_WHITEBOARD && has_blackboard {
            entries.push(blackboard());
        } else if current_id == BOARD_ID_BLACKBOARD && has_whiteboard {
            entries.push(whiteboard());
        }
    }
}
