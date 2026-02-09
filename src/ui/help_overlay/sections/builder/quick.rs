use crate::config::{Action, action_label};
use crate::label_format::NOT_BOUND_LABEL;
use crate::toolbar_icons;

use super::super::super::types::{Section, row};
use super::super::bindings::{HelpOverlayBindings, binding_or_fallback, bindings_or_fallback};

pub(super) fn build_quick_sections(bindings: &HelpOverlayBindings) -> Vec<Section> {
    let quick_drawing = Section {
        title: "Drawing",
        rows: vec![
            row(
                binding_or_fallback(bindings, Action::SelectPenTool, NOT_BOUND_LABEL),
                action_label(Action::SelectPenTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectEraserTool, NOT_BOUND_LABEL),
                action_label(Action::SelectEraserTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectLineTool, "Shift+Drag"),
                action_label(Action::SelectLineTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectRectTool, "Ctrl+Drag"),
                action_label(Action::SelectRectTool),
            ),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_pen),
    };
    let quick_actions = Section {
        title: "Actions",
        rows: vec![
            row(
                binding_or_fallback(bindings, Action::Undo, NOT_BOUND_LABEL),
                action_label(Action::Undo),
            ),
            row(
                binding_or_fallback(bindings, Action::ClearCanvas, NOT_BOUND_LABEL),
                action_label(Action::ClearCanvas),
            ),
            row(
                binding_or_fallback(bindings, Action::EnterTextMode, NOT_BOUND_LABEL),
                action_label(Action::EnterTextMode),
            ),
            row(
                binding_or_fallback(bindings, Action::Exit, NOT_BOUND_LABEL),
                action_label(Action::Exit),
            ),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_undo),
    };
    let quick_navigation = Section {
        title: "Navigation",
        rows: vec![
            row(
                bindings_or_fallback(
                    bindings,
                    &[Action::BoardPrev, Action::BoardNext],
                    NOT_BOUND_LABEL,
                ),
                "Previous/next board",
            ),
            row(
                bindings_or_fallback(
                    bindings,
                    &[Action::PagePrev, Action::PageNext],
                    NOT_BOUND_LABEL,
                ),
                "Previous/next page",
            ),
            row(
                binding_or_fallback(bindings, Action::ToggleHelp, NOT_BOUND_LABEL),
                "Full help",
            ),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_file),
    };

    vec![quick_drawing, quick_actions, quick_navigation]
}
