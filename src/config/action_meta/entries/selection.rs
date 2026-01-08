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
];
