use std::collections::HashMap;

use crate::config::{Action, action_meta_iter};
use crate::input::{InputState, Tool};
use crate::label_format::join_binding_labels;

use super::events::{
    ToolbarEvent, action_for_apply_preset as event_action_for_apply_preset,
    action_for_clear_preset as event_action_for_clear_preset,
    action_for_save_preset as event_action_for_save_preset,
};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ToolbarBindingHints {
    bindings: HashMap<Action, String>,
    badges: HashMap<Action, String>,
}

impl ToolbarBindingHints {
    pub fn for_tool(&self, tool: Tool) -> Option<&str> {
        action_for_tool(tool).and_then(|action| self.binding_for_action(action))
    }

    pub fn badge_for_tool(&self, tool: Tool) -> Option<&str> {
        action_for_tool(tool).and_then(|action| self.badge_for_action(action))
    }

    pub fn from_input_state(state: &InputState) -> Self {
        let mut bindings = HashMap::new();
        let mut badges = HashMap::new();
        for meta in action_meta_iter().filter(|meta| meta.in_toolbar) {
            let labels = state.action_binding_labels(meta.action);
            if let Some(label) = join_binding_labels(&labels) {
                bindings.insert(meta.action, label);
            }
            if let Some(label) = labels.first().and_then(|label| compact_badge_label(label)) {
                badges.insert(meta.action, label);
            }
        }
        Self { bindings, badges }
    }

    pub fn binding_for_action(&self, action: Action) -> Option<&str> {
        self.bindings.get(&action).map(String::as_str)
    }

    pub fn badge_for_action(&self, action: Action) -> Option<&str> {
        self.badges.get(&action).map(String::as_str)
    }

    pub fn quick_color_badge(&self, index: usize) -> Option<&str> {
        crate::config::QuickColorPalette::action_for_index(index)
            .and_then(|action| self.badge_for_action(action))
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

    pub fn badge_for_event(&self, event: &ToolbarEvent) -> Option<&str> {
        action_for_event(event).and_then(|action| self.badge_for_action(action))
    }
}

/// A visible toolbar badge must fit inside the existing fixed-size control.
/// Modifier chords and multi-binding lists remain available in the tooltip;
/// abbreviating them here would make the displayed shortcut misleading.
fn compact_badge_label(label: &str) -> Option<String> {
    let trimmed = label.trim();
    let compact = trimmed.chars().count() <= 3
        && !trimmed.is_empty()
        && (!trimmed.contains('+') || trimmed == "+");
    compact.then(|| trimmed.to_string())
}

pub(crate) fn action_for_tool(tool: Tool) -> Option<Action> {
    tool.action()
}

// Consumed by the bin's toolbar render layer; the lib target builds this
// module without the backend, so the reference is target-dependent.
#[allow(dead_code)]
pub(crate) fn tool_label(tool: Tool) -> &'static str {
    tool.short_label()
}

#[allow(dead_code)]
pub(crate) fn tool_tooltip_label(tool: Tool) -> &'static str {
    tool.display_label()
}

pub(crate) fn action_for_event(event: &ToolbarEvent) -> Option<Action> {
    event.action()
}

pub(crate) fn action_for_apply_preset(slot: usize) -> Option<Action> {
    event_action_for_apply_preset(slot)
}

pub(crate) fn action_for_save_preset(slot: usize) -> Option<Action> {
    event_action_for_save_preset(slot)
}

