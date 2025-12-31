use super::super::base::InputState;
use super::types::{ContextMenuEntry, ContextMenuKind, ContextMenuState, MenuCommand};
use crate::draw::{Shape, ShapeId};
use crate::input::board_mode::BoardMode;

impl InputState {
    /// Returns the entries to render for the currently open context menu.
    pub fn context_menu_entries(&self) -> Vec<ContextMenuEntry> {
        match &self.context_menu_state {
            ContextMenuState::Hidden => Vec::new(),
            ContextMenuState::Open {
                kind,
                shape_ids,
                hovered_shape_id,
                ..
            } => match kind {
                ContextMenuKind::Canvas => self.canvas_menu_entries(),
                ContextMenuKind::Shape => self.shape_menu_entries(shape_ids, *hovered_shape_id),
                ContextMenuKind::Pages => self.pages_menu_entries(),
            },
        }
    }

    fn canvas_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let mut entries = Vec::new();
        let frame = self.canvas_set.active_frame();
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

        match self.canvas_set.active_mode() {
            BoardMode::Transparent => {
                entries.push(ContextMenuEntry::new(
                    "Switch to Whiteboard",
                    Some("Ctrl+W"),
                    false,
                    false,
                    Some(MenuCommand::SwitchToWhiteboard),
                ));
                entries.push(ContextMenuEntry::new(
                    "Switch to Blackboard",
                    Some("Ctrl+B"),
                    false,
                    false,
                    Some(MenuCommand::SwitchToBlackboard),
                ));
            }
            BoardMode::Whiteboard => {
                entries.push(ContextMenuEntry::new(
                    "Return to Transparent",
                    Some("Ctrl+Shift+T"),
                    false,
                    false,
                    Some(MenuCommand::ReturnToTransparent),
                ));
                entries.push(ContextMenuEntry::new(
                    "Switch to Blackboard",
                    Some("Ctrl+B"),
                    false,
                    false,
                    Some(MenuCommand::SwitchToBlackboard),
                ));
            }
            BoardMode::Blackboard => {
                entries.push(ContextMenuEntry::new(
                    "Return to Transparent",
                    Some("Ctrl+Shift+T"),
                    false,
                    false,
                    Some(MenuCommand::ReturnToTransparent),
                ));
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

    fn pages_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let mode = self.canvas_set.active_mode();
        let page_count = self.canvas_set.page_count(mode);
        let page_index = self.canvas_set.active_page_index(mode);
        let can_prev = page_index > 0;
        let can_next = page_index + 1 < page_count;

        vec![
            ContextMenuEntry::new(
                format!("Page {}/{}", page_index + 1, page_count.max(1)),
                None::<String>,
                false,
                true,
                None,
            ),
            ContextMenuEntry::new(
                "Previous Page",
                None::<String>,
                false,
                !can_prev,
                Some(MenuCommand::PagePrev),
            ),
            ContextMenuEntry::new(
                "Next Page",
                None::<String>,
                false,
                !can_next,
                Some(MenuCommand::PageNext),
            ),
            ContextMenuEntry::new(
                "New Page",
                Some("Ctrl+Alt+N"),
                false,
                false,
                Some(MenuCommand::PageNew),
            ),
            ContextMenuEntry::new(
                "Duplicate Page",
                Some("Ctrl+Alt+D"),
                false,
                false,
                Some(MenuCommand::PageDuplicate),
            ),
            ContextMenuEntry::new(
                "Delete Page",
                Some("Ctrl+Alt+Delete"),
                false,
                false,
                Some(MenuCommand::PageDelete),
            ),
        ]
    }

    fn shape_menu_entries(
        &self,
        ids: &[ShapeId],
        hovered_shape_id: Option<ShapeId>,
    ) -> Vec<ContextMenuEntry> {
        let mut entries = Vec::new();
        let frame = self.canvas_set.active_frame();
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
