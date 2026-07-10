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
    DragSetFontSize,
    /// 2-D saturation/value area; the fixed hue rides along so the drag
    /// handler can rebuild the full color from the pointer position and the
    /// hit rect alone (payload rects are banned — the rect IS the region).
    PickSatVal {
        hue: f64,
    },
    /// Hue bar; the fixed saturation/value ride along like PickSatVal's hue.
    PickHue {
        sat: f64,
        val: f64,
    },
    DragUndoDelay,
    DragRedoDelay,
    DragCustomUndoDelay,
    DragCustomRedoDelay,
    DragMoveTop,
    DragMoveSide,
    DragScrollSide {
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
    /// Crosshair for color pickers.
    Crosshair,
}

impl HitKind {
    /// Get the appropriate cursor hint for this hit kind.
    pub fn cursor_hint(&self) -> ToolbarCursorHint {
        match self {
            HitKind::Click => ToolbarCursorHint::Pointer,
            HitKind::DragSetThickness { .. }
            | HitKind::DragSetMarkerOpacity { .. }
            | HitKind::DragSetFontSize
            | HitKind::DragUndoDelay
            | HitKind::DragRedoDelay
            | HitKind::DragCustomUndoDelay
            | HitKind::DragCustomRedoDelay
            | HitKind::DragMoveTop
            | HitKind::DragMoveSide
            | HitKind::DragScrollSide { .. }
            | HitKind::DragToolbarItem { .. } => ToolbarCursorHint::Grab,
            HitKind::PickSatVal { .. } | HitKind::PickHue { .. } => ToolbarCursorHint::Crosshair,
        }
    }
}
