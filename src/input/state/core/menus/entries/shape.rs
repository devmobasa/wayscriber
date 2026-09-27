use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, ContextMenuKind, MenuCommand};
use crate::domain::Action;
use crate::draw::{Shape, ShapeId};

impl InputState {
    pub(super) fn shape_menu_entries(
        &self,
        ids: &[ShapeId],
        hovered_shape_id: Option<ShapeId>,
    ) -> Vec<ContextMenuEntry> {
        let mut entries = Vec::new();
        let frame = self.boards.active_frame();
        let locked = ids
            .iter()
            .any(|id| frame.shape(*id).map(|shape| shape.locked).unwrap_or(false));
        let all_locked = !ids.is_empty()
            && ids
                .iter()
                .all(|id| frame.shape(*id).map(|shape| shape.locked).unwrap_or(false));

        // What a right-click on a shape is usually for comes first: its own
        // editor, then its properties.
        if let [shape_id] = ids
            && let Some(drawn) = frame.shape(*shape_id)
        {
            let label = match drawn.shape {
                Shape::Text { .. } => Some("Edit Text"),
                Shape::StickyNote { .. } => Some("Edit Note"),
                _ => None,
            };
            if let Some(label) = label {
                entries.push(ContextMenuEntry::new(
                    label,
                    None::<String>, // Edit text not a configurable keybinding
                    drawn.locked,
                    Some(MenuCommand::EditText),
                ));
            }
        }
        entries.push(ContextMenuEntry::new(
            "Properties\u{2026}",
            self.shortcut_for_action(Action::ToggleSelectionProperties),
            false,
            Some(MenuCommand::Properties),
        ));
        // A right-click selects the clicked shape, so this row only narrows a
        // multi-selection to it; with the shape already the whole selection it
        // would do nothing.
        if let Some(hovered) = hovered_shape_id
            && ids != [hovered]
        {
            entries.push(ContextMenuEntry::new(
                "Select This Shape",
                Some("Alt+Click"), // Mouse action, not configurable
                false,
                Some(MenuCommand::SelectHoveredShape),
            ));
        }

        entries.push(
            ContextMenuEntry::new(
                "Copy",
                self.shortcut_for_action(Action::CopySelection),
                all_locked,
                Some(MenuCommand::Copy),
            )
            .with_separator(),
        );
        entries.push(self.paste_entry());
        entries.push(ContextMenuEntry::new(
            "Duplicate",
            self.shortcut_for_action(Action::DuplicateSelection),
            false,
            Some(MenuCommand::Duplicate),
        ));
        // The four stacking moves share one row; dimmed when the selection
        // can go neither way.
        let can_move = self.selection_can_step(true) || self.selection_can_step(false);
        entries.push(
            ContextMenuEntry::new("Arrange", None::<String>, !can_move, None)
                .with_submenu(ContextMenuKind::Arrange),
        );
        entries.push(ContextMenuEntry::new(
            if locked { "Unlock" } else { "Lock" },
            None::<String>, // Lock/unlock not a configurable keybinding
            false,
            Some(if locked {
                MenuCommand::Unlock
            } else {
                MenuCommand::Lock
            }),
        ));

        let view_group_start = entries.len();
        if self.boards.pan_enabled() && !self.board_is_transparent() {
            let reset_disabled = self.boards.active_frame().view_offset() == (0, 0);
            entries.push(ContextMenuEntry::new(
                "Reset Canvas Position",
                Some("Space+Drag"),
                reset_disabled,
                Some(MenuCommand::ResetCanvasPosition),
            ));
        }
        entries.push(
            ContextMenuEntry::new("Zoom", Some(self.zoom_summary()), false, None)
                .with_submenu(ContextMenuKind::Zoom),
        );
        entries[view_group_start].separator_before = true;
        entries.push(ContextMenuEntry::new(
            "Radial Menu",
            self.shortcut_for_action(Action::ToggleRadialMenu),
            false,
            Some(MenuCommand::OpenRadialMenu),
        ));

        self.push_chrome_recovery_entries(&mut entries);
        // Destructive last, apart from the rest, as Clear All is on the
        // canvas menu: never the row under the pointer when the menu opens.
        entries.push(
            ContextMenuEntry::new(
                "Delete",
                self.shortcut_for_action(Action::DeleteSelection),
                all_locked,
                Some(MenuCommand::Delete),
            )
            .with_separator(),
        );
        entries.push(self.exit_entry());

        entries
    }

    /// The stacking moves, dimmed when the selection is already as far that
    /// way as it goes. Standing alone, the menu starts with a header row.
    pub(super) fn arrange_menu_entries(&self, with_header: bool) -> Vec<ContextMenuEntry> {
        let can_raise = self.selection_can_step(true);
        let can_lower = self.selection_can_step(false);
        let mut entries = Vec::new();

        if with_header {
            entries.push(ContextMenuEntry::new("Arrange", None::<String>, true, None));
        }
        entries.push(ContextMenuEntry::new(
            "Move to Front",
            self.shortcut_for_action(Action::MoveSelectionToFront),
            !can_raise,
            Some(MenuCommand::MoveToFront),
        ));
        entries.push(ContextMenuEntry::new(
            "Move Forward",
            None::<String>,
            !can_raise,
            Some(MenuCommand::MoveForward),
        ));
        entries.push(ContextMenuEntry::new(
            "Move Backward",
            None::<String>,
            !can_lower,
            Some(MenuCommand::MoveBackward),
        ));
        entries.push(ContextMenuEntry::new(
            "Move to Back",
            self.shortcut_for_action(Action::MoveSelectionToBack),
            !can_lower,
            Some(MenuCommand::MoveToBack),
        ));

        entries
    }
}
