use super::base::{DrawingState, InputState, TextEditEntryFeedback, TextInputMode};
use super::index::SpatialGrid;
use super::{ColorPickerPopupLayout, ColorPickerPopupState};
use crate::draw::frame::ShapeSnapshot;
use crate::draw::{Color, DirtyTracker, FontDescriptor, ShapeId};
use crate::input::state::core::base::PolygonClickState;
use crate::input::state::highlight::ClickHighlightState;
use crate::input::tool::PerToolDrawingSettings;
use crate::input::{BoardManager, MouseButton};
use crate::util::Rect;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
struct ActiveInteractionRollback {
    boards: BoardManager,
    state: DrawingState,
    active_drag_button: Option<MouseButton>,
    active_drag_color: Option<Color>,
    current_color: Color,
    current_thickness: f64,
    tool_settings: PerToolDrawingSettings,
    current_font_size: f64,
    font_descriptor: FontDescriptor,
    text_background_enabled: bool,
    text_wrap_width: Option<i32>,
    text_input_mode: TextInputMode,
    text_edit_target: Option<(ShapeId, ShapeSnapshot)>,
    text_edit_entry_feedback: Option<TextEditEntryFeedback>,
    color_picker_popup_state: ColorPickerPopupState,
    color_picker_popup_layout: Option<ColorPickerPopupLayout>,
    active_preset_slot: Option<usize>,
    click_highlight: ClickHighlightState,
    needs_redraw: bool,
    session_dirty: bool,
    dirty_tracker: DirtyTracker,
    last_provisional_bounds: Option<Rect>,
    last_text_preview_bounds: Option<Rect>,
    last_polygon_click: Option<PolygonClickState>,
    hit_test_cache: HashMap<ShapeId, Rect>,
    spatial_index: Option<SpatialGrid>,
}

#[allow(dead_code)]
impl ActiveInteractionRollback {
    fn capture(input: &InputState) -> Self {
        Self {
            boards: input.boards.clone_preserving_identity_generation(),
            state: input.state.clone(),
            active_drag_button: input.active_drag_button,
            active_drag_color: input.active_drag_color,
            current_color: input.current_color,
            current_thickness: input.current_thickness,
            tool_settings: input.tool_settings.clone(),
            current_font_size: input.current_font_size,
            font_descriptor: input.font_descriptor.clone(),
            text_background_enabled: input.text_background_enabled,
            text_wrap_width: input.text_wrap_width,
            text_input_mode: input.text_input_mode,
            text_edit_target: input.text_edit_target.clone(),
            text_edit_entry_feedback: input.text_edit_entry_feedback.clone(),
            color_picker_popup_state: input.color_picker_popup_state.clone(),
            color_picker_popup_layout: input.color_picker_popup_layout,
            active_preset_slot: input.active_preset_slot,
            click_highlight: input.click_highlight.clone(),
            needs_redraw: input.needs_redraw,
            session_dirty: input.session_dirty,
            dirty_tracker: input.dirty_tracker.clone(),
            last_provisional_bounds: input.last_provisional_bounds,
            last_text_preview_bounds: input.last_text_preview_bounds,
            last_polygon_click: input.last_polygon_click,
            hit_test_cache: input.hit_test_cache.clone(),
            spatial_index: input.spatial_index.clone(),
        }
    }

