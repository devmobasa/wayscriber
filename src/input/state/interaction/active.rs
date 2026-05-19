use super::outcome::ActiveInteractionKind;
use crate::input::state::{DrawingState, InputState};

pub(crate) fn active_interaction_kind(state: &InputState) -> Option<ActiveInteractionKind> {
    match state.state {
        DrawingState::Idle => None,
        DrawingState::Drawing { .. } => Some(ActiveInteractionKind::Drawing),
        DrawingState::TextInput { .. } => Some(ActiveInteractionKind::TextInput),
        DrawingState::PendingTextClick { .. } => Some(ActiveInteractionKind::PendingTextClick),
        DrawingState::MovingSelection { .. } => Some(ActiveInteractionKind::MovingSelection),
        DrawingState::Selecting { .. } => Some(ActiveInteractionKind::BoxSelecting),
        DrawingState::ResizingText { .. } => Some(ActiveInteractionKind::ResizingText),
        DrawingState::ResizingSelection { .. } => Some(ActiveInteractionKind::ResizingSelection),
    }
}
