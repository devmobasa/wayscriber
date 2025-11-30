use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

/// Backend-local intent type for toolbar interactions.
#[derive(Clone, Debug)]
pub struct ToolbarIntent(pub ToolbarEvent);

impl ToolbarIntent {
    pub fn into_event(self) -> ToolbarEvent {
        self.0
    }
}

/// Translate a backend intent into the UI-facing ToolbarEvent. Snapshot is
/// available for future context-sensitive mapping.
pub fn intent_to_event(intent: ToolbarIntent, _snapshot: Option<&ToolbarSnapshot>) -> ToolbarEvent {
    let _ = _snapshot;
    intent.into_event()
}
