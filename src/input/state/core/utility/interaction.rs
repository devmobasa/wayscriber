use super::super::base::{DrawingState, InputState, PasteAnchor};
use crate::draw::DirtyRegionReport;
use crate::draw::{TextMeasurer, with_legacy_measurer};
use crate::util::Rect;
use std::time::Instant;

impl InputState {
    fn board_view_offset(&self) -> (f64, f64) {
        if self.board_is_transparent() || !self.boards.pan_enabled() {
            (0.0, 0.0)
        } else {
            let (x, y) = self.boards.active_frame().view_offset();
            (x as f64, y as f64)
        }
    }

    pub(crate) fn canvas_coords_for_screen(&self, screen_x: i32, screen_y: i32) -> (i32, i32) {
        self.view
            .canvas_coords_for_screen(self.board_view_offset(), screen_x, screen_y)
    }

    pub(crate) fn sync_canvas_pointer_to_current_transform(&mut self) {
        let (screen_x, screen_y) = self.pointer.screen();
        let canvas = self.canvas_coords_for_screen(screen_x, screen_y);
        self.pointer.set_canvas(canvas);
    }

    pub(crate) fn screen_coords_for_canvas(&self, canvas_x: i32, canvas_y: i32) -> (i32, i32) {
        self.view
            .screen_coords_for_canvas(self.board_view_offset(), canvas_x, canvas_y)
    }

    pub(crate) fn screen_rect_for_canvas(&self, rect: Rect) -> Option<Rect> {
        self.view
            .screen_rect_for_canvas(self.board_view_offset(), rect)
    }

    /// Returns the last known pointer position.
    pub(crate) fn pointer_position(&self) -> (i32, i32) {
        self.pointer.screen()
    }

    /// Returns the last known pointer position in canvas/world coordinates.
    pub(crate) fn canvas_pointer_position(&self) -> (i32, i32) {
        self.pointer.canvas()
    }

    pub(crate) fn paste_anchor(&self) -> PasteAnchor {
        if self.pointer.seen() {
            let (x, y) = self.pointer.canvas();
            PasteAnchor::Pointer { x, y }
        } else {
            let (x, y) = self.visible_canvas_center();
            PasteAnchor::VisibleCenter { x, y }
        }
    }

    /// Returns the visible canvas area, or a 1x1 fallback at its minimum corner
    /// when the transformed extent cannot be represented by [`Rect`].
    pub(crate) fn visible_canvas_rect(&self) -> Rect {
        self.view.visible_canvas_rect(self.board_view_offset())
    }

    fn visible_canvas_center(&self) -> (i32, i32) {
        self.view.visible_canvas_center(self.board_view_offset())
    }

    /// Updates the cached pointer location.
    pub fn update_pointer_position(&mut self, x: i32, y: i32) {
        let (canvas_x, canvas_y) = self.canvas_coords_for_screen(x, y);
        self.update_pointer_positions(x, y, canvas_x, canvas_y);
    }

    /// Updates cached screen and canvas pointer locations together.
    pub fn update_pointer_positions(
        &mut self,
        screen_x: i32,
        screen_y: i32,
        canvas_x: i32,
        canvas_y: i32,
    ) {
        self.pointer
            .update((screen_x, screen_y), (canvas_x, canvas_y));
        if self.click_highlight.update_tool_ring(
            self.highlight_tool_active(),
            canvas_x,
            canvas_y,
            &mut self.dirty_tracker,
        ) {
            self.needs_redraw = true;
        }
    }

    /// Updates the cached pointer location without triggering pointer-driven visuals.
    pub fn update_pointer_position_synthetic(&mut self, x: i32, y: i32) {
        let canvas = self.canvas_coords_for_screen(x, y);
        self.pointer.update_synthetic((x, y), canvas);
    }

    /// Record drawing activity (stroke start/commit); resets the top-strip
    /// idle-fade clock.
    pub(crate) fn mark_draw_activity(&mut self) {
        self.pointer.mark_draw_activity(Instant::now());
    }

    /// When drawing input last started or committed a stroke.
    pub fn last_draw_activity(&self) -> Instant {
        self.pointer.last_draw_activity()
    }

    #[cfg(test)]
    pub(crate) fn provisional_bounds(&self) -> Option<Rect> {
        self.pointer.provisional_bounds()
    }

    pub(crate) fn record_first_stroke_done_for_onboarding(&mut self) {
        if self.pending_onboarding_usage.first_stroke_done {
            return;
        }

        self.pending_onboarding_usage.first_stroke_done = true;
    }

    /// Updates the undo stack limit for subsequent actions.
    pub fn set_undo_stack_limit(&mut self, limit: usize) {
        self.history_limits.set_undo_stack_limit(limit);
    }

    /// Updates screen dimensions after backend configuration.
    ///
    /// This should be called by the backend when it receives the actual
    /// screen dimensions from the display server.
    pub fn update_screen_dimensions(&mut self, width: u32, height: u32) {
        self.view.set_screen_dimensions(width, height);
        // A surface resize is painted with full damage by the backend. Make
        // that newly painted geometry the picker's damage baseline, or the
        // next narrowing query clears the panel from before the resize instead
        // of the tall panel now visible at its new position.
        if self.font_picker.open {
            self.font_picker.last_panel = self.font_picker_panel_bounds();
        }
    }

