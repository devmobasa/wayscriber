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

/// Temporary adapter for callers whose input roots have not migrated yet.
pub(in crate::input::state) fn with_legacy_text_resources<R>(
    operation: impl FnOnce(InputTextResources<'_>) -> R,
) -> R {
    crate::draw::with_legacy_measurer(|measurer| {
        crate::ui_text::with_legacy_engine(|ui_engine| {
            operation(InputTextResources {
                measurer,
                ui_engine,
            })
        })
    })
}

#[cfg(test)]
mod tests;