    fn restore(self, input: &mut InputState) {
        input.boards = self.boards;
        input.state = self.state;
        input.active_drag_button = self.active_drag_button;
        input.active_drag_color = self.active_drag_color;
        input.current_color = self.current_color;
        input.current_thickness = self.current_thickness;
        input.tool_settings = self.tool_settings;
        input.current_font_size = self.current_font_size;
        input.font_descriptor = self.font_descriptor;
        input.text_background_enabled = self.text_background_enabled;
        input.text_wrap_width = self.text_wrap_width;
        input.text_input_mode = self.text_input_mode;
        input.text_edit_target = self.text_edit_target;
        input.text_edit_entry_feedback = self.text_edit_entry_feedback;
        input.color_picker_popup_state = self.color_picker_popup_state;
        input.color_picker_popup_layout = self.color_picker_popup_layout;
        input.active_preset_slot = self.active_preset_slot;
        input.click_highlight = self.click_highlight;
        input.needs_redraw = self.needs_redraw;
        input.session_dirty = self.session_dirty;
        input.dirty_tracker = self.dirty_tracker;
        input.last_provisional_bounds = self.last_provisional_bounds;
        input.last_text_preview_bounds = self.last_text_preview_bounds;
        input.last_polygon_click = self.last_polygon_click;
        input.hit_test_cache = self.hit_test_cache;
        input.spatial_index = self.spatial_index;
    }
}

impl InputState {
    /// Marks session data as dirty for autosave tracking.
    pub(crate) fn mark_session_dirty(&mut self) {
        self.session_dirty = true;
    }

    /// Returns true if session data was marked dirty since the last check.
    #[allow(dead_code)]
    pub(crate) fn take_session_dirty(&mut self) -> bool {
        if self.session_dirty {
            self.session_dirty = false;
            true
        } else {
            false
        }
    }

    /// Clears session dirtiness after loading persisted state into memory.
    #[allow(dead_code)]
    pub(crate) fn clear_session_dirty(&mut self) {
        self.session_dirty = false;
    }

    /// Returns whether session data is dirty without clearing the dirty flag.
    #[allow(dead_code)]
    pub(crate) fn is_session_dirty(&self) -> bool {
        self.session_dirty
    }

    #[allow(dead_code)]
    pub(crate) fn pending_save_as_overwrite(&self) -> Option<&Path> {
        self.pending_save_as_overwrite.as_deref()
    }

    #[allow(dead_code)]
    pub(crate) fn set_pending_save_as_overwrite(&mut self, path: PathBuf) {
        self.pending_save_as_overwrite = Some(path);
        self.needs_redraw = true;
    }

    #[allow(dead_code)]
    pub(crate) fn clear_pending_save_as_overwrite(&mut self) -> Option<PathBuf> {
        let previous = self.pending_save_as_overwrite.take();
        if previous.is_some() {
            self.needs_redraw = true;
        }
        previous
    }

    /// Returns true while pointer-driven work is in progress and autosave should wait.
    #[allow(dead_code)]
    pub(crate) fn has_active_pointer_interaction(&self) -> bool {
        self.active_drag_button.is_some()
            || matches!(
                self.state,
                DrawingState::Drawing { .. }
                    | DrawingState::BuildingPolygon { .. }
                    | DrawingState::PendingTextClick { .. }
                    | DrawingState::MovingSelection { .. }
                    | DrawingState::Selecting { .. }
                    | DrawingState::ResizingText { .. }
                    | DrawingState::ResizingSelection { .. }
            )
            || self.board_picker_is_dragging()
            || self.board_picker_is_page_dragging()
            || self.color_picker_popup_is_dragging()
            || self.radial_menu_is_size_dragging()
    }

    fn has_cancelable_session_capture_interaction(&self) -> bool {
        self.has_active_pointer_interaction()
            || matches!(self.state, DrawingState::TextInput { .. })
            || self.is_color_picker_popup_open()
    }

    #[allow(dead_code)]
    pub(crate) fn with_active_interaction_canceled_for_capture<T>(
        &mut self,
        capture: impl FnOnce(&Self) -> T,
    ) -> T {
        if !self.has_cancelable_session_capture_interaction() {
            return capture(self);
        }

        let rollback = ActiveInteractionRollback::capture(self);
        self.cancel_active_interaction();
        if self.is_color_picker_popup_open() {
            self.close_color_picker_popup(true);
        }
        let value = capture(self);
        rollback.restore(self);
        value
    }
}

