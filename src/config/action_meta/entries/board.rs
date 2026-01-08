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
];