    pub(crate) fn set_active_output_label(&mut self, label: Option<String>) -> bool {
        self.view.set_active_output_label(label)
    }

    pub(crate) fn active_output_label(&self) -> Option<&str> {
        self.view.active_output_label()
    }

    /// Cancels the current text input session and restores any edited shape.
    pub(crate) fn cancel_text_input_with(&mut self, measurer: &TextMeasurer) {
        self.cancel_text_edit_with(measurer);
        self.end_text_input_session();
    }

    /// Tears down the transient editor state shared by every text-input exit.
    pub(crate) fn end_text_input_session(&mut self) {
        self.text_editing.reset_composition_and_pointer();
        self.clear_pending_text_pastes();
        self.end_pointer_drag();
        self.clear_text_preview_dirty();
        self.style.text_wrap_width = None;
        self.state = DrawingState::Idle;
        self.needs_redraw = true;
    }

    /// Cancels the current interaction when one is active.
    ///
    /// Returns `true` when an active interaction consumed the caller's event.
    pub(crate) fn try_cancel_active_interaction(&mut self) -> bool {
        with_legacy_measurer(|measurer| self.try_cancel_active_interaction_with(measurer))
    }

    pub(crate) fn try_cancel_active_interaction_with(&mut self, measurer: &TextMeasurer) -> bool {
        if matches!(self.state, DrawingState::Idle) {
            return false;
        }

        self.cancel_active_interaction_with(measurer);
        true
    }

    /// Cancels any in-progress interaction without exiting the application.
    pub(crate) fn cancel_active_interaction(&mut self) {
        with_legacy_measurer(|measurer| self.cancel_active_interaction_with(measurer))
    }

    pub(crate) fn cancel_active_interaction_with(&mut self, measurer: &TextMeasurer) {
        // A canceled interaction never leaves a dangling block-move drag.
        self.text_editing.set_text_block_drag(None);
        match &self.state {
            DrawingState::TextInput { .. } => {
                self.cancel_text_input_with(measurer);
            }
            DrawingState::PendingTextClick { .. } => {
                self.state = DrawingState::Idle;
            }
            DrawingState::Drawing { .. } => {
                self.clear_provisional_dirty();
                self.state = DrawingState::Idle;
                self.needs_redraw = true;
            }
            DrawingState::BuildingPolygon { .. } => {
                self.clear_provisional_dirty();
                self.selection_interaction.clear_polygon_click();
                self.state = DrawingState::Idle;
                self.needs_redraw = true;
            }
            DrawingState::MovingSelection { snapshots, .. } => {
                self.restore_selection_from_snapshots_with(measurer, snapshots.clone());
                self.state = DrawingState::Idle;
            }
            DrawingState::Selecting { .. } => {
                self.clear_provisional_dirty();
                self.state = DrawingState::Idle;
                self.needs_redraw = true;
            }
            DrawingState::ResizingText {
                shape_id, snapshot, ..
            } => {
                self.restore_selection_from_snapshots_with(
                    measurer,
                    vec![(*shape_id, snapshot.clone())],
                );
                self.state = DrawingState::Idle;
            }
            DrawingState::BendingArrow { shape_id, snapshot }
            | DrawingState::AdjustingSpotlightMagnification { shape_id, snapshot } => {
                self.restore_selection_from_snapshots_with(
                    measurer,
                    vec![(*shape_id, snapshot.clone())],
                );
                self.state = DrawingState::Idle;
            }
            DrawingState::ResizingSelection { snapshots, .. } => {
                let snapshots = snapshots.clone();
                self.restore_resize_from_snapshots_with(measurer, snapshots.as_ref());
                self.state = DrawingState::Idle;
            }
            DrawingState::Idle => {}
        }
        if matches!(self.state, DrawingState::Idle) {
            self.end_pointer_drag();
        }
    }

    /// Drains pending dirty rectangles for the current surface size.
    #[allow(dead_code)]
    pub fn take_dirty_regions(&mut self) -> Vec<Rect> {
        self.take_dirty_region_report().regions
    }

