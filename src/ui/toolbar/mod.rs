mod apply;
pub(crate) mod bindings;
mod events;
// The model is consumed by the bin's backend render layer; the lib target
// builds this crate without the backend, so references are target-dependent.
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
