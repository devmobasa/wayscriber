use super::types::{BoardPagesSnapshot, SessionSnapshot, ToolStateSnapshot};
use crate::draw::BoardPages;
use crate::input::{BOARD_ID_TRANSPARENT, InputState};
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
///
/// Drawing state only. Chrome the user toggles for the running process — the
/// status bar and the badges around it — is configured in `config.toml` and
/// deliberately absent from the snapshot, so restoring a session leaves the
/// configured value in place instead of reinstating a toggle that promised to
/// last one run.
pub(crate) fn apply_tool_state_snapshot(input: &mut InputState, tool_state: ToolStateSnapshot) {
    let marker_opacity = tool_state
        .marker_opacity
        .unwrap_or(input.style.marker_opacity);
    let fill_enabled = tool_state.fill_enabled.unwrap_or(input.style.fill_enabled);
    log::info!(
        "Applying tool state: color={:?}, thickness={:.2}, eraser[size={:.2}, kind={:?}, mode={:?}], marker_opacity={:.2}, fill_enabled={}, tool_override={:?}, font_size={:.1}, text_bg={}, arrow[length={:.1}, angle={:.1}], prev_color={:?}, arrow_labels={:?}",
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
        tool_state.board_previous_color,
        tool_state.arrow_label_enabled
    );
    let active_tool = input.active_tool();
    input.style.restore_snapshot(&tool_state, active_tool);
    input.sync_highlight_color();
    let _ = input.set_tool_override(tool_state.tool_override);
    input.board_previous_color = tool_state.board_previous_color;
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
    // Every page is about to be replaced. A wheel adjustment still in flight
    // belongs to a frame that will not exist afterwards, so record it now
    // rather than letting the identity guard drop it.
    input.flush_spotlight_magnification_gesture();
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
