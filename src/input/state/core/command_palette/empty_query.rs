//! What the palette offers before anything is typed.
//!
//! An empty query used to list the registry in order, so Exit (the only Core
//! command) opened highlighted at the top and Ctrl+K, Enter closed the
//! overlay, with Clear Canvas right below it. Now recent commands come first,
//! then everyday ones, then the rest by category, and exit and destructive
//! commands wait at the very end, where nothing preselects them.

use super::{CommandEntry, command_palette_entries};
use crate::domain::Action;

/// Group label shown above recent commands when the query is empty.
pub(crate) const COMMAND_PALETTE_RECENT_HEADER: &str = "Recent";
/// Group label shown above the everyday commands when the query is empty.
pub(crate) const COMMAND_PALETTE_COMMON_HEADER: &str = "Common";
/// Group label of the trailing exit/destructive block.
pub(crate) const COMMAND_PALETTE_CAREFUL_HEADER: &str = "Clear, delete & exit";

/// Everyday commands offered right after the recents, in this order.
const COMMON_COMMANDS: &[Action] = &[
    Action::Undo,
    Action::Redo,
    Action::SelectPenTool,
    Action::SelectMarkerTool,
    Action::SelectEraserTool,
    Action::EnterTextMode,
    Action::SelectSelectionTool,
    Action::CaptureRegionInteractive,
    Action::ToggleWhiteboard,
    Action::ToggleHelp,
];

/// Commands that end the session or throw work away. With an empty query
/// they are listed last, even when recently used, so none is ever the
/// preselected row that Enter would run.
pub(crate) fn command_is_exit_or_destructive(action: Action) -> bool {
    matches!(
        action,
        Action::Exit
            | Action::ClearCanvas
            | Action::DeleteSelection
            | Action::BoardDelete
            | Action::PageDelete
            | Action::ClearSavedToolState
            | Action::ClearPreset1
            | Action::ClearPreset2
            | Action::ClearPreset3
            | Action::ClearPreset4
            | Action::ClearPreset5
    )
}

/// Sizes of the leading groups of an empty-query list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct EmptyQueryGroups {
    pub(super) recent_len: usize,
    pub(super) common_len: usize,
}

impl EmptyQueryGroups {
    /// The group header for the command at `index` of an empty-query list,
    /// or `None` when its category names the group.
    pub(super) fn header(self, index: usize, action: Action) -> Option<&'static str> {
        if index < self.recent_len {
            Some(COMMAND_PALETTE_RECENT_HEADER)
        } else if index < self.recent_len + self.common_len {
            Some(COMMAND_PALETTE_COMMON_HEADER)
        } else if command_is_exit_or_destructive(action) {
            Some(COMMAND_PALETTE_CAREFUL_HEADER)
        } else {
            None
        }
    }
}

/// The empty-query list: recents, everyday commands, everything else in
/// registry order, then the exit and destructive commands.
pub(super) fn empty_query_commands(recent: &[Action]) -> Vec<&'static CommandEntry> {
    let entries: Vec<&'static CommandEntry> = command_palette_entries().collect();
    let mut ordered: Vec<&'static CommandEntry> = Vec::with_capacity(entries.len());
    let push = |ordered: &mut Vec<&'static CommandEntry>, action: Action| {
        if command_is_exit_or_destructive(action)
            || ordered.iter().any(|command| command.action == action)
        {
            return;
        }
        if let Some(command) = entries.iter().find(|command| command.action == action) {
            ordered.push(command);
        }
    };

    for action in recent.iter().chain(COMMON_COMMANDS) {
        push(&mut ordered, *action);
    }
    for command in &entries {
        push(&mut ordered, command.action);
    }
    ordered.extend(
        entries
            .iter()
            .filter(|command| command_is_exit_or_destructive(command.action)),
    );

    ordered
}

/// Measures the recent and common groups at the head of `ordered`.
pub(super) fn empty_query_groups(
    ordered: &[&'static CommandEntry],
    recent: &[Action],
) -> EmptyQueryGroups {
    let recent_len = ordered
        .iter()
        .take_while(|command| recent.contains(&command.action))
        .count();
    let common_len = ordered[recent_len..]
        .iter()
        .take_while(|command| COMMON_COMMANDS.contains(&command.action))
        .count();

    EmptyQueryGroups {
        recent_len,
        common_len,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions(ordered: &[&'static CommandEntry]) -> Vec<Action> {
        ordered.iter().map(|command| command.action).collect()
    }

    #[test]
    fn nothing_typed_leads_with_everyday_commands_and_ends_with_exit() {
        let ordered = empty_query_commands(&[]);
        let actions = actions(&ordered);

        assert_eq!(actions.first(), Some(&Action::Undo));
        assert!(!command_is_exit_or_destructive(actions[0]));
        let exit = actions.iter().position(|action| *action == Action::Exit);
        let clear = actions
            .iter()
            .position(|action| *action == Action::ClearCanvas);
        let first_careful = actions
            .iter()
            .position(|action| command_is_exit_or_destructive(*action))
            .expect("a destructive block");
        assert!(exit.expect("exit listed") >= first_careful);
        assert!(clear.expect("clear listed") >= first_careful);
        assert!(
            actions[first_careful..]
                .iter()
                .all(|action| command_is_exit_or_destructive(*action)),
            "the destructive block is the tail of the list"
        );
        assert_eq!(
            actions.len(),
            command_palette_entries().count(),
            "every command appears exactly once"
        );
    }

    #[test]
    fn recents_come_first_but_a_recent_destructive_command_stays_last() {
        let recent = [
            Action::ClearCanvas,
            Action::TogglePresenterMode,
            Action::Undo,
        ];
        let ordered = empty_query_commands(&recent);
        let actions = actions(&ordered);
        let groups = empty_query_groups(&ordered, &recent);

        assert_eq!(&actions[..2], &[Action::TogglePresenterMode, Action::Undo]);
        assert_eq!(groups.recent_len, 2);
        assert_eq!(actions[2], Action::Redo, "Undo is not listed twice");
        assert!(
            actions.iter().rposition(|a| *a == Action::ClearCanvas) > Some(groups.recent_len),
            "a recent Clear Canvas still waits at the end"
        );
    }

    #[test]
    fn group_headers_follow_the_blocks() {
        let recent = [Action::TogglePresenterMode];
        let ordered = empty_query_commands(&recent);
        let groups = empty_query_groups(&ordered, &recent);
        assert_eq!(groups.recent_len, 1);
        assert_eq!(groups.common_len, COMMON_COMMANDS.len());

        assert_eq!(
            groups.header(0, ordered[0].action),
            Some(COMMAND_PALETTE_RECENT_HEADER)
        );
        assert_eq!(
            groups.header(1, ordered[1].action),
            Some(COMMAND_PALETTE_COMMON_HEADER)
        );
        let after_common = groups.recent_len + groups.common_len;
        assert_eq!(
            groups.header(after_common, ordered[after_common].action),
            None
        );
        let last = ordered.len() - 1;
        assert_eq!(
            groups.header(last, ordered[last].action),
            Some(COMMAND_PALETTE_CAREFUL_HEADER)
        );
    }
}
