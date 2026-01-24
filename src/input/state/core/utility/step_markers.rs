use super::super::base::InputState;
use crate::draw::{Shape, StepMarkerLabel};

const STEP_MARKER_FONT_SCALE: f64 = 0.6;
const STEP_MARKER_MIN_SIZE: f64 = 12.0;
const STEP_MARKER_MAX_SIZE: f64 = 36.0;

impl InputState {
    pub(crate) fn next_step_marker_label(&self) -> StepMarkerLabel {
        StepMarkerLabel {
            value: self.step_marker_counter.max(1),
            size: self.step_marker_size(),
            font_descriptor: self.font_descriptor.clone(),
        }
    }

    pub(crate) fn bump_step_marker(&mut self) {
        self.step_marker_counter = self.step_marker_counter.saturating_add(1);
    }

    pub(crate) fn reset_step_marker_counter(&mut self) -> bool {
        if self.step_marker_counter == 1 {
            return false;
        }
        self.step_marker_counter = 1;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn sync_step_marker_counter(&mut self) {
        let mut max_label = 0;
        for board in self.boards.board_states() {
            for frame in board.pages.pages() {
                for drawn in &frame.shapes {
                    if let Shape::StepMarker { label, .. } = &drawn.shape {
                        max_label = max_label.max(label.value);
                    }
                }
            }
        }
        self.step_marker_counter = max_label.saturating_add(1);
    }

    fn step_marker_size(&self) -> f64 {
        (self.current_font_size * STEP_MARKER_FONT_SCALE)
            .clamp(STEP_MARKER_MIN_SIZE, STEP_MARKER_MAX_SIZE)
    }
}
