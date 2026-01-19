use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, MenuCommand};
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
                Some("Alt+Click"),
                false,
                false,
                Some(MenuCommand::SelectHoveredShape),
            ));
        }

        entries.push(ContextMenuEntry::new(
            "Delete",
            Some("Del"),
            false,
            all_locked,
            Some(MenuCommand::Delete),
        ));
        entries.push(ContextMenuEntry::new(
            "Duplicate",
            Some("Ctrl+D"),
            false,
            false,
            Some(MenuCommand::Duplicate),
        ));
        entries.push(ContextMenuEntry::new(
            "Move to Front",
            Some("]"),
            false,
            false,
            Some(MenuCommand::MoveToFront),
        ));
        entries.push(ContextMenuEntry::new(
            "Move to Back",
            Some("["),
            false,
            false,
            Some(MenuCommand::MoveToBack),
        ));
        entries.push(ContextMenuEntry::new(
            if locked { "Unlock" } else { "Lock" },
            Some("Ctrl+L"),
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
            Some("Ctrl+Enter"),
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
                        Some("Enter"),
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
