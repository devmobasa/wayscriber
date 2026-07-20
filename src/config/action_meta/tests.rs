use super::*;
use std::collections::HashSet;

const HELP_ACTIONS: &[Action] = &[
    Action::ToggleWhiteboard,
    Action::ToggleBlackboard,
    Action::ReturnToTransparent,
    Action::Board1,
    Action::Board2,
    Action::Board3,
    Action::Board4,
    Action::Board5,
    Action::Board6,
    Action::Board7,
    Action::Board8,
    Action::Board9,
    Action::BoardPrev,
    Action::BoardNext,
    Action::FocusNextOutput,
    Action::FocusPrevOutput,
    Action::BoardNew,
    Action::BoardDelete,
    Action::BoardPicker,
    Action::PagePrev,
    Action::PageNext,
    Action::PageNew,
    Action::PageDuplicate,
    Action::PageDelete,
    Action::PageRestoreDeleted,
    Action::BoardPrev,
    Action::BoardNext,
    Action::BoardNew,
    Action::BoardDelete,
    Action::SetColorRed,
    Action::SetColorGreen,
    Action::SetColorBlue,
    Action::SetColorYellow,
    Action::SetColorOrange,
    Action::SetColorPink,
    Action::SetColorWhite,
    Action::SetColorBlack,
    Action::SelectSelectionTool,
    Action::SelectPenTool,
    Action::SelectLineTool,
    Action::SelectRectTool,
    Action::SelectEllipseTool,
    Action::SelectArrowTool,
    Action::SelectBlurTool,
    Action::ToggleHighlightTool,
    Action::SelectMarkerTool,
    Action::SelectStepMarkerTool,
    Action::SelectEraserTool,
    Action::IncreaseThickness,
    Action::DecreaseThickness,
    Action::SelectAll,
    Action::DuplicateSelection,
    Action::CopySelection,
    Action::PasteSelection,
    Action::DeleteSelection,
    Action::ToggleSelectionProperties,
    Action::IncreaseFontSize,
    Action::DecreaseFontSize,
    Action::EnterTextMode,
    Action::EnterStickyNoteMode,
    Action::ToggleFill,
    Action::ZoomIn,
    Action::ZoomOut,
    Action::ResetZoom,
    Action::ToggleZoomLock,
    Action::RefreshZoomCapture,
    Action::ClearCanvas,
    Action::Undo,
    Action::ToggleClickHighlight,
    Action::OpenContextMenu,
    Action::Exit,
    Action::ToggleHelp,
    Action::ToggleToolbar,
    Action::TogglePresenterMode,
    Action::OpenConfigurator,
    Action::ClearSavedToolState,
    Action::ToggleStatusBar,
    Action::ToggleFrozenMode,
    Action::CaptureClipboardFull,
    Action::CaptureFileFull,
    Action::ExportCanvasFile,
    Action::ExportCanvasClipboard,
    Action::ExportCanvasClipboardAndFile,
    Action::ExportBoardPdfFile,
    Action::ExportAllBoardsPdfFile,
    Action::CaptureClipboardSelection,
    Action::CaptureFileSelection,
    Action::CaptureActiveWindow,
    Action::CaptureSelection,
    Action::OpenCaptureFolder,
];

const TOOLBAR_ACTIONS: &[Action] = &[
    Action::SelectPenTool,
    Action::SelectLineTool,
    Action::SelectRectTool,
    Action::SelectEllipseTool,
    Action::SelectArrowTool,
    Action::SelectBlurTool,
    Action::SelectSelectionTool,
    Action::SelectMarkerTool,
    Action::SelectStepMarkerTool,
    Action::SelectHighlightTool,
    Action::SelectEraserTool,
    Action::EnterTextMode,
    Action::EnterStickyNoteMode,
    Action::CaptureSelection,
    Action::ToggleFill,
    Action::Undo,
    Action::Redo,
    Action::UndoAll,
    Action::RedoAll,
    Action::UndoAllDelayed,
    Action::RedoAllDelayed,
    Action::ClearCanvas,
    Action::PagePrev,
    Action::PageNext,
    Action::PageNew,
    Action::PageDuplicate,
    Action::PageDelete,
    Action::ToggleHighlightTool,
    Action::ToggleFrozenMode,
    Action::ZoomIn,
    Action::ZoomOut,
    Action::ResetZoom,
    Action::ToggleZoomLock,
    Action::ApplyPreset1,
    Action::ApplyPreset2,
    Action::ApplyPreset3,
    Action::ApplyPreset4,
    Action::ApplyPreset5,
    Action::SavePreset1,
    Action::SavePreset2,
    Action::SavePreset3,
    Action::SavePreset4,
    Action::SavePreset5,
    Action::ClearPreset1,
    Action::ClearPreset2,
    Action::ClearPreset3,
    Action::ClearPreset4,
    Action::ClearPreset5,
    Action::OpenConfigurator,
    Action::ToggleCommandPalette,
];

