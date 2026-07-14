mod apply;
pub(crate) mod bindings;
mod events;
// The shared model is being adopted incrementally by both toolbar frontends;
// some staged controls are not wired into a renderer yet.
#[allow(dead_code)]
pub(crate) mod model;
pub mod snapshot;

pub use bindings::ToolbarBindingHints;
pub use events::{SidePane, ToolbarEvent, ToolbarItemCustomizeGroup, ToolbarSideSection};
#[allow(unused_imports)]
pub use snapshot::{
    PresetFeedbackSnapshot, PresetSlotSnapshot, SessionRecentSnapshot, ToolContext,
    ToolOptionsKind, ToolbarSnapshot,
};
