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
        Tool::StepMarker => Some(Action::SelectStepMarkerTool),
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
        ToolbarEvent::ResetStepMarkerCounter => Some(Action::ResetStepMarkerCounter),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");
        let action_bindings = keybindings
            .build_action_bindings()
            .expect("default keybindings bindings");

        let mut state = InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        );
        state.set_action_bindings(action_bindings);
        state
    }

    #[test]
    fn action_for_tool_maps_selection_and_eraser_tools() {
        assert_eq!(action_for_tool(Tool::Select), Some(Action::SelectSelectionTool));
        assert_eq!(action_for_tool(Tool::Eraser), Some(Action::SelectEraserTool));
    }

    #[test]
    fn action_for_event_maps_board_picker_related_events() {
        assert_eq!(action_for_event(&ToolbarEvent::BoardRename), Some(Action::BoardPicker));
        assert_eq!(
            action_for_event(&ToolbarEvent::ToggleBoardPicker),
            Some(Action::BoardPicker)
        );
    }

    #[test]
    fn action_for_event_returns_none_for_layout_only_events() {
        assert_eq!(action_for_event(&ToolbarEvent::OpenConfigFile), None);
        assert_eq!(action_for_event(&ToolbarEvent::ToggleShapePicker(true)), None);
    }

    #[test]
    fn preset_action_helpers_cover_valid_slots_only() {
        assert_eq!(action_for_apply_preset(1), Some(Action::ApplyPreset1));
        assert_eq!(action_for_save_preset(5), Some(Action::SavePreset5));
        assert_eq!(action_for_clear_preset(3), Some(Action::ClearPreset3));
        assert_eq!(action_for_apply_preset(0), None);
        assert_eq!(action_for_save_preset(6), None);
        assert_eq!(action_for_clear_preset(99), None);
    }

    #[test]
    fn toolbar_binding_hints_collect_only_toolbar_actions() {
        let state = make_state();
        let hints = ToolbarBindingHints::from_input_state(&state);

        assert_eq!(hints.binding_for_action(Action::OpenConfigurator), Some("F11"));
        assert_eq!(hints.binding_for_action(Action::Exit), None);
    }

    #[test]
    fn toolbar_binding_hints_follow_event_mapping() {
        let state = make_state();
        let hints = ToolbarBindingHints::from_input_state(&state);

        assert_eq!(
            hints.binding_for_event(&ToolbarEvent::OpenConfigurator),
            Some("F11")
        );
        assert_eq!(hints.binding_for_event(&ToolbarEvent::OpenConfigFile), None);
    }

    #[test]
    fn toolbar_binding_hints_resolve_tool_bindings() {
        let state = make_state();
        let hints = ToolbarBindingHints::from_input_state(&state);

        assert_eq!(hints.for_tool(Tool::Pen), Some("F"));
        assert_eq!(hints.for_tool(Tool::Eraser), Some("D"));
        assert_eq!(hints.for_tool(Tool::StepMarker), None);
    }

    #[test]
    fn toolbar_binding_hints_resolve_preset_bindings() {
        let state = make_state();
        let hints = ToolbarBindingHints::from_input_state(&state);

        assert_eq!(hints.apply_preset(1), Some("1"));
        assert_eq!(hints.save_preset(1), Some("Shift+1"));
        assert_eq!(hints.clear_preset(1), Some("Ctrl+1"));
        assert_eq!(hints.apply_preset(6), None);
    }

    #[test]
    fn tool_label_and_tooltip_label_use_action_metadata() {
        assert_eq!(tool_label(Tool::Ellipse), "Circle");
        assert_eq!(tool_tooltip_label(Tool::Ellipse), "Ellipse Tool");
        assert_eq!(tool_label(Tool::Select), "Select");
    }

    #[test]
    fn action_for_event_maps_select_tool_and_freeze_events() {
        assert_eq!(
            action_for_event(&ToolbarEvent::SelectTool(Tool::Pen)),
            Some(Action::SelectPenTool)
        );
        assert_eq!(action_for_event(&ToolbarEvent::ToggleFreeze), Some(Action::ToggleFrozenMode));
    }
}
