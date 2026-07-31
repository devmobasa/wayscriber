use super::ActionMeta;

pub const ENTRIES: &[ActionMeta] = &[
    meta!(
        SelectAll,
        "Select All",
        None,
        "Select all shapes",
        Selection,
        true,
        true,
        false
    ),
    meta!(
        DeleteSelection,
        "Delete Selection",
        None,
        "Delete selected shapes",
        Selection,
        true,
        true,
        false
    ),
    meta!(
        DuplicateSelection,
        "Duplicate Selection",
        None,
        "Duplicate selected shapes",
        Selection,
        true,
        true,
        false
    ),
    meta!(
        CopySelection,
        "Copy",
        None,
        "Copy selection to clipboard",
        Selection,
        true,
        true,
        false
    ),
    meta!(
        PasteSelection,
        "Paste",
        None,
        "Paste from clipboard",
        Selection,
        true,
        true,
        false
    ),
    meta!(
        MoveSelectionToFront,
        "Move Selection to Front",
        Some("Front"),
        "Raise selected shapes above the others",
        Selection,
        false,
        false,
        false
    ),
    meta!(
        MoveSelectionToBack,
        "Move Selection to Back",
        Some("Back"),
        "Lower selected shapes below the others",
        Selection,
        false,
        false,
        false
    ),
    meta!(
        NudgeSelectionUp,
        "Nudge Selection Up",
        None,
        "Move selected shapes up a small step",
        Selection,
        false,
        false,
        false
    ),
    meta!(
        NudgeSelectionDown,
        "Nudge Selection Down",
        None,
        "Move selected shapes down a small step",
        Selection,
        false,
        false,
        false
    ),
    meta!(
        NudgeSelectionLeft,
        "Nudge Selection Left",
        None,
        "Move selected shapes left a small step",
        Selection,
        false,
        false,
        false
    ),
    meta!(
        NudgeSelectionRight,
        "Nudge Selection Right",
        None,
        "Move selected shapes right a small step",
        Selection,
        false,
        false,
        false
    ),
    meta!(
        NudgeSelectionUpLarge,
        "Nudge Selection Up (Large)",
        None,
        "Move selected shapes up a large step",
        Selection,
        false,
        false,
        false
    ),
    meta!(
        NudgeSelectionDownLarge,
        "Nudge Selection Down (Large)",
        None,
        "Move selected shapes down a large step",
        Selection,
        false,
        false,
        false
    ),
    meta!(
        MoveSelectionToStart,
        "Move Selection to Start",
        None,
        "Move selected shapes to the left edge",
        Selection,
        false,
        false,
        false
    ),
    meta!(
        MoveSelectionToEnd,
        "Move Selection to End",
        None,
        "Move selected shapes to the right edge",
        Selection,
        false,
        false,
        false
    ),
    meta!(
        MoveSelectionToTop,
        "Move Selection to Top",
        None,
        "Move selected shapes to the top edge",
        Selection,
        false,
        false,
        false
    ),
    meta!(
        MoveSelectionToBottom,
        "Move Selection to Bottom",
        None,
        "Move selected shapes to the bottom edge",
        Selection,
        false,
        false,
        false
    ),
];