const EXPECTED_COMMAND_PALETTE_ACTIONS: &[Action] = &[
    Action::Exit,
    Action::EnterTextMode,
    Action::EnterStickyNoteMode,
    Action::ClearCanvas,
    Action::Undo,
    Action::Redo,
    Action::SelectSelectionTool,
    Action::SelectPenTool,
    Action::SelectLineTool,
    Action::SelectRectTool,
    Action::SelectEllipseTool,
    Action::SelectTriangleTool,
    Action::SelectParallelogramTool,
    Action::SelectRhombusTool,
    Action::SelectRegularPolygonTool,
    Action::SelectFreeformPolygonTool,
    Action::SelectArrowTool,
    Action::SelectBlurTool,
    Action::SelectHighlightTool,
    Action::SelectMarkerTool,
    Action::SelectStepMarkerTool,
    Action::SelectEraserTool,
    Action::ToggleEraserMode,
    Action::IncreaseThickness,
    Action::DecreaseThickness,
    Action::IncreaseMarkerOpacity,
    Action::DecreaseMarkerOpacity,
    Action::IncreaseFontSize,
    Action::DecreaseFontSize,
    Action::ResetArrowLabelCounter,
    Action::ResetStepMarkerCounter,
    Action::ToggleFill,
    Action::ToggleWhiteboard,
    Action::ToggleBlackboard,
    Action::ReturnToTransparent,
    Action::PagePrev,
    Action::PageNext,
    Action::PageNew,
    Action::PageDuplicate,
    Action::PageDelete,
    Action::PageRestoreDeleted,
    Action::Board1,
    Action::Board2,
    Action::Board3,
    Action::Board4,
    Action::Board5,
    Action::Board6,
    Action::Board7,
    Action::Board8,
    Action::Board9,
    Action::BoardNext,
    Action::BoardPrev,
    Action::FocusNextOutput,
    Action::FocusPrevOutput,
    Action::BoardNew,
    Action::BoardDelete,
    Action::BoardPicker,
    Action::BoardRestoreDeleted,
    Action::BoardDuplicate,
    Action::BoardSwitchRecent,
    Action::ToggleHelp,
    Action::ToggleQuickHelp,
    Action::ToggleToolbar,
    Action::CycleToolbarDisplay,
    Action::ToggleStatusBar,
    Action::TogglePresenterMode,
    Action::ToggleLightMode,
    Action::ToggleLightModeDrawing,
    Action::RenderProfileNext,
    Action::RenderProfilePrevious,
    Action::RenderProfileOff,
    Action::ToggleClickHighlight,
    Action::ToggleRadialMenu,
    Action::ToggleSelectionProperties,
    Action::OpenContextMenu,
    Action::OpenConfigurator,
    Action::ClearSavedToolState,
    Action::ToggleCommandPalette,
    Action::ReplayTour,
    Action::SetColorRed,
    Action::SetColorGreen,
    Action::SetColorBlue,
    Action::SetColorYellow,
    Action::SetColorOrange,
    Action::SetColorPink,
    Action::SetColorWhite,
    Action::SetColorBlack,
    Action::PickScreenColor,
    Action::CaptureClipboardFull,
    Action::CaptureFileFull,
    Action::ExportCanvasFile,
    Action::ExportCanvasClipboard,
    Action::ExportCanvasClipboardAndFile,
    Action::ExportBoardPdfFile,
    Action::ExportAllBoardsPdfFile,
    Action::OpenCaptureFolder,
    Action::ToggleFrozenMode,
    Action::ZoomIn,
    Action::ZoomOut,
    Action::ResetZoom,
    Action::ToggleZoomLock,
    Action::RefreshZoomCapture,
    Action::SelectAll,
    Action::DeleteSelection,
    Action::DuplicateSelection,
    Action::CopySelection,
    Action::PasteSelection,
    Action::ApplyPreset1,
    Action::ApplyPreset2,
    Action::ApplyPreset3,
    Action::ApplyPreset4,
    Action::ApplyPreset5,
];

fn assert_actions_have_flag(
    actions: &[Action],
    flag_name: &str,
    check: impl Fn(&ActionMeta) -> bool,
) {
    let mut seen = HashSet::new();
    for action in actions {
        if !seen.insert(*action) {
            continue;
        }
        let meta =
            action_meta(*action).unwrap_or_else(|| panic!("missing ActionMeta for {:?}", action));
        assert!(check(meta), "Action {:?} missing {}", action, flag_name);
    }
}

#[test]
fn action_meta_entries_are_unique() {
    let mut seen = HashSet::new();

    for meta in action_meta_iter() {
        assert!(
            seen.insert(meta.action),
            "duplicate ActionMeta for {:?}",
            meta.action
        );
    }
}

