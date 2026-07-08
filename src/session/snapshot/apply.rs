use super::types::{BoardPagesSnapshot, SessionSnapshot, ToolStateSnapshot};
use crate::draw::{BoardPages, clamp_regular_sides};
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};
use crate::input::{BOARD_ID_TRANSPARENT, InputState, PerToolDrawingSettings};
use crate::session::options::SessionOptions;
use anyhow::{Result, anyhow};
use std::collections::HashSet;

/// Apply a session snapshot to the live [`InputState`].
pub fn apply_snapshot(input: &mut InputState, snapshot: SessionSnapshot, options: &SessionOptions) {
    apply_snapshot_inner(input, snapshot, options, None);
}

fn apply_snapshot_inner(
    input: &mut InputState,
    snapshot: SessionSnapshot,
    options: &SessionOptions,
    replacement_board_ids: Option<&HashSet<String>>,
) {
    let runtime_history_limit = options.effective_history_limit(input.undo_stack_limit);
    let board_generation_before = input.boards.board_identity_generation();
    input.clear_pending_delete_confirmations();

    for board in &snapshot.boards {
        if !input.boards.has_board(&board.id)
            && let Some(replacement_board_ids) = replacement_board_ids
        {
            input
                .boards
                .release_session_replace_slot(replacement_board_ids);
        }
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
    input.clear_pending_deletes_after_board_generation_change(board_generation_before);

    if input.boards.has_board(&snapshot.active_board_id) {
        input.switch_board_force(&snapshot.active_board_id);
    } else {
        log::warn!(
            "Session active board '{}' missing after restore; keeping current board '{}'",
            snapshot.active_board_id,
            input.board_id()
        );
    }

    if options.restore_tool_state {
        if let Some(tool_state) = snapshot.tool_state {
            apply_tool_state_snapshot(input, tool_state);
        } else {
            log::info!("No tool state found in session; skipping tool restore");
        }
    }

    input.sync_step_marker_counter();
    input.needs_redraw = true;
}

/// Apply persisted or config-derived tool state to the live [`InputState`].
pub(crate) fn apply_tool_state_snapshot(input: &mut InputState, tool_state: ToolStateSnapshot) {
    let marker_opacity = tool_state.marker_opacity.unwrap_or(input.marker_opacity);
    let fill_enabled = tool_state.fill_enabled.unwrap_or(input.fill_enabled);
    log::info!(
        "Applying tool state: color={:?}, thickness={:.2}, eraser[size={:.2}, kind={:?}, mode={:?}], marker_opacity={:.2}, fill_enabled={}, tool_override={:?}, font_size={:.1}, text_bg={}, arrow[length={:.1}, angle={:.1}], status_bar={}, prev_color={:?}, arrow_labels={:?}",
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
    let current_thickness = tool_state
        .current_thickness
        .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
    let tool_settings = tool_state.tool_settings.unwrap_or_else(|| {
        let mut settings = PerToolDrawingSettings::new(tool_state.current_color, current_thickness);
        settings.step_marker.thickness =
            crate::input::state::default_step_marker_size(tool_state.current_font_size);
        settings
    });
    let tool_settings = tool_settings.clamp_thicknesses(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
    input.replace_tool_settings(tool_settings);
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
    if let Some(font_descriptor) = tool_state.font_descriptor {
        let _ = input.set_font_descriptor(font_descriptor);
    }
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
    input.polygon_sides = clamp_regular_sides(tool_state.polygon_sides);
    input.board_previous_color = tool_state.board_previous_color;
    input.show_status_bar = tool_state.show_status_bar;
    input.sync_step_marker_counter();
    input.needs_redraw = true;
}

/// Replace live board page contents with a session snapshot.
///
/// Startup restore keeps any boards that are absent from an older or partial
/// snapshot. Runtime session switching needs stronger replacement semantics so
/// pages from the previously opened session cannot leak into the newly opened
/// one.
#[allow(dead_code)]
pub(crate) fn apply_snapshot_replacing_boards(
    input: &mut InputState,
    snapshot: SessionSnapshot,
    options: &SessionOptions,
) -> Result<()> {
    let replacement_board_ids = snapshot
        .boards
        .iter()
        .map(|board| board.id.clone())
        .collect::<HashSet<_>>();
    let preserves_overlay = input.boards.has_board(BOARD_ID_TRANSPARENT)
        && !replacement_board_ids.contains(BOARD_ID_TRANSPARENT);
    let available_slots = input
        .boards
        .max_count()
        .saturating_sub(usize::from(preserves_overlay));
    if replacement_board_ids.len() > available_slots {
        return Err(anyhow!(
            "session snapshot contains {} boards but the current runtime allows {} while preserving the overlay board",
            replacement_board_ids.len(),
            available_slots
        ));
    }
    clear_board_pages(input);
    apply_snapshot_inner(input, snapshot, options, Some(&replacement_board_ids));
    input.dirty_tracker.mark_full();
    input.sync_canvas_pointer_to_current_transform();
    Ok(())
}

fn clear_board_pages(input: &mut InputState) {
    input.cancel_active_interaction();
    if input.is_board_picker_open() {
        input.close_board_picker();
    }
    if input.is_color_picker_popup_open() {
        input.close_color_picker_popup(true);
    }
    input.clear_selection();
    input.close_context_menu();
    input.invalidate_hit_cache();
    input.sync_canvas_pointer_to_current_transform();
    input.clear_session_delete_restore_state();
    for board in input.boards.board_states_mut() {
        board.pages = BoardPages::new();
        board.pages.bump_generation();
    }
}

fn snapshot_to_board_pages(pages: BoardPagesSnapshot) -> BoardPages {
    BoardPages::from_pages(pages.pages, pages.active)
}

fn clamp_runtime_history(pages: &mut BoardPages, limit: usize) {
    for page in pages.pages_mut() {
        page.clamp_history_depth(limit);
    }
}
