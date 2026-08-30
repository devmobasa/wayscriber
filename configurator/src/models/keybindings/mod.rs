mod conflicts;
mod draft;
mod edit;
mod field;
mod manager;
mod parse;
mod recording;

pub use conflicts::{
    PendingShortcutConflict, apply_recorded_replace, apply_text_replace,
    field_has_internal_duplicate, other_claimants, text_conflicts_for,
};
pub use draft::KeybindingsDraft;
pub use edit::{
    AppendOutcome, ShortcutTextEditor, append_binding, field_matches_defaults, remove_binding,
    reset_field, reset_fields, reset_tooltip, serialize_bindings,
};
pub use field::{KeybindingField, keybinding_fields, keybinding_tab};
pub use manager::{
    ShortcutManagerFilter, ShortcutManagerSort, ShortcutManagerSummary, field_matching_search_term,
    next_review_conflict,
};
pub(crate) use parse::parse_keybindings;
#[cfg(test)]
pub(crate) use recording::keyval;
#[cfg(not(feature = "tablet-input"))]
pub use recording::tablet_unavailable_hint;
pub use recording::{
    KeyboardModifiers, RecordedDevice, RecordedKeyboard, RecorderDeviceKind, ShortcutRecorderState,
    normalize_button_event, normalize_key_event, sequence_keyboard_only_message,
    super_consumed_hint, waiting_prompt,
};

#[cfg(test)]
mod tests;