    pub(crate) fn take_dirty_region_report(&mut self) -> DirtyRegionReport {
        let (screen_width, screen_height) = self.view.screen_size();
        let width = screen_width.min(i32::MAX as u32) as i32;
        let height = screen_height.min(i32::MAX as u32) as i32;
        self.dirty_tracker.take_region_report(width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::events::MouseButton;
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::{BOARD_ID_WHITEBOARD, Tool};

    #[test]
    fn update_pointer_position_synthetic_updates_pointer_without_redraw() {
        let mut state = make_test_input_state();
        state.needs_redraw = false;

        state.update_pointer_position_synthetic(12, 34);

        assert_eq!(state.pointer_position(), (12, 34));
        assert_eq!(state.canvas_pointer_position(), (12, 34));
        assert!(!state.needs_redraw);
    }

    #[test]
    fn update_pointer_position_synthetic_preserves_canvas_transform() {
        let mut state = make_test_input_state();
        state.set_zoom_status(true, false, 2.0, (100.0, 200.0));

        state.update_pointer_position_synthetic(30, 40);

        assert_eq!(state.pointer_position(), (30, 40));
        assert_eq!(state.canvas_pointer_position(), (115, 220));
    }

    #[test]
    fn update_pointer_position_uses_canvas_transform_for_screen_space_updates() {
        let mut state = make_test_input_state();
        state.switch_board(BOARD_ID_WHITEBOARD);
        assert!(state.boards.active_frame_mut().set_view_offset(100, 50));

        state.update_pointer_position(30, 40);

        assert_eq!(state.pointer_position(), (30, 40));
        assert_eq!(state.canvas_pointer_position(), (130, 90));
    }

    #[test]
    fn screen_rect_for_canvas_tracks_board_offset_after_pointer_cache_changes() {
        let mut state = make_test_input_state();
        state.switch_board(BOARD_ID_WHITEBOARD);
        assert!(state.boards.active_frame_mut().set_view_offset(100, 50));
        state.update_pointer_position(400, 300);
        let rect = Rect::new(138, 88, 24, 24).expect("valid rect");

        assert_eq!(
            state.screen_rect_for_canvas(rect),
            Rect::new(38, 38, 24, 24)
        );

        assert!(state.reset_active_canvas_position());
        assert_eq!(
            state.screen_rect_for_canvas(rect),
            Rect::new(138, 88, 24, 24)
        );
    }

    #[test]
    fn set_undo_stack_limit_clamps_to_at_least_one() {
        let mut state = make_test_input_state();
        state.set_undo_stack_limit(0);
        assert_eq!(state.history_limits.undo_stack_limit(), 1);

        state.set_undo_stack_limit(25);
        assert_eq!(state.history_limits.undo_stack_limit(), 25);
    }

    #[test]
    fn cancel_text_input_clears_wrap_width_and_returns_to_idle() {
        let measurer = crate::draw::TextMeasurer::default();
        let mut state = make_test_input_state();
        state.style.text_wrap_width = Some(240);
        state.state = DrawingState::text_input(10, 20, "hello".to_string());
        state.needs_redraw = false;

        state.cancel_text_input_with(&measurer);

        assert!(matches!(state.state, DrawingState::Idle));
        assert!(state.style.text_wrap_width.is_none());
        assert!(state.needs_redraw);
    }

    #[test]
    fn cancel_text_input_releases_an_active_block_drag() {
        let measurer = crate::draw::TextMeasurer::default();
        let mut state = make_test_input_state();
        state.state = DrawingState::text_input(10, 20, "hello".to_string());
        state.modifiers.alt = true;
        state.on_mouse_press_with_canvas(MouseButton::Left, 12, 20, 12, 20);
        assert!(state.text_block_drag_active());
        assert!(state.has_active_pointer_interaction());

        state.cancel_text_input_with(&measurer);

        assert!(!state.text_block_drag_active());
        assert!(!state.has_active_pointer_interaction());
    }

    #[test]
    fn try_cancel_active_interaction_reports_false_when_idle() {
        let mut state = make_test_input_state();
        state.needs_redraw = false;

        assert!(!state.try_cancel_active_interaction());

        assert!(matches!(state.state, DrawingState::Idle));
        assert!(!state.needs_redraw);
    }

    #[test]
    fn try_cancel_active_interaction_cancels_drawing_and_ends_drag() {
        let mut state = make_test_input_state();
        state.state = DrawingState::Drawing {
            tool: Tool::Pen,
            start_x: 10,
            start_y: 20,
            points: vec![(10, 20), (30, 40)],
            point_thicknesses: vec![1.0, 1.0],
        };
        state.begin_pointer_drag(MouseButton::Left, None);
        state.needs_redraw = false;

        assert!(state.try_cancel_active_interaction());

        assert!(matches!(state.state, DrawingState::Idle));
        assert!(!state.pointer_drag_active());
        assert!(state.needs_redraw);
    }

    #[test]
    fn take_dirty_regions_returns_full_surface_and_drains_tracker() {
        let mut state = make_test_input_state();
        state.update_screen_dimensions(100, 50);
        state.dirty_tracker.mark_full();

        assert_eq!(
            state.take_dirty_regions(),
            vec![Rect::new(0, 0, 100, 50).unwrap()]
        );
        assert!(state.take_dirty_regions().is_empty());
    }

    #[test]
    fn take_dirty_region_report_preserves_full_reason() {
        let mut state = make_test_input_state();
        state.update_screen_dimensions(100, 50);
        state
            .dirty_tracker
            .mark_full_for(crate::draw::DirtyFullReason::CanvasClear);

        let report = state.take_dirty_region_report();

        assert_eq!(report.regions, vec![Rect::new(0, 0, 100, 50).unwrap()]);
        assert_eq!(
            report.full_reason,
            Some(crate::draw::DirtyFullReason::CanvasClear)
        );
        assert!(state.take_dirty_region_report().regions.is_empty());
    }
}
