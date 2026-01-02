use super::types::{BoardPagesSnapshot, BoardSnapshot, SessionSnapshot, ToolStateSnapshot};
use crate::input::InputState;
use crate::session::options::SessionOptions;

/// Capture a snapshot from the current input state if persistence is enabled.
pub fn snapshot_from_input(
    input: &InputState,
    options: &SessionOptions,
) -> Option<SessionSnapshot> {
    if !options.any_enabled() && !options.restore_tool_state && !options.persist_history {
        return None;
    }

    let mut snapshot = SessionSnapshot {
        active_board_id: input.board_id().to_string(),
        boards: Vec::new(),
        tool_state: None,
    };

    let history_limit = options.effective_history_limit(input.undo_stack_limit);

    let capture_pages = |pages: &crate::draw::BoardPages| -> Option<BoardPagesSnapshot> {
        let mut cloned_pages = pages.pages().to_vec();
        for page in &mut cloned_pages {
            if history_limit == 0 {
                page.clamp_history_depth(0);
            } else if history_limit < usize::MAX {
                page.clamp_history_depth(history_limit);
            }
        }
        let snapshot = BoardPagesSnapshot {
            pages: cloned_pages,
            active: pages.active_index(),
        };
        snapshot.has_persistable_data().then_some(snapshot)
    };

    let persist_non_transparent = options.persist_whiteboard || options.persist_blackboard;
    for board in input.boards.board_states() {
        let is_transparent = board.spec.background.is_transparent();
        let should_persist = if is_transparent {
            options.persist_transparent
        } else {
            persist_non_transparent && board.spec.persist
        };
        if !should_persist {
            continue;
        }
        if let Some(pages) = capture_pages(&board.pages) {
            snapshot.boards.push(BoardSnapshot {
                id: board.spec.id.clone(),
                pages,
            });
        }
    }

    if options.restore_tool_state {
        snapshot.tool_state = Some(ToolStateSnapshot::from_input_state(input));
    }

    if snapshot.is_empty() && snapshot.tool_state.is_none() {
        None
    } else {
        Some(snapshot)
    }
}
