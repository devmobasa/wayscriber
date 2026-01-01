use std::time::Instant;

use crate::input::InputState;
use crate::input::state::core::TextClickState;

use super::super::{TEXT_DOUBLE_CLICK_DISTANCE, TEXT_DOUBLE_CLICK_MS};

pub(super) fn handle_pending_text_click(
    state: &mut InputState,
    x: i32,
    y: i32,
    shape_id: crate::draw::ShapeId,
) {
    let now = Instant::now();
    let is_double = state
        .last_text_click
        .map(|last| {
            last.shape_id == shape_id
                && now.duration_since(last.at).as_millis() <= TEXT_DOUBLE_CLICK_MS as u128
                && (x - last.x).abs() <= TEXT_DOUBLE_CLICK_DISTANCE
                && (y - last.y).abs() <= TEXT_DOUBLE_CLICK_DISTANCE
        })
        .unwrap_or(false);

    if is_double {
        state.last_text_click = None;
        state.set_selection(vec![shape_id]);
        let _ = state.edit_selected_text();
    } else {
        state.last_text_click = Some(TextClickState {
            shape_id,
            x,
            y,
            at: now,
        });
    }
}
