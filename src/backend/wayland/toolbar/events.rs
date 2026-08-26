use crate::config::{ToolbarItemId, ToolbarItemOrderGroup};

/// Kinds of hit regions and their drag semantics.
#[derive(Clone, Debug, PartialEq)]
pub enum HitKind {
    Click,
    DragSetThickness {
        min: f64,
        max: f64,
    },
    DragSetMarkerOpacity {
        min: f64,
        max: f64,
    },
    DragSetSpotlightMagnification,
    DragSetPenSmoothing,
    DragSetFontSize,
    DragUndoDelay,
    DragRedoDelay,
    DragCustomUndoDelay,
    DragCustomRedoDelay,
    DragMoveTop,
    /// Internal scrollbar of the top strip's Canvas/Session/Settings popover.
    DragScrollTopPopover {
        max_scroll: f64,
    },
    DragToolbarItem {
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
        target_index: usize,
    },
}

/// Cursor hint for toolbar regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarCursorHint {
    /// Default arrow cursor.
    Default,
    /// Pointer/hand cursor for clickable buttons.
    Pointer,
    /// Grab cursor for sliders and drag handles.
    Grab,
}

impl HitKind {
    /// Get the appropriate cursor hint for this hit kind.
    pub fn cursor_hint(&self) -> ToolbarCursorHint {
        match self {
            HitKind::Click => ToolbarCursorHint::Pointer,
            HitKind::DragSetThickness { .. }
            | HitKind::DragSetMarkerOpacity { .. }
            | HitKind::DragSetSpotlightMagnification
            | HitKind::DragSetPenSmoothing
            | HitKind::DragSetFontSize
            | HitKind::DragUndoDelay
            | HitKind::DragRedoDelay
            | HitKind::DragCustomUndoDelay
            | HitKind::DragCustomRedoDelay
            | HitKind::DragMoveTop
            | HitKind::DragScrollTopPopover { .. }
            | HitKind::DragToolbarItem { .. } => ToolbarCursorHint::Grab,
        }
    }
}
