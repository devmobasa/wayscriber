use super::*;

impl WaylandState {
    /// Generate a drag intent from the active toolbar move drag state.
    /// This bypasses hit testing to allow dragging to continue when the mouse
    /// moves outside the original drag handle region.
    pub(in crate::backend::wayland) fn move_drag_intent(
        &self,
        x: f64,
        y: f64,
    ) -> Option<crate::backend::wayland::toolbar_intent::ToolbarIntent> {
        use crate::backend::wayland::toolbar_intent::ToolbarIntent;
        use crate::ui::toolbar::ToolbarEvent;

        self.toolbar_drag
            .kind()
            .map(|MoveDragKind::Top| ToolbarIntent(ToolbarEvent::MoveTopToolbar { x, y }))
    }
}
