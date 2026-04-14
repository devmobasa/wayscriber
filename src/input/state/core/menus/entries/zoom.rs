use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, MenuCommand};
use crate::config::Action;

impl InputState {
    pub(super) fn zoom_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let mut entries = Vec::new();
        let zoom_active = self.zoom_active();
        let zoom_percent = if zoom_active {
            (self.zoom_scale() * 100.0).round() as i32
        } else {
            100
        };

        entries.push(ContextMenuEntry::new(
            format!("Zoom {}%", zoom_percent),
            None::<String>,
            false,
            true,
            None,
        ));
        entries.push(ContextMenuEntry::new(
            "Zoom In",
            self.shortcut_for_action(Action::ZoomIn),
            false,
            false,
            Some(MenuCommand::ZoomIn),
        ));
        entries.push(ContextMenuEntry::new(
            "Zoom Out",
            self.shortcut_for_action(Action::ZoomOut),
            false,
            !zoom_active,
            Some(MenuCommand::ZoomOut),
        ));
        entries.push(ContextMenuEntry::new(
            "Reset Zoom",
            self.shortcut_for_action(Action::ResetZoom),
            false,
            !zoom_active,
            Some(MenuCommand::ResetZoom),
        ));

        entries
    }
}
