use super::super::base::InputState;
use crate::draw::{ArrowLabel, Shape};

const ARROW_LABEL_FONT_SCALE: f64 = 0.6;
const ARROW_LABEL_MIN_SIZE: f64 = 10.0;
const ARROW_LABEL_MAX_SIZE: f64 = 28.0;

impl InputState {
    pub(crate) fn next_arrow_label(&self) -> Option<ArrowLabel> {
        if !self.style.arrow_label_enabled {
            return None;
        }
        Some(ArrowLabel {
            value: self.style.arrow_label_counter.max(1),
            size: self.arrow_label_size(),
            font_descriptor: self.style.font_descriptor.clone(),
        })
    }

    pub(crate) fn bump_arrow_label(&mut self) {
        self.style.arrow_label_counter = self.style.arrow_label_counter.saturating_add(1);
    }

    pub(crate) fn reset_arrow_label_counter(&mut self) -> bool {
        if self.style.arrow_label_counter == 1 {
            return false;
        }
        self.style.arrow_label_counter = 1;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn set_arrow_label_enabled(&mut self, enabled: bool) -> bool {
        if self.style.arrow_label_enabled == enabled {
            return false;
        }
        self.style.arrow_label_enabled = enabled;
        if enabled {
            self.sync_arrow_label_counter();
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    pub(crate) fn sync_arrow_label_counter(&mut self) {
        let mut max_label = 0;
        for board in self.boards.board_states() {
            for frame in board.pages.pages() {
                for drawn in &frame.shapes {
                    if let Shape::Arrow {
                        label: Some(label), ..
                    } = &drawn.shape
                    {
                        max_label = max_label.max(label.value);
                    }
                }
            }
        }
        self.style.arrow_label_counter = max_label.saturating_add(1);
    }

    fn arrow_label_size(&self) -> f64 {
        (self.style.current_font_size * ARROW_LABEL_FONT_SCALE)
            .clamp(ARROW_LABEL_MIN_SIZE, ARROW_LABEL_MAX_SIZE)
    }
}
