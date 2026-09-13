use super::super::super::base::InputState;
use super::super::types::{ContextMenuEntry, MenuCommand};
use crate::domain::Action;

impl InputState {
    /// The current zoom level, shown on the parent row of the zoom submenu.
    pub(super) fn zoom_summary(&self) -> String {
        let zoom_percent = if self.zoom_active() {
            (self.zoom_scale() * 100.0).round() as i32
        } else {
            100
        };
        format!("{zoom_percent}%")
    }

    pub(super) fn zoom_menu_entries(&self, with_header: bool) -> Vec<ContextMenuEntry> {
        let mut entries = Vec::new();
        let zoom_active = self.zoom_active();

        if with_header {
            entries.push(ContextMenuEntry::new(
                format!("Zoom {}", self.zoom_summary()),
                None::<String>,
                true,
                None,
            ));
        }
        entries.push(ContextMenuEntry::new(
            "Zoom In",
            self.shortcut_for_action(Action::ZoomIn),
            false,
            Some(MenuCommand::ZoomIn),
        ));
        entries.push(ContextMenuEntry::new(
            "Zoom Out",
            self.shortcut_for_action(Action::ZoomOut),
            !zoom_active,
            Some(MenuCommand::ZoomOut),
        ));
        entries.push(ContextMenuEntry::new(
            "Reset Zoom",
            self.shortcut_for_action(Action::ResetZoom),
            !zoom_active,
            Some(MenuCommand::ResetZoom),
        ));

        entries
    }
}