pub(crate) fn action_for_clear_preset(slot: usize) -> Option<Action> {
    event_action_for_clear_preset(slot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeyBinding;
    use crate::input::state::test_support::make_test_input_state_with_action_bindings;

    fn binding_map(entries: &[(Action, &[&str])]) -> HashMap<Action, Vec<KeyBinding>> {
        entries
            .iter()
            .map(|(action, values)| {
                (
                    *action,
                    values
                        .iter()
                        .map(|value| KeyBinding::parse(value).expect("binding"))
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn action_for_tool_maps_selection_and_eraser_tools() {
        assert_eq!(
            action_for_tool(Tool::Select),
            Some(Action::SelectSelectionTool)
        );
        assert_eq!(
            action_for_tool(Tool::Eraser),
            Some(Action::SelectEraserTool)
        );
    }

    #[test]
    fn action_for_event_maps_board_picker_related_events() {
        assert_eq!(
            action_for_event(&ToolbarEvent::BoardRename),
            Some(Action::BoardPicker)
        );
        assert_eq!(
            action_for_event(&ToolbarEvent::ToggleBoardPicker),
            Some(Action::BoardPicker)
        );
    }

    #[test]
    fn action_for_event_maps_screen_eyedropper() {
        assert_eq!(
            action_for_event(&ToolbarEvent::PickScreenColor),
            Some(Action::PickScreenColor)
        );
    }

    #[test]
    fn action_for_event_returns_none_for_layout_only_events() {
        assert_eq!(action_for_event(&ToolbarEvent::OpenConfigFile), None);
        assert_eq!(
            action_for_event(&ToolbarEvent::ToggleShapePicker(true)),
            None
        );
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
        let state = make_test_input_state_with_action_bindings(binding_map(&[
            (Action::OpenConfigurator, &["Ctrl+Alt+Shift+O"]),
            (Action::Exit, &["Ctrl+Alt+Shift+Q"]),
        ]));
        let hints = ToolbarBindingHints::from_input_state(&state);
        let expected = KeyBinding::parse("Ctrl+Alt+Shift+O").unwrap().to_string();

        assert_eq!(
            hints.binding_for_action(Action::OpenConfigurator),
            Some(expected.as_str())
        );
        assert_eq!(hints.binding_for_action(Action::Exit), None);
    }

    #[test]
    fn toolbar_binding_hints_follow_event_mapping() {
        let state = make_test_input_state_with_action_bindings(binding_map(&[(
            Action::OpenConfigurator,
            &["Alt+P"],
        )]));
        let hints = ToolbarBindingHints::from_input_state(&state);

        assert_eq!(
            hints.binding_for_event(&ToolbarEvent::OpenConfigurator),
            Some("Alt+P")
        );
        assert_eq!(hints.binding_for_event(&ToolbarEvent::OpenConfigFile), None);
    }

    #[test]
    fn toolbar_binding_hints_resolve_tool_bindings() {
        let state = make_test_input_state_with_action_bindings(binding_map(&[
            (Action::SelectPenTool, &["Ctrl+P"]),
            (Action::SelectEraserTool, &["Ctrl+E"]),
        ]));
        let hints = ToolbarBindingHints::from_input_state(&state);

        assert_eq!(hints.for_tool(Tool::Pen), Some("Ctrl+P"));
        assert_eq!(hints.for_tool(Tool::Eraser), Some("Ctrl+E"));
        assert_eq!(hints.for_tool(Tool::StepMarker), None);
        assert_eq!(hints.badge_for_tool(Tool::Pen), None);
    }

    #[test]
    fn toolbar_binding_badges_use_current_compact_primary_binding() {
        let hints = ToolbarBindingHints::from_input_state(
            &make_test_input_state_with_action_bindings(binding_map(&[
                (Action::SelectPenTool, &["9", "Ctrl+P"]),
                (Action::SelectEraserTool, &["Ctrl+E"]),
                (Action::SetColorRed, &["F1"]),
                (Action::CaptureSelection, &["S"]),
            ])),
        );

        assert_eq!(hints.badge_for_tool(Tool::Pen), Some("9"));
        assert_eq!(hints.badge_for_tool(Tool::Eraser), None);
        assert_eq!(hints.quick_color_badge(0), Some("F1"));
        assert_eq!(hints.quick_color_badge(8), None);
        assert_eq!(hints.badge_for_action(Action::CaptureSelection), Some("S"));
    }

    #[test]
    fn compact_badge_label_rejects_chords_and_accepts_short_keys() {
        assert_eq!(compact_badge_label("R").as_deref(), Some("R"));
        assert_eq!(compact_badge_label("F12").as_deref(), Some("F12"));
        assert_eq!(compact_badge_label("+").as_deref(), Some("+"));
        assert_eq!(compact_badge_label("Ctrl+R"), None);
        assert_eq!(compact_badge_label("Space"), None);
    }

    #[test]
    fn toolbar_binding_hints_resolve_preset_bindings() {
        let state = make_test_input_state_with_action_bindings(binding_map(&[
            (Action::ApplyPreset1, &["Alt+1"]),
            (Action::SavePreset1, &["Alt+2"]),
            (Action::ClearPreset1, &["Alt+3"]),
        ]));
        let hints = ToolbarBindingHints::from_input_state(&state);

        assert_eq!(hints.apply_preset(1), Some("Alt+1"));
        assert_eq!(hints.save_preset(1), Some("Alt+2"));
        assert_eq!(hints.clear_preset(1), Some("Alt+3"));
        assert_eq!(hints.apply_preset(6), None);
    }

    #[test]
    fn tool_label_and_tooltip_label_use_action_metadata() {
        use crate::config::{action_label, action_short_label};

        assert_eq!(
            tool_label(Tool::Ellipse),
            action_short_label(Action::SelectEllipseTool)
        );
        assert_eq!(
            tool_tooltip_label(Tool::Ellipse),
            action_label(Action::SelectEllipseTool)
        );
        assert_eq!(
            tool_label(Tool::Select),
            action_short_label(Action::SelectSelectionTool)
        );
    }

    #[test]
    fn action_for_event_maps_select_tool_and_freeze_events() {
        assert_eq!(
            action_for_event(&ToolbarEvent::SelectTool(Tool::Pen)),
            Some(Action::SelectPenTool)
        );
        assert_eq!(
            action_for_event(&ToolbarEvent::ToggleFreeze),
            Some(Action::ToggleFrozenMode)
        );
    }
}
