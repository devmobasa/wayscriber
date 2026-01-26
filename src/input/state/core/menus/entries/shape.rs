use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, MenuCommand};
use crate::config::Action;
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

        if hovered_shape_id.is_some() {
            entries.push(ContextMenuEntry::new(
                "Select This Shape",
                Some("Alt+Click"), // Mouse action, not configurable
                false,
                false,
                Some(MenuCommand::SelectHoveredShape),
            ));
        }

        entries.push(ContextMenuEntry::new(
            "Delete",
            self.shortcut_for_action(Action::DeleteSelection),
            false,
            all_locked,
            Some(MenuCommand::Delete),
        ));
        entries.push(ContextMenuEntry::new(
            "Duplicate",
            self.shortcut_for_action(Action::DuplicateSelection),
            false,
            false,
            Some(MenuCommand::Duplicate),
        ));
        entries.push(ContextMenuEntry::new(
            "Move to Front",
            self.shortcut_for_action(Action::MoveSelectionToFront),
            false,
            false,
            Some(MenuCommand::MoveToFront),
        ));
        entries.push(ContextMenuEntry::new(
            "Move to Back",
            self.shortcut_for_action(Action::MoveSelectionToBack),
            false,
            false,
            Some(MenuCommand::MoveToBack),
        ));
        entries.push(ContextMenuEntry::new(
            if locked { "Unlock" } else { "Lock" },
            None::<String>, // Lock/unlock not a configurable keybinding
            false,
            false,
            Some(if locked {
                MenuCommand::Unlock
            } else {
                MenuCommand::Lock
            }),
        ));
        entries.push(ContextMenuEntry::new(
            "Properties",
            self.shortcut_for_action(Action::ToggleSelectionProperties),
            false,
            false,
            Some(MenuCommand::Properties),
        ));

        if ids.len() == 1 {
            let shape_id = ids[0];
            if let Some(drawn) = frame.shape(shape_id) {
                let label = match drawn.shape {
                    Shape::Text { .. } => Some("Edit Text"),
                    Shape::StickyNote { .. } => Some("Edit Note"),
                    _ => None,
                };
                if let Some(label) = label {
                    entries.push(ContextMenuEntry::new(
                        label,
                        None::<String>, // Edit text not a configurable keybinding
                        false,
                        drawn.locked,
                        Some(MenuCommand::EditText),
                    ));
                }
            }
        }

        entries
    }
}
