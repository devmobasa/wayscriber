mod conflicts;
mod draft;
mod edit;
mod field;
mod parse;
mod recording;

pub use conflicts::{
    PendingShortcutConflict, apply_recorded_replace, apply_text_replace,
    field_has_internal_duplicate, other_claimants, text_conflicts_for,
};
pub use draft::KeybindingsDraft;
pub use edit::{
    AppendOutcome, ShortcutTextEditor, append_binding, field_matches_defaults, remove_binding,
    reset_field, reset_tooltip, serialize_bindings,
};
pub use field::KeybindingField;
pub(crate) use parse::parse_keybindings;
#[cfg(test)]
pub(crate) use recording::keyval;
pub use recording::{
    KeyboardModifiers, RecordedKeyboard, ShortcutRecorderState, normalize_key_event, waiting_prompt,
};

#[cfg(test)]
mod tests;