#[test]
fn action_meta_covers_surface_actions() {
    assert_actions_have_flag(HELP_ACTIONS, "in_help", |meta| meta.in_help);
    assert_actions_have_flag(TOOLBAR_ACTIONS, "in_toolbar", |meta| meta.in_toolbar);
    assert_actions_have_flag(
        EXPECTED_COMMAND_PALETTE_ACTIONS,
        "in_command_palette",
        |meta| meta.in_command_palette,
    );
}

#[test]
fn command_palette_actions_match_expected_contract() {
    let actual: HashSet<Action> = action_meta_iter()
        .filter(|meta| meta.in_command_palette)
        .map(|meta| meta.action)
        .collect();
    let expected: HashSet<Action> = EXPECTED_COMMAND_PALETTE_ACTIONS.iter().copied().collect();
    let unexpected: HashSet<Action> = actual.difference(&expected).copied().collect();
    let missing: HashSet<Action> = expected.difference(&actual).copied().collect();

    assert!(
        unexpected.is_empty() && missing.is_empty(),
        "command palette contract changed; unexpected: {:?}; missing: {:?}",
        unexpected,
        missing
    );
}

/// The actions the radial compass renders as icons: every tool/mode wedge
/// and sub-ring child. Everything else stays icon-free until an icon
/// surface needs it.
const EXPECTED_ICON_ACTIONS: &[Action] = &[
    Action::SelectPenTool,
    Action::SelectMarkerTool,
    Action::SelectLineTool,
    Action::SelectRectTool,
    Action::SelectEllipseTool,
    Action::SelectRegularPolygonTool,
    Action::SelectBlurTool,
    Action::SelectArrowTool,
    Action::SelectSelectionTool,
    Action::SelectEraserTool,
    Action::SelectStepMarkerTool,
    Action::EnterTextMode,
    Action::EnterStickyNoteMode,
];

#[test]
fn icon_coverage_matches_the_radial_contract() {
    for meta in action_meta_iter() {
        let expected = EXPECTED_ICON_ACTIONS.contains(&meta.action);
        assert_eq!(
            meta.icon.is_some(),
            expected,
            "icon presence contract changed for {:?}",
            meta.action
        );
    }
}

#[test]
fn icons_reuse_the_shared_semantic_tool_painters() {
    use crate::input::Tool;
    use crate::toolbar_icons::top_toolbar_icon_painter;
    use crate::ui::toolbar::model::{TopToolbarIcon, semantic_icon_for_tool};

    for action in EXPECTED_ICON_ACTIONS {
        let icon = action_meta(*action)
            .and_then(|meta| meta.icon)
            .unwrap_or_else(|| panic!("missing icon for {:?}", action));
        let expected = match action {
            Action::EnterTextMode => top_toolbar_icon_painter(TopToolbarIcon::Text),
            Action::EnterStickyNoteMode => top_toolbar_icon_painter(TopToolbarIcon::StickyNote),
            _ => {
                let tool = Tool::from_select_action(*action)
                    .unwrap_or_else(|| panic!("{:?} should select a tool", action));
                top_toolbar_icon_painter(TopToolbarIcon::Tool(semantic_icon_for_tool(tool)))
            }
        };
        assert!(
            std::ptr::fn_addr_eq(icon, expected),
            "icon painter for {:?} drifted from the shared semantic painter",
            action
        );
    }
}

#[test]
fn meta_macro_arms_default_aliases_and_icon() {
    const PLAIN: ActionMeta = meta!(Undo, "Undo", None, "d", History, false, false, false);
    const ALIASED: ActionMeta = meta!(
        Undo,
        "Undo",
        None,
        "d",
        History,
        false,
        false,
        false,
        &["back"]
    );
    const WITH_ICON: ActionMeta = meta!(
        Undo,
        "Undo",
        None,
        "d",
        History,
        false,
        false,
        false,
        icon: crate::toolbar_icons::draw_icon_undo
    );
    const FULL: ActionMeta = meta!(
        Undo,
        "Undo",
        None,
        "d",
        History,
        false,
        false,
        false,
        &["back"],
        icon: crate::toolbar_icons::draw_icon_undo
    );

    assert!(PLAIN.icon.is_none() && PLAIN.search_aliases.is_empty());
    assert!(ALIASED.icon.is_none() && ALIASED.search_aliases == ["back"]);
    assert!(WITH_ICON.icon.is_some() && WITH_ICON.search_aliases.is_empty());
    assert!(FULL.icon.is_some() && FULL.search_aliases == ["back"]);
}

#[test]
fn action_display_label_strips_toggle_prefix() {
    assert_eq!(action_display_label(Action::ToggleStatusBar), "Status Bar");
}

#[test]
fn action_display_label_strips_mode_suffix() {
    assert_eq!(action_display_label(Action::ToggleWhiteboard), "Whiteboard");
}

#[test]
fn action_display_label_uses_short_label_for_ellipse_tool() {
    assert_eq!(action_display_label(Action::SelectEllipseTool), "Circle");
}
