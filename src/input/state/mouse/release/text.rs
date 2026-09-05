use std::time::Instant;

use crate::input::InputState;

use super::super::{TEXT_DOUBLE_CLICK_DISTANCE, TEXT_DOUBLE_CLICK_MS};

pub(super) fn handle_pending_text_click(
    state: &mut InputState,
    measurer: &crate::draw::TextMeasurer,
    x: i32,
    y: i32,
    shape_id: crate::draw::ShapeId,
) {
    let is_double = state.text_editing.register_click(
        shape_id,
        x,
        y,
        Instant::now(),
        TEXT_DOUBLE_CLICK_MS,
        TEXT_DOUBLE_CLICK_DISTANCE,
    );
    if is_double {
        state.set_selection(vec![shape_id]);
        let _ = state.edit_selected_text_with(measurer);
    }
}
