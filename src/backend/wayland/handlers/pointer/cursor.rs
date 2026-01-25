use log::warn;
use smithay_client_toolkit::seat::pointer::CursorIcon;
use wayland_client::Connection;

use super::*;
use crate::input::{DrawingState, SelectionHandle};

impl WaylandState {
    pub(super) fn update_pointer_cursor(&mut self, toolbar_hover: bool, conn: &Connection) {
        let icon = self.compute_cursor_icon(toolbar_hover);
        if let Some(pointer) = self.themed_pointer.as_ref()
            && self.current_pointer_shape != Some(icon)
        {
            if let Err(err) = pointer.set_cursor(conn, icon) {
                warn!("Failed to set cursor icon: {}", err);
            } else {
                self.current_pointer_shape = Some(icon);
            }
        }
    }

    /// Computes the appropriate cursor icon based on current context.
    fn compute_cursor_icon(&mut self, toolbar_hover: bool) -> CursorIcon {
        // Toolbar always gets default cursor
        if toolbar_hover {
            return CursorIcon::Default;
        }

        // Check drawing state for context
        match &self.input_state.state {
            // Text input mode - show text cursor
            DrawingState::TextInput { .. } => {
                return CursorIcon::Text;
            }
            // Dragging selection - show grabbing cursor
            DrawingState::MovingSelection { .. } => {
                return CursorIcon::Grabbing;
            }
            // Resizing text - show resize cursor
            DrawingState::ResizingText { .. } => {
                return CursorIcon::SeResize;
            }
            // Drawing - use crosshair
            DrawingState::Drawing { .. } => {
                return CursorIcon::Crosshair;
            }
            // Selecting (marquee) - use crosshair
            DrawingState::Selecting { .. } => {
                return CursorIcon::Crosshair;
            }
            // Pending text click - use default
            DrawingState::PendingTextClick { .. } => {
                return CursorIcon::Default;
            }
            // Resizing selection - show appropriate resize cursor
            DrawingState::ResizingSelection { handle, .. } => {
                return match handle {
                    SelectionHandle::TopLeft | SelectionHandle::BottomRight => {
                        CursorIcon::NwseResize
                    }
                    SelectionHandle::TopRight | SelectionHandle::BottomLeft => {
                        CursorIcon::NeswResize
                    }
                    SelectionHandle::Top | SelectionHandle::Bottom => CursorIcon::NsResize,
                    SelectionHandle::Left | SelectionHandle::Right => CursorIcon::EwResize,
                };
            }
            // Idle - check for hover contexts
            DrawingState::Idle => {}
        }

        // Check if hovering over selection handles
        let (mx, my) = self.current_mouse();
        if let Some(handle) = self.input_state.hit_selection_handle(mx, my) {
            return match handle {
                SelectionHandle::TopLeft | SelectionHandle::BottomRight => CursorIcon::NwseResize,
                SelectionHandle::TopRight | SelectionHandle::BottomLeft => CursorIcon::NeswResize,
                SelectionHandle::Top | SelectionHandle::Bottom => CursorIcon::NsResize,
                SelectionHandle::Left | SelectionHandle::Right => CursorIcon::EwResize,
            };
        }

        // Check if hovering over text resize handle
        if self.input_state.hit_text_resize_handle(mx, my).is_some() {
            return CursorIcon::SeResize;
        }

        // Check if hovering over a selected shape (for move)
        if let Some(hit_id) = self.input_state.hit_test_at(mx, my)
            && self
                .input_state
                .selected_shape_ids_set()
                .is_some_and(|set| set.contains(&hit_id))
        {
            return CursorIcon::Grab;
        }

        // Default: crosshair for drawing
        CursorIcon::Crosshair
    }
}
