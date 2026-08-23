use log::warn;
use smithay_client_toolkit::seat::pointer::{CursorIcon, PointerData};
use wayland_client::{Connection, Proxy};

use super::*;
use crate::backend::wayland::toolbar::ToolbarCursorHint;
use crate::input::{
    BoardPickerCursorHint, ColorPickerCursorHint, CommandPaletteCursorHint, ContextMenuCursorHint,
    DrawingState, HelpOverlayCursorHint, SelectionHandle,
};

/// What the pointer is over on the screen-modal surfaces (the eyedropper and
/// the region picker), resolved by [`WaylandState::screen_modal_cursor_context`]
/// so that [`screen_modal_cursor`] stays a pure decision table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ScreenModalCursorContext {
    window_snap_active: bool,
    review: bool,
    /// A device owns a Review move-drag right now.
    review_dragging: bool,
    over_action: bool,
    /// Inside the Review bar, whether or not over one of its controls.
    over_bar: bool,
    over_selection: bool,
}

/// Cursor for the screen-modal surfaces. Targeting keeps the crosshair; a
/// finished selection in Review swaps to ordinary chrome cursors, because
/// nothing is being aimed any more — the pointer now drags the rectangle or
/// presses a button.
fn screen_modal_cursor(context: ScreenModalCursorContext) -> CursorIcon {
    if context.window_snap_active {
        return CursorIcon::Pointer;
    }
    if !context.review {
        return CursorIcon::Crosshair;
    }
    if context.review_dragging {
        return CursorIcon::Grabbing;
    }
    if context.over_action {
        return CursorIcon::Pointer;
    }
    if context.over_bar {
        return CursorIcon::Default;
    }
    if context.over_selection {
        return CursorIcon::Grab;
    }
    CursorIcon::Default
}

