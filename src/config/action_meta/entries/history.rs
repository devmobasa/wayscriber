use super::ActionMeta;

pub const ENTRIES: &[ActionMeta] = &[
    meta!(
        ClearCanvas,
        "Clear Canvas",
        Some("Clear"),
        "Remove all drawings",
        History,
        true,
        true,
        true
    ),
    meta!(
        Undo,
        "Undo",
        None,
        "Undo last action",
        History,
        true,
        true,
        true
    ),
    meta!(
        Redo,
        "Redo",
        None,
        "Redo last undone action",
        History,
        true,
        true,
        true
    ),
    meta!(
        UndoAll,
        "Undo All",
        None,
        "Undo all actions",
        History,
        false,
        false,
        true
    ),
    meta!(
        RedoAll,
        "Redo All",
        None,
        "Redo all actions",
        History,
        false,
        false,
        true
    ),
    meta!(
        UndoAllDelayed,
        "Undo All Delay",
        None,
        "Undo all actions with delay",
        History,
        false,
        false,
        true
    ),
    meta!(
        RedoAllDelayed,
        "Redo All Delay",
        None,
        "Redo all actions with delay",
        History,
        false,
        false,
        true
    ),
];