#[cfg(test)]
mod tests {
    use crate::draw::frame::ShapeSnapshot;
    use crate::draw::{BLACK, Shape};
    use crate::input::state::core::board_picker::{BoardPickerDrag, BoardPickerPageDrag};
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::{BOARD_ID_BLACKBOARD, DrawingState, MouseButton, SelectionHandle, Tool};
    use crate::util::Rect;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[test]
    fn active_pointer_interaction_tracks_drag_button() {
        let mut state = make_test_input_state();
        assert!(!state.has_active_pointer_interaction());

        state.begin_pointer_drag(MouseButton::Left, None);
        assert!(state.has_active_pointer_interaction());

        state.end_pointer_drag();
        assert!(!state.has_active_pointer_interaction());
    }

    #[test]
    fn active_pointer_interaction_covers_drawing_states() {
        let states = vec![
            DrawingState::Drawing {
                tool: Tool::Pen,
                start_x: 10,
                start_y: 20,
                points: vec![(10, 20)],
                point_thicknesses: vec![2.0],
            },
            DrawingState::PendingTextClick {
                x: 10,
                y: 20,
                tool: Tool::Pen,
                shape_id: 1,
            },
            DrawingState::BuildingPolygon {
                points: vec![(10, 20)],
                preview: None,
                fill: false,
                color: BLACK,
                thick: 2.0,
            },
            DrawingState::MovingSelection {
                last_x: 10,
                last_y: 20,
                snapshots: Vec::new(),
                moved: false,
            },
            DrawingState::Selecting {
                start_x: 10,
                start_y: 20,
                additive: false,
            },
            DrawingState::ResizingText {
                shape_id: 1,
                snapshot: test_shape_snapshot(),
                base_x: 10,
                size: 12.0,
            },
            DrawingState::ResizingSelection {
                handle: SelectionHandle::BottomRight,
                original_bounds: Rect::new(0, 0, 20, 20).expect("valid rect"),
                start_x: 10,
                start_y: 20,
                snapshots: Arc::new(Vec::new()),
            },
        ];

        for drawing_state in states {
            let mut state = make_test_input_state();
            state.state = drawing_state;
            assert!(state.has_active_pointer_interaction());
        }
    }

    #[test]
    fn active_pointer_interaction_covers_picker_drags() {
        let mut state = make_test_input_state();
        state.board_picker_drag = Some(BoardPickerDrag {
            source_row: 0,
            source_board: 0,
            current_row: 0,
        });
        assert!(state.has_active_pointer_interaction());

        let mut state = make_test_input_state();
        state.board_picker_page_drag = Some(BoardPickerPageDrag {
            source_index: 0,
            current_index: 0,
            board_index: 0,
            target_board: Some(0),
        });
        assert!(state.has_active_pointer_interaction());

        let mut state = make_test_input_state();
        state.open_color_picker_popup();
        state.color_picker_popup_set_dragging(true);
        assert!(state.has_active_pointer_interaction());
    }

    #[test]
    fn session_capture_rollback_preserves_board_delete_confirmation_identity() {
        let mut state = make_test_input_state();
        state.switch_board(BOARD_ID_BLACKBOARD);
        assert_eq!(state.board_id(), BOARD_ID_BLACKBOARD);
        let requested_at = Instant::now();

        state.delete_active_board_at(requested_at);
        assert!(state.has_pending_board_delete());
        state.begin_pointer_drag(MouseButton::Left, None);

        state.with_active_interaction_canceled_for_capture(|input| {
            assert!(!input.has_active_pointer_interaction());
        });
        assert!(state.has_active_pointer_interaction());

        state.delete_active_board_at(requested_at + Duration::from_millis(1));

        assert!(!state.boards.has_board(BOARD_ID_BLACKBOARD));
        assert!(!state.has_pending_board_delete());
    }

    fn test_shape_snapshot() -> ShapeSnapshot {
        ShapeSnapshot {
            shape: Shape::Freehand {
                points: vec![(0, 0), (1, 1)],
                color: BLACK,
                thick: 1.0,
            },
            locked: false,
        }
    }
}
