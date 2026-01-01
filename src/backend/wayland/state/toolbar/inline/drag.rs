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

        match self.data.toolbar_move_drag {
            Some(MoveDrag {
                kind: MoveDragKind::Top,
                ..
            }) => Some(ToolbarIntent(ToolbarEvent::MoveTopToolbar { x, y })),
            Some(MoveDrag {
                kind: MoveDragKind::Side,
                ..
            }) => Some(ToolbarIntent(ToolbarEvent::MoveSideToolbar { x, y })),
            None => None,
        }
    }

    /// Returns true if we're currently in a toolbar move drag operation.
    pub(in crate::backend::wayland) fn is_move_dragging(&self) -> bool {
        self.data.toolbar_move_drag.is_some()
    }

    pub(in crate::backend::wayland) fn active_move_drag_kind(&self) -> Option<MoveDragKind> {
        self.data.active_drag_kind
    }
}
