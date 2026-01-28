use log::warn;
use smithay_client_toolkit::seat::pointer::{CursorIcon, PointerData};
use wayland_client::{Connection, Proxy};

use super::*;
use crate::backend::wayland::toolbar::ToolbarCursorHint;
use crate::input::{
    BoardPickerCursorHint, ColorPickerCursorHint, CommandPaletteCursorHint, ContextMenuCursorHint,
    DrawingState, HelpOverlayCursorHint, SelectionHandle,
};

impl WaylandState {
    pub(super) fn update_pointer_cursor(&mut self, toolbar_hover: bool, conn: &Connection) {
        if self.toolbar_dragging() && self.pointer_lock_active() {
            self.hide_pointer_cursor();
            return;
        }

        if self.cursor_hidden {
            self.cursor_hidden = false;
            self.current_pointer_shape = None;
        }
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
        // Check color picker popup first (takes priority)
        if self.input_state.is_color_picker_popup_open() {
            let (mx, my) = self.current_mouse();
            if let Some(layout) = self.input_state.color_picker_popup_layout() {
                // When dragging on gradient, always show crosshair
                if self.input_state.color_picker_popup_is_dragging() {
                    return CursorIcon::Crosshair;
                }
                return match layout.cursor_hint_at(mx as f64, my as f64) {
                    ColorPickerCursorHint::Text => CursorIcon::Text,
                    ColorPickerCursorHint::Crosshair => CursorIcon::Crosshair,
                    ColorPickerCursorHint::Pointer => CursorIcon::Pointer,
                    ColorPickerCursorHint::Default => CursorIcon::Default,
                };
            }
        }

        // Check board picker popup
        if self.input_state.is_board_picker_open() {
            let (mx, my) = self.current_mouse();
            if let Some(hint) = self.input_state.board_picker_cursor_hint_at(mx, my) {
                return match hint {
                    BoardPickerCursorHint::Text => CursorIcon::Text,
                    BoardPickerCursorHint::Pointer => CursorIcon::Pointer,
                    BoardPickerCursorHint::Grab => CursorIcon::Grab,
                    BoardPickerCursorHint::Grabbing => CursorIcon::Grabbing,
                    BoardPickerCursorHint::Default => CursorIcon::Default,
                };
            }
        }

        // Check context menu
        if self.input_state.is_context_menu_open() {
            let (mx, my) = self.current_mouse();
            if let Some(hint) = self.input_state.context_menu_cursor_hint_at(mx, my) {
                return match hint {
                    ContextMenuCursorHint::Pointer => CursorIcon::Pointer,
                    ContextMenuCursorHint::Default => CursorIcon::Default,
                };
            }
        }

        // Check command palette
        if self.input_state.command_palette_open {
            let (mx, my) = self.current_mouse();
            let screen_width = self.surface.width();
            let screen_height = self.surface.height();
            if let Some(hint) =
                self.input_state
                    .command_palette_cursor_hint_at(mx, my, screen_width, screen_height)
            {
                return match hint {
                    CommandPaletteCursorHint::Text => CursorIcon::Text,
                    CommandPaletteCursorHint::Pointer => CursorIcon::Pointer,
                    CommandPaletteCursorHint::Default => CursorIcon::Default,
                };
            }
        }

        // Check help overlay
        if self.input_state.show_help {
            let (mx, my) = self.current_mouse();
            let screen_width = self.surface.width();
            let screen_height = self.surface.height();
            if let Some(hint) =
                self.input_state
                    .help_overlay_cursor_hint_at(mx, my, screen_width, screen_height)
            {
                return match hint {
                    HelpOverlayCursorHint::Text => CursorIcon::Text,
                    HelpOverlayCursorHint::Default => CursorIcon::Default,
                };
            }
        }

        if self.toolbar_dragging() {
            return CursorIcon::Grabbing;
        }

        // Inline toolbar cursor hints (when using inline mode)
        if self.inline_toolbars_active()
            && self.pointer_over_toolbar()
            && let Some(hint) = self.inline_toolbar_cursor_hint()
        {
            return match hint {
                ToolbarCursorHint::Pointer => CursorIcon::Pointer,
                ToolbarCursorHint::Grab => CursorIcon::Grab,
                ToolbarCursorHint::Crosshair => CursorIcon::Crosshair,
                ToolbarCursorHint::Default => CursorIcon::Default,
            };
        }

        // Layer-shell toolbar cursor hints (sliders get grab, buttons get pointer, etc.)
        if toolbar_hover {
            if let Some(hint) = self.toolbar.cursor_hint() {
                return match hint {
                    ToolbarCursorHint::Pointer => CursorIcon::Pointer,
                    ToolbarCursorHint::Grab => CursorIcon::Grab,
                    ToolbarCursorHint::Crosshair => CursorIcon::Crosshair,
                    ToolbarCursorHint::Default => CursorIcon::Default,
                };
            }
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

    pub(in crate::backend::wayland) fn hide_pointer_cursor(&mut self) {
        if self.cursor_hidden {
            return;
        }
        let Some(pointer) = self.current_pointer() else {
            return;
        };
        let serial = pointer
            .data::<PointerData>()
            .and_then(|data| data.latest_button_serial().or(data.latest_enter_serial()));
        let Some(serial) = serial else {
            return;
        };
        pointer.set_cursor(serial, None, 0, 0);
        self.cursor_hidden = true;
        self.current_pointer_shape = None;
    }
}
