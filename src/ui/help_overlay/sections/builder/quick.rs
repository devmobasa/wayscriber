use crate::config::Action;
use crate::label_format::NOT_BOUND_LABEL;
use crate::toolbar_icons;

use super::super::super::types::{Section, row};
use super::super::bindings::{
    HelpOverlayBindings, action_row, binding_or_fallback, bindings_or_fallback,
};

pub(super) fn build_quick_sections(bindings: &HelpOverlayBindings) -> Vec<Section> {
    let quick_drawing = Section {
        title: "Drawing",
        rows: vec![
            action_row(bindings, Action::SelectPenTool, NOT_BOUND_LABEL),
            action_row(bindings, Action::SelectEraserTool, NOT_BOUND_LABEL),
            action_row(bindings, Action::SelectLineTool, "Shift+Drag"),
            action_row(bindings, Action::SelectRectTool, "Ctrl+Drag"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_pen),
    };
    let quick_actions = Section {
        title: "Actions",
        rows: vec![
            action_row(bindings, Action::Undo, NOT_BOUND_LABEL),
            action_row(bindings, Action::ClearCanvas, NOT_BOUND_LABEL),
            action_row(bindings, Action::EnterTextMode, NOT_BOUND_LABEL),
            action_row(bindings, Action::Exit, NOT_BOUND_LABEL),
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
            )
            .with_action(Action::ToggleHelp),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_file),
    };

    vec![quick_drawing, quick_actions, quick_navigation]
}
