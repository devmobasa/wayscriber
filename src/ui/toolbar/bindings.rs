use std::collections::HashMap;

use crate::config::{Action, action_label, action_meta_iter, action_short_label};
use crate::input::{InputState, Tool};
use crate::label_format::join_binding_labels;

use super::events::ToolbarEvent;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ToolbarBindingHints {
    bindings: HashMap<Action, String>,
}

impl ToolbarBindingHints {
    pub fn for_tool(&self, tool: Tool) -> Option<&str> {
        action_for_tool(tool).and_then(|action| self.binding_for_action(action))
    }

    pub fn from_input_state(state: &InputState) -> Self {
        let mut bindings = HashMap::new();
        for meta in action_meta_iter().filter(|meta| meta.in_toolbar) {
            let labels = state.action_binding_labels(meta.action);
            if let Some(label) = join_binding_labels(&labels) {
                bindings.insert(meta.action, label);
            }
        }
        Self { bindings }
    }

    pub fn binding_for_action(&self, action: Action) -> Option<&str> {
        self.bindings.get(&action).map(String::as_str)
    }

    pub fn apply_preset(&self, slot: usize) -> Option<&str> {
        action_for_apply_preset(slot).and_then(|action| self.binding_for_action(action))
    }

    pub fn save_preset(&self, slot: usize) -> Option<&str> {
        action_for_save_preset(slot).and_then(|action| self.binding_for_action(action))
    }

    pub fn clear_preset(&self, slot: usize) -> Option<&str> {
        action_for_clear_preset(slot).and_then(|action| self.binding_for_action(action))
    }

    pub fn binding_for_event(&self, event: &ToolbarEvent) -> Option<&str> {
        action_for_event(event).and_then(|action| self.binding_for_action(action))
    }
}

pub(crate) fn action_for_tool(tool: Tool) -> Option<Action> {
    match tool {
        Tool::Select => Some(Action::SelectSelectionTool),
        Tool::Pen => Some(Action::SelectPenTool),
        Tool::Line => Some(Action::SelectLineTool),
        Tool::Rect => Some(Action::SelectRectTool),
        Tool::Ellipse => Some(Action::SelectEllipseTool),
        Tool::Arrow => Some(Action::SelectArrowTool),
        Tool::Marker => Some(Action::SelectMarkerTool),
        Tool::Highlight => Some(Action::SelectHighlightTool),
        Tool::Eraser => Some(Action::SelectEraserTool),
    }
}

#[allow(dead_code)]
pub(crate) fn tool_label(tool: Tool) -> &'static str {
    action_for_tool(tool)
        .map(action_short_label)
        .unwrap_or("Select")
}

#[allow(dead_code)]
pub(crate) fn tool_tooltip_label(tool: Tool) -> &'static str {
    action_for_tool(tool).map(action_label).unwrap_or("Select")
}

pub(crate) fn action_for_event(event: &ToolbarEvent) -> Option<Action> {
    match event {
        ToolbarEvent::SelectTool(tool) => action_for_tool(*tool),
        ToolbarEvent::EnterTextMode => Some(Action::EnterTextMode),
        ToolbarEvent::EnterStickyNoteMode => Some(Action::EnterStickyNoteMode),
        ToolbarEvent::ToggleFill(_) => Some(Action::ToggleFill),
        ToolbarEvent::Undo => Some(Action::Undo),
        ToolbarEvent::Redo => Some(Action::Redo),
        ToolbarEvent::UndoAll => Some(Action::UndoAll),
        ToolbarEvent::RedoAll => Some(Action::RedoAll),
        ToolbarEvent::UndoAllDelayed => Some(Action::UndoAllDelayed),
        ToolbarEvent::RedoAllDelayed => Some(Action::RedoAllDelayed),
        ToolbarEvent::ClearCanvas => Some(Action::ClearCanvas),
        ToolbarEvent::PagePrev => Some(Action::PagePrev),
        ToolbarEvent::PageNext => Some(Action::PageNext),
        ToolbarEvent::PageNew => Some(Action::PageNew),
        ToolbarEvent::PageDuplicate => Some(Action::PageDuplicate),
        ToolbarEvent::PageDelete => Some(Action::PageDelete),
        ToolbarEvent::BoardPrev => Some(Action::BoardPrev),
        ToolbarEvent::BoardNext => Some(Action::BoardNext),
        ToolbarEvent::BoardNew => Some(Action::BoardNew),
        ToolbarEvent::BoardDelete => Some(Action::BoardDelete),
        ToolbarEvent::BoardDuplicate => Some(Action::BoardDuplicate),
        ToolbarEvent::BoardRename => Some(Action::BoardPicker),
        ToolbarEvent::ToggleBoardPicker => Some(Action::BoardPicker),
        ToolbarEvent::ToggleAllHighlight(_) => Some(Action::ToggleHighlightTool),
        ToolbarEvent::ToggleFreeze => Some(Action::ToggleFrozenMode),
        ToolbarEvent::ZoomIn => Some(Action::ZoomIn),
        ToolbarEvent::ZoomOut => Some(Action::ZoomOut),
        ToolbarEvent::ResetZoom => Some(Action::ResetZoom),
        ToolbarEvent::ToggleZoomLock => Some(Action::ToggleZoomLock),
        ToolbarEvent::ApplyPreset(slot) => action_for_apply_preset(*slot),
        ToolbarEvent::SavePreset(slot) => action_for_save_preset(*slot),
        ToolbarEvent::ClearPreset(slot) => action_for_clear_preset(*slot),
        ToolbarEvent::OpenConfigurator => Some(Action::OpenConfigurator),
        _ => None,
    }
}

pub(crate) fn action_for_apply_preset(slot: usize) -> Option<Action> {
    match slot {
        1 => Some(Action::ApplyPreset1),
        2 => Some(Action::ApplyPreset2),
        3 => Some(Action::ApplyPreset3),
        4 => Some(Action::ApplyPreset4),
        5 => Some(Action::ApplyPreset5),
        _ => None,
    }
}

pub(crate) fn action_for_save_preset(slot: usize) -> Option<Action> {
    match slot {
        1 => Some(Action::SavePreset1),
        2 => Some(Action::SavePreset2),
        3 => Some(Action::SavePreset3),
        4 => Some(Action::SavePreset4),
        5 => Some(Action::SavePreset5),
        _ => None,
    }
}

pub(crate) fn action_for_clear_preset(slot: usize) -> Option<Action> {
    match slot {
        1 => Some(Action::ClearPreset1),
        2 => Some(Action::ClearPreset2),
        3 => Some(Action::ClearPreset3),
        4 => Some(Action::ClearPreset4),
        5 => Some(Action::ClearPreset5),
        _ => None,
    }
}