impl WaylandState {
    pub(in crate::backend::wayland) fn update_pointer_cursor(
        &mut self,
        toolbar_hover: bool,
        conn: &Connection,
    ) {
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

    /// Refresh the cursor around a pointer press or release that touched the
    /// screen-modal surfaces. Both dispatches can change the pointer's role
    /// with no motion following: a press starts a Review move-drag, submits an
    /// action that closes the picker, or right-click-cancels it; a release
    /// ends a drag or moves targeting into Review. Scoped to the modal
    /// surfaces so ordinary drawing, toolbar and pointer-lock paths keep their
    /// existing behaviour.
    pub(super) fn refresh_screen_modal_cursor(
        &mut self,
        modal_before: bool,
        on_toolbar: bool,
        conn: &Connection,
    ) {
        if modal_before || self.input_state.screen_modal_is_active() {
            self.update_pointer_cursor(on_toolbar || self.pointer_over_toolbar(), conn);
        }
    }

    /// Gather what the screen-modal cursor decision needs. The hit tests each
    /// re-place the Review bar, so they run only once Review is actually
    /// engaged.
    fn screen_modal_cursor_context(&self) -> ScreenModalCursorContext {
        let window_snap_active = self.region_window_snap_active();
        let region_state = self.input_state.region_state();
        if window_snap_active || !region_state.is_review() {
            return ScreenModalCursorContext {
                window_snap_active,
                ..ScreenModalCursorContext::default()
            };
        }
        let (mouse_x, mouse_y) = self.current_mouse();
        let point = (f64::from(mouse_x), f64::from(mouse_y));
        ScreenModalCursorContext {
            window_snap_active,
            review: true,
            review_dragging: region_state.selection_owner().is_some(),
            over_action: self.region_review_action_at(point).is_some(),
            over_bar: self.region_review_bar_contains(point),
            over_selection: self.region_review_selection_contains(point),
        }
    }

    /// Computes the appropriate cursor icon based on current context.
    fn compute_cursor_icon(&mut self, toolbar_hover: bool) -> CursorIcon {
        if self.input_state.screen_modal_is_active() && !toolbar_hover {
            return screen_modal_cursor(self.screen_modal_cursor_context());
        }

        // Check color picker popup first (takes priority)
        if self.input_state.is_color_picker_popup_open() {
            let (mx, my) = self.current_mouse();
            if let Some(layout) = self.input_state.color_picker_popup_layout() {
                // When dragging on gradient, always show crosshair
                if self.input_state.color_picker_popup_is_dragging() {
                    return CursorIcon::Crosshair;
                }
                let recent_count = self.input_state.recent_colors().len();
                return match layout.cursor_hint_at(mx as f64, my as f64, recent_count) {
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
            if let Some(hint) = self.input_state.help_overlay_cursor_hint_at(mx, my) {
                return match hint {
                    HelpOverlayCursorHint::Text => CursorIcon::Text,
                    HelpOverlayCursorHint::Pointer => CursorIcon::Pointer,
                    HelpOverlayCursorHint::Default => CursorIcon::Default,
                };
            }
        }

        if self.toolbar_dragging() {
            return CursorIcon::Grabbing;
        }
        if self.board_panning_active() {
            return CursorIcon::Grabbing;
        }
        if self.board_pan_key_held() && self.can_start_board_pan() {
            return CursorIcon::Grab;
        }

        // Inline toolbar cursor hints (when using inline mode)
        if self.inline_toolbars_active()
            && self.pointer_over_toolbar()
            && let Some(hint) = self.inline_toolbar_cursor_hint()
        {
            return match hint {
                ToolbarCursorHint::Pointer => CursorIcon::Pointer,
                ToolbarCursorHint::Grab => CursorIcon::Grab,
                ToolbarCursorHint::Default => CursorIcon::Default,
            };
        }

        // Layer-shell toolbar cursor hints (sliders get grab, buttons get pointer, etc.)
        if toolbar_hover {
            if let Some(hint) = self.toolbar.cursor_hint() {
                return match hint {
                    ToolbarCursorHint::Pointer => CursorIcon::Pointer,
                    ToolbarCursorHint::Grab => CursorIcon::Grab,
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
            DrawingState::BuildingPolygon { .. } => {
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

        // Interactive chrome under an idle pointer: hand cursor over
        // actionable chips/buttons, neutral arrow over the rest of the
        // pill. Both surfaces render above the canvas, so they outrank
        // selection-handle hover in the pixels they occupy.
        if self.input_state.status_hud_hover.is_some() || self.input_state.zoom_chip_hover.is_some()
        {
            return CursorIcon::Pointer;
        }
        let (pointer_x, pointer_y) = self.input_state.pointer_position();
        if self.input_state.status_hud_contains(pointer_x, pointer_y)
            || self.input_state.zoom_chip_contains(pointer_x, pointer_y)
        {
            return CursorIcon::Default;
        }

        // Check if hovering over selection handles
        let (canvas_x, canvas_y) = self.input_state.canvas_pointer_position();
        if let Some(handle) = self.input_state.hit_selection_handle(canvas_x, canvas_y) {
            return match handle {
                SelectionHandle::TopLeft | SelectionHandle::BottomRight => CursorIcon::NwseResize,
                SelectionHandle::TopRight | SelectionHandle::BottomLeft => CursorIcon::NeswResize,
                SelectionHandle::Top | SelectionHandle::Bottom => CursorIcon::NsResize,
                SelectionHandle::Left | SelectionHandle::Right => CursorIcon::EwResize,
            };
        }

        // Check if hovering over text resize handle
        if self
            .input_state
            .hit_text_resize_handle(canvas_x, canvas_y)
            .is_some()
        {
            return CursorIcon::SeResize;
        }

        // Check if hovering over a selected shape (for move)
        if let Some(hit_id) = self.input_state.hit_test_at(canvas_x, canvas_y)
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

#[cfg(test)]
mod tests {
    use super::{ScreenModalCursorContext, screen_modal_cursor};
    use smithay_client_toolkit::seat::pointer::CursorIcon;

    fn review(context: ScreenModalCursorContext) -> ScreenModalCursorContext {
        ScreenModalCursorContext {
            review: true,
            ..context
        }
    }

    #[test]
    fn targeting_keeps_the_crosshair_and_window_mode_takes_the_hand() {
        assert_eq!(
            screen_modal_cursor(ScreenModalCursorContext::default()),
            CursorIcon::Crosshair,
            "armed, selecting, measuring and the eyedropper all aim"
        );
        assert_eq!(
            screen_modal_cursor(ScreenModalCursorContext {
                window_snap_active: true,
                ..ScreenModalCursorContext::default()
            }),
            CursorIcon::Pointer,
            "window mode picks a target rather than aiming"
        );
    }

    #[test]
    fn window_mode_outranks_review_state() {
        // Window mode replaces the selection wholesale, so any Review hover
        // flags left over from a previous rectangle must not leak through.
        assert_eq!(
            screen_modal_cursor(review(ScreenModalCursorContext {
                window_snap_active: true,
                over_selection: true,
                ..ScreenModalCursorContext::default()
            })),
            CursorIcon::Pointer
        );
    }

    #[test]
    fn review_resolves_dragging_then_controls_then_the_rectangle() {
        let cases = [
            (
                ScreenModalCursorContext {
                    review_dragging: true,
                    over_selection: true,
                    ..ScreenModalCursorContext::default()
                },
                CursorIcon::Grabbing,
                "a live move-drag outranks every hover",
            ),
            (
                ScreenModalCursorContext {
                    over_action: true,
                    over_bar: true,
                    ..ScreenModalCursorContext::default()
                },
                CursorIcon::Pointer,
                "an action button",
            ),
            (
                ScreenModalCursorContext {
                    over_bar: true,
                    ..ScreenModalCursorContext::default()
                },
                CursorIcon::Default,
                "a gap inside the bar stays modal-owned but inert",
            ),
            (
                ScreenModalCursorContext {
                    over_selection: true,
                    ..ScreenModalCursorContext::default()
                },
                CursorIcon::Grab,
                "the rectangle can still be dragged",
            ),
            (
                ScreenModalCursorContext::default(),
                CursorIcon::Default,
                "the scrim outside both",
            ),
        ];
        for (context, expected, what) in cases {
            assert_eq!(screen_modal_cursor(review(context)), expected, "{what}");
        }
    }

    #[test]
    fn a_button_that_overlaps_the_rectangle_still_reads_as_a_button() {
        // The bar can be clamped over a full-screen selection, so the two
        // hover flags are not mutually exclusive.
        assert_eq!(
            screen_modal_cursor(review(ScreenModalCursorContext {
                over_action: true,
                over_bar: true,
                over_selection: true,
                ..ScreenModalCursorContext::default()
            })),
            CursorIcon::Pointer
        );
    }
}
