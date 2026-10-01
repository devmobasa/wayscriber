use crate::input::events::MouseButton;

use super::super::{
    DrawingState, InputState,
    interaction::{CanvasPoint, PointerPoints, PointerRelease, ScreenPoint, route_pointer_release},
};

mod drawing;
mod panels;
mod selection;
mod text;

/// How a pointer interaction ends: by its own button release, or because
/// another action settled it while the button was still held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GestureEnd {
    Release,
    Interrupted,
}

impl InputState {
    /// Processes mouse button release events.
    ///
    /// # Arguments
    /// * `button` - Which mouse button was released
    /// * `x` - Mouse X coordinate at release
    /// * `y` - Mouse Y coordinate at release
    ///
    /// # Behavior
    /// When left button is released during drawing:
    /// - Finalizes the shape using start position and current position
    /// - Adds the completed shape to the frame
    /// - Returns to Idle state
    #[allow(dead_code)] // Retained for older callers that only have canvas coordinates.
    pub fn on_mouse_release(&mut self, button: MouseButton, x: i32, y: i32) {
        let (screen_x, screen_y) = self.screen_coords_for_canvas(x, y);
        self.on_mouse_release_with_canvas(button, screen_x, screen_y, x, y);
    }

    pub fn on_mouse_release_with_canvas(
        &mut self,
        button: MouseButton,
        screen_x: i32,
        screen_y: i32,
        canvas_x: i32,
        canvas_y: i32,
    ) {
        let measurer = crate::draw::TextMeasurer::default();
        let ui_engine = crate::ui_text::UiTextEngine::default();
        self.on_mouse_release_with_canvas_and_resources(
            crate::input::state::InputTextResources {
                measurer: &measurer,
                ui_engine: &ui_engine,
            },
            button,
            screen_x,
            screen_y,
            canvas_x,
            canvas_y,
        );
    }

    pub(crate) fn on_mouse_release_with_canvas_and_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        button: MouseButton,
        screen_x: i32,
        screen_y: i32,
        canvas_x: i32,
        canvas_y: i32,
    ) {
        let points = PointerPoints::new(
            ScreenPoint::new(screen_x, screen_y),
            CanvasPoint::new(canvas_x, canvas_y),
        );
        self.note_session_interaction_activity();
        let _ = route_pointer_release(self, resources, PointerRelease::new(button, points));
        self.note_session_interaction_activity();
    }

    pub(in crate::input::state) fn handle_color_picker_popup_release_at(
        &mut self,
        x: i32,
        y: i32,
    ) -> bool {
        panels::handle_color_picker_popup_release(self, x, y)
    }

    pub(in crate::input::state) fn handle_context_menu_release_at_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        x: i32,
        y: i32,
    ) -> bool {
        panels::handle_context_menu_release(self, resources, x, y)
    }

    pub(in crate::input::state) fn handle_board_picker_release_at_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        x: i32,
        y: i32,
    ) -> bool {
        panels::handle_board_picker_release(self, resources, x, y)
    }

    pub(in crate::input::state) fn handle_properties_panel_release_at_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        x: i32,
        y: i32,
    ) -> bool {
        panels::handle_properties_panel_release(self, measurer, x, y)
    }

    pub(in crate::input::state) fn finish_pointer_interaction_at_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        canvas_x: i32,
        canvas_y: i32,
    ) {
        self.end_pointer_interaction_with(measurer, (canvas_x, canvas_y), GestureEnd::Release);
    }

    /// Lands a held gesture that owns pre-gesture snapshots, recording it
    /// exactly as its release would, and returns whether one was running.
    ///
    /// Such a gesture records nothing until it ends and then commits one entry
    /// measured from the snapshots taken when it began. Any other edit that
    /// records history while it is held — a nudge, Delete, Undo — would leave
    /// the release committing from a stale "before", so that edit settles the
    /// gesture first. The later release then finds nothing left to finish.
    pub(in crate::input::state) fn settle_snapshot_gesture_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
    ) -> bool {
        if !matches!(
            self.state,
            DrawingState::MovingSelection { .. }
                | DrawingState::ResizingSelection { .. }
                | DrawingState::ResizingText { .. }
                | DrawingState::AdjustingSpotlightMagnification { .. }
                | DrawingState::BendingArrow { .. }
        ) {
            return false;
        }

        let canvas = self.pointer.canvas();
        self.end_pointer_interaction_with(measurer, canvas, GestureEnd::Interrupted);
        true
    }

    fn end_pointer_interaction_with(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        (canvas_x, canvas_y): (i32, i32),
        end: GestureEnd,
    ) {
        let state = std::mem::replace(&mut self.state, DrawingState::Idle);
        match state {
            DrawingState::MovingSelection {
                grab,
                snapshots,
                moved,
                ..
            } => {
                let release = matches!(end, GestureEnd::Release).then_some((canvas_x, canvas_y));
                selection::finish_moving_selection(self, measurer, grab, release, snapshots, moved);
            }
            DrawingState::Selecting {
                start_x,
                start_y,
                additive,
            } => {
                selection::finish_selection_drag(
                    self, measurer, start_x, start_y, canvas_x, canvas_y, additive,
                );
            }
            DrawingState::ResizingText {
                shape_id, snapshot, ..
            } => {
                selection::finish_text_resize(self, shape_id, snapshot);
            }
            DrawingState::ResizingSelection { snapshots, .. } => {
                selection::finish_selection_resize(self, measurer, snapshots.as_ref());
            }
            DrawingState::AdjustingSpotlightMagnification { shape_id, snapshot } => {
                selection::finish_spotlight_magnification(self, shape_id, snapshot);
            }
            DrawingState::BendingArrow { shape_id, snapshot } => {
                selection::finish_arrow_bend(self, shape_id, snapshot);
            }
            DrawingState::Drawing {
                tool,
                start_x,
                start_y,
                points,
                point_thicknesses,
            } => {
                drawing::finish_drawing(
                    self,
                    measurer,
                    tool,
                    drawing::DrawingRelease {
                        start: (start_x, start_y),
                        end: (canvas_x, canvas_y),
                        points,
                        point_thicknesses,
                    },
                );
            }
            DrawingState::PendingTextClick { x, y, shape_id, .. } => {
                text::handle_pending_text_click(self, measurer, x, y, shape_id);
            }
            other_state => {
                self.state = other_state;
            }
        }
        // An Alt+drag block move keeps us in TextInput; end that drag explicitly
        // since the Idle-based cleanup below only fires for interactions that
        // return to Idle. `end_pointer_drag` clears the block-drag flag itself.
        let ended_block_drag = self.text_editing.text_block_drag().is_some();
        if matches!(self.state, DrawingState::Idle) {
            self.end_pointer_drag();
            self.sync_current_settings_from_active_tool();
        } else if ended_block_drag {
            self.end_pointer_drag();
            self.needs_redraw = true;
        }
    }
}
