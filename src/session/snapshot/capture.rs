use super::types::{BoardPagesSnapshot, SessionSnapshot, ToolStateSnapshot};
use crate::input::{InputState, board_mode::BoardMode};
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
        active_mode: input.board_mode(),
        transparent: None,
        whiteboard: None,
        blackboard: None,
        tool_state: None,
    };

    let history_limit = options.effective_history_limit(input.undo_stack_limit);

    let capture_pages = |mode: BoardMode| -> Option<BoardPagesSnapshot> {
        let pages = input.canvas_set.pages(mode)?;
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

    if options.persist_transparent {
        snapshot.transparent = capture_pages(BoardMode::Transparent);
    }

    if options.persist_whiteboard {
        snapshot.whiteboard = capture_pages(BoardMode::Whiteboard);
    }

    if options.persist_blackboard {
        snapshot.blackboard = capture_pages(BoardMode::Blackboard);
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
