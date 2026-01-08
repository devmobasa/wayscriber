use super::ActionMeta;

pub const ENTRIES: &[ActionMeta] = &[meta!(
    Exit,
    "Exit",
    None,
    "Close the overlay",
    Core,
    true,
    true,
    false
)];
