//! Text services borrowed for one input operation.

use crate::draw::TextMeasurer;
use crate::ui_text::UiTextEngine;

/// Keeps shape geometry and UI sizing on their respective persistent owners.
/// This borrow is operation-local and must not be retained in input values,
/// editor state, shapes, or snapshots.
#[derive(Clone, Copy)]
pub(crate) struct InputTextResources<'a> {
    pub(crate) measurer: &'a TextMeasurer,
    pub(crate) ui_engine: &'a UiTextEngine,
}

/// Run a public convenience operation with isolated call-local resources.
/// Backend runtime paths pass their persistent owners explicitly instead.
pub(in crate::input::state) fn with_scoped_text_resources<R>(
    operation: impl FnOnce(InputTextResources<'_>) -> R,
) -> R {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    operation(InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    })
}

#[cfg(test)]
mod tests;
