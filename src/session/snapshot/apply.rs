use super::types::{BoardPagesSnapshot, SessionSnapshot};
use crate::draw::BoardPages;
use crate::input::InputState;
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};
use crate::session::options::SessionOptions;

/// Apply a session snapshot to the live [`InputState`].
pub fn apply_snapshot(input: &mut InputState, snapshot: SessionSnapshot, options: &SessionOptions) {
    let runtime_history_limit = options.effective_history_limit(input.undo_stack_limit);

    for board in &snapshot.boards {
        let pages = snapshot_to_board_pages(board.pages.clone());
        if input.boards.set_board_pages(&board.id, pages)
            && let Some(board_state) = input
                .boards
                .board_states_mut()
                .iter_mut()
                .find(|state| state.spec.id == board.id)
        {
            clamp_runtime_history(&mut board_state.pages, runtime_history_limit);
        }
    }

    input.switch_board_force(&snapshot.active_board_id);

    if options.restore_tool_state {
        if let Some(tool_state) = snapshot.tool_state {
            let marker_opacity = tool_state.marker_opacity.unwrap_or(input.marker_opacity);
            let fill_enabled = tool_state.fill_enabled.unwrap_or(input.fill_enabled);
            log::info!(
                "Restoring tool state: color={:?}, thickness={:.2}, eraser[size={:.2}, kind={:?}, mode={:?}], marker_opacity={:.2}, fill_enabled={}, tool_override={:?}, font_size={:.1}, text_bg={}, arrow[length={:.1}, angle={:.1}], status_bar={}, prev_color={:?}, arrow_labels={:?}",
                tool_state.current_color,
                tool_state.current_thickness,
                tool_state.eraser_size,
                tool_state.eraser_kind,
                tool_state.eraser_mode,
                marker_opacity,
                fill_enabled,
                tool_state.tool_override,
                tool_state.current_font_size,
                tool_state.text_background_enabled,
                tool_state.arrow_length,
                tool_state.arrow_angle,
                tool_state.show_status_bar,
                tool_state.board_previous_color,
                tool_state.arrow_label_enabled
            );
            let _ = input.set_color(tool_state.current_color);
            let _ = input.set_thickness(
                tool_state
                    .current_thickness
                    .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS),
            );
            let _ = input.set_eraser_size(
                tool_state
                    .eraser_size
                    .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS),
            );
            input.eraser_kind = tool_state.eraser_kind;
            let _ = input.set_eraser_mode(tool_state.eraser_mode);
            if let Some(opacity) = tool_state.marker_opacity {
                let _ = input.set_marker_opacity(opacity);
            }
            if let Some(fill_enabled) = tool_state.fill_enabled {
                let _ = input.set_fill_enabled(fill_enabled);
            }
            let _ = input.set_tool_override(tool_state.tool_override);
            let _ = input.set_font_size(tool_state.current_font_size.clamp(8.0, 72.0));
            input.text_background_enabled = tool_state.text_background_enabled;
            input.arrow_length = tool_state.arrow_length.clamp(5.0, 50.0);
            input.arrow_angle = tool_state.arrow_angle.clamp(15.0, 60.0);
            if let Some(head_at_end) = tool_state.arrow_head_at_end {
                input.arrow_head_at_end = head_at_end;
            }
            if let Some(label_enabled) = tool_state.arrow_label_enabled {
                input.arrow_label_enabled = label_enabled;
            }
            input.board_previous_color = tool_state.board_previous_color;
            input.show_status_bar = tool_state.show_status_bar;
        } else {
            log::info!("No tool state found in session; skipping tool restore");
        }
    }

    input.needs_redraw = true;
}

fn snapshot_to_board_pages(pages: BoardPagesSnapshot) -> BoardPages {
    BoardPages::from_pages(pages.pages, pages.active)
}

fn clamp_runtime_history(pages: &mut BoardPages, limit: usize) {
    for page in pages.pages_mut() {
        page.clamp_history_depth(limit);
    }
}
