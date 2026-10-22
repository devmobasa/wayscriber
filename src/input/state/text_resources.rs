//! Text services borrowed for one input operation.

use crate::draw::TextMeasurer;
use crate::ui_text::UiTextEngine;

/// Keeps shape geometry and UI sizing on their respective persistent owners.
/// This borrow is operation-local and must not be retained in input values,
/// editor state, shapes, or snapshots.
#[derive(Clone, Copy)]
pub(crate) struct InputTextResources<'text> {
    pub(crate) measurer: &'text TextMeasurer,
    pub(crate) ui_engine: &'text UiTextEngine,
}

#[cfg(test)]
mod tests;
