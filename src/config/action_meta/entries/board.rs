use super::ActionMeta;

pub const ENTRIES: &[ActionMeta] = &[
    meta!(
        ToggleWhiteboard,
        "Whiteboard Mode",
        None,
        "Toggle whiteboard background",
        Board,
        true,
        true,
        false
    ),
    meta!(
        ToggleBlackboard,
        "Blackboard Mode",
        None,
        "Toggle blackboard background",
        Board,
        true,
        true,
        false
    ),
    meta!(
        ReturnToTransparent,
        "Transparent Mode",
        None,
        "Return to transparent overlay",
        Board,
        true,
        true,
        false
    ),
    meta!(
        PagePrev,
        "Previous Page",
        Some("Prev"),
        "Go to previous page",
        Board,
        true,
        true,
        true
    ),
    meta!(
        PageNext,
        "Next Page",
        Some("Next"),
        "Go to next page",
        Board,
        true,
        true,
        true
    ),
    meta!(
        PageNew,
        "New Page",
        Some("New"),
        "Create a new page",
        Board,
        true,
        true,
        true
    ),
    meta!(
        PageDuplicate,
        "Duplicate Page",
        Some("Dup"),
        "Duplicate the current page",
        Board,
        false,
        true,
        true
    ),
    meta!(
        PageDelete,
        "Delete Page",
        Some("Del"),
        "Delete the current page",
        Board,
        false,
        true,
        true
    ),
    meta!(
        Board1,
        "Board 1",
        None,
        "Switch to board 1",
        Board,
        true,
        true,
        false
    ),
    meta!(
        Board2,
        "Board 2",
        None,
        "Switch to board 2",
        Board,
        true,
        true,
        false
    ),
    meta!(
        Board3,
        "Board 3",
        None,
        "Switch to board 3",
        Board,
        true,
        true,
        false
    ),
    meta!(
        Board4,
        "Board 4",
        None,
        "Switch to board 4",
        Board,
        true,
        true,
        false
    ),
    meta!(
        Board5,
        "Board 5",
        None,
        "Switch to board 5",
        Board,
        true,
        true,
        false
    ),
    meta!(
        Board6,
        "Board 6",
        None,
        "Switch to board 6",
        Board,
        true,
        true,
        false
    ),
    meta!(
        Board7,
        "Board 7",
        None,
        "Switch to board 7",
        Board,
        true,
        true,
        false
    ),
    meta!(
        Board8,
        "Board 8",
        None,
        "Switch to board 8",
        Board,
        true,
        true,
        false
    ),
    meta!(
        Board9,
        "Board 9",
        None,
        "Switch to board 9",
        Board,
        true,
        true,
        false
    ),
    meta!(
        BoardNext,
        "Next Board",
        Some("Next"),
        "Switch to next board",
        Board,
        true,
        true,
        true
    ),
    meta!(
        BoardPrev,
        "Previous Board",
        Some("Prev"),
        "Switch to previous board",
        Board,
        true,
        true,
        true
    ),
    meta!(
        FocusNextOutput,
        "Next Output",
        Some("Next Output"),
        "Move overlay focus to next output",
        Board,
        true,
        true,
        false
    ),
    meta!(
        FocusPrevOutput,
        "Previous Output",
        Some("Prev Output"),
        "Move overlay focus to previous output",
        Board,
        true,
        true,
        false
    ),
    meta!(
        BoardNew,
        "New Board",
        Some("New"),
        "Create a new board",
        Board,
        false,
        true,
        true
    ),
    meta!(
        BoardDelete,
        "Delete Board",
        Some("Del"),
        "Delete the active board",
        Board,
        false,
        true,
        true
    ),
    meta!(
        BoardPicker,
        "Board Picker",
        None,
        "Open the board picker",
        Board,
        true,
        true,
        true
    ),
    meta!(
        BoardRestoreDeleted,
        "Restore Deleted Board",
        Some("Restore"),
        "Restore the most recently deleted board",
        Board,
        true,
        true,
        false
    ),
    meta!(
        BoardDuplicate,
        "Duplicate Board",
        Some("Dup"),
        "Duplicate the active board",
        Board,
        true,
        true,
        true
    ),
    meta!(
        BoardSwitchRecent,
        "Recent Board",
        Some("Recent"),
        "Switch to most recent board",
        Board,
        true,
        true,
        false
    ),
];
