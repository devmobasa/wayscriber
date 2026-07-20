use crate::config::{Action, QuickColorSlot, action_label};
use crate::label_format::NOT_BOUND_LABEL;
use crate::toolbar_icons;

use super::super::super::types::{Badge, Section, row};
use super::super::bindings::{
    HelpOverlayBindings, action_row, binding_or_fallback, bindings_compact_or_fallback,
    bindings_or_fallback, joined_labels, primary_or_fallback,
};

pub(super) struct MainSections {
    pub(super) board_modes: Option<Section>,
    pub(super) pages: Section,
    pub(super) drawing: Section,
    pub(super) selection: Section,
    pub(super) pen_text: Section,
    pub(super) zoom: Section,
    pub(super) actions: Section,
    pub(super) screenshots: Option<Section>,
}

fn color_badge(bindings: &HelpOverlayBindings, index: usize) -> Option<Badge> {
    let slot = QuickColorSlot::from_index(index)?;
    Some(Badge {
        label: primary_or_fallback(bindings, slot.action(), slot.fallback_key()),
        color: bindings.quick_color_badge(index)?,
    })
}

pub(super) fn build_main_sections(
    bindings: &HelpOverlayBindings,
    frozen_enabled: bool,
    context_filter: bool,
    board_enabled: bool,
    capture_enabled: bool,
) -> MainSections {
    let board_modes = (!context_filter || board_enabled).then(|| Section {
        title: "Boards",
        rows: vec![
            action_row(bindings, Action::ToggleWhiteboard, NOT_BOUND_LABEL),
            action_row(bindings, Action::ToggleBlackboard, NOT_BOUND_LABEL),
            action_row(bindings, Action::ReturnToTransparent, NOT_BOUND_LABEL),
            row(
                bindings_compact_or_fallback(
                    bindings,
                    &[
                        Action::Board1,
                        Action::Board2,
                        Action::Board3,
                        Action::Board4,
                        Action::Board5,
                        Action::Board6,
                        Action::Board7,
                        Action::Board8,
                        Action::Board9,
                    ],
                    "Ctrl+Shift+1..9",
                ),
                "Switch board slot",
            ),
            row(
                bindings_or_fallback(
                    bindings,
                    &[Action::BoardPrev, Action::BoardNext],
                    "Ctrl+Shift+Left/Right",
                ),
                "Previous/next board",
            ),
            row(
                bindings_or_fallback(
                    bindings,
                    &[Action::FocusPrevOutput, Action::FocusNextOutput],
                    "Ctrl+Shift+Alt+Left/Right",
                ),
                "Previous/next output",
            ),
            action_row(bindings, Action::BoardNew, NOT_BOUND_LABEL),
            action_row(bindings, Action::BoardDelete, NOT_BOUND_LABEL),
            action_row(bindings, Action::BoardPicker, NOT_BOUND_LABEL),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_settings),
    });

    let pages = Section {
        title: "Pages",
        rows: vec![
            action_row(bindings, Action::PagePrev, NOT_BOUND_LABEL),
            action_row(bindings, Action::PageNext, NOT_BOUND_LABEL),
            action_row(bindings, Action::PageNew, NOT_BOUND_LABEL),
            action_row(bindings, Action::PageDuplicate, NOT_BOUND_LABEL),
            action_row(bindings, Action::PageDelete, NOT_BOUND_LABEL),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_file),
    };

    let color_badges: Vec<Badge> = (0..bindings.quick_color_count())
        .filter_map(|index| color_badge(bindings, index))
        .collect();

    let drawing = Section {
        title: "Drawing",
        rows: vec![
            action_row(bindings, Action::SelectPenTool, NOT_BOUND_LABEL),
            action_row(bindings, Action::SelectLineTool, "Shift+Drag"),
            action_row(bindings, Action::SelectRectTool, "Ctrl+Drag"),
            action_row(bindings, Action::SelectEllipseTool, "Tab+Drag"),
            action_row(bindings, Action::SelectArrowTool, "Ctrl+Shift+Drag"),
            action_row(bindings, Action::SelectBlurTool, NOT_BOUND_LABEL),
            action_row(bindings, Action::ToggleHighlightTool, NOT_BOUND_LABEL),
            action_row(bindings, Action::SelectMarkerTool, NOT_BOUND_LABEL),
            action_row(bindings, Action::SelectStepMarkerTool, NOT_BOUND_LABEL),
            action_row(bindings, Action::SelectEraserTool, NOT_BOUND_LABEL),
            row(
                bindings_or_fallback(
                    bindings,
                    &[Action::IncreaseThickness, Action::DecreaseThickness],
                    NOT_BOUND_LABEL,
                ),
                "Adjust thickness",
            ),
        ],
        badges: color_badges.clone(),
        icon: Some(toolbar_icons::draw_icon_pen),
    };

    let selection = Section {
        title: "Selection",
        rows: vec![
            action_row(bindings, Action::SelectSelectionTool, NOT_BOUND_LABEL),
            row("Drag", "Selection tool"),
            action_row(bindings, Action::SelectAll, NOT_BOUND_LABEL),
            action_row(bindings, Action::DuplicateSelection, NOT_BOUND_LABEL),
            action_row(bindings, Action::CopySelection, NOT_BOUND_LABEL),
            action_row(bindings, Action::PasteSelection, NOT_BOUND_LABEL),
            action_row(bindings, Action::DeleteSelection, NOT_BOUND_LABEL),
            action_row(bindings, Action::ToggleSelectionProperties, NOT_BOUND_LABEL),
            action_row(bindings, Action::IncreaseFontSize, NOT_BOUND_LABEL),
            action_row(bindings, Action::DecreaseFontSize, NOT_BOUND_LABEL),
        ],
        badges: color_badges.clone(),
        icon: Some(toolbar_icons::draw_icon_select),
    };

    let pen_text = Section {
        title: "Pen & Text",
        rows: vec![
            action_row(bindings, Action::EnterTextMode, NOT_BOUND_LABEL),
            action_row(bindings, Action::EnterStickyNoteMode, NOT_BOUND_LABEL),
            action_row(bindings, Action::IncreaseFontSize, NOT_BOUND_LABEL),
            action_row(bindings, Action::DecreaseFontSize, NOT_BOUND_LABEL),
            action_row(bindings, Action::ToggleFill, NOT_BOUND_LABEL),
            row("Selection properties panel", "Text background"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_text),
    };

    let zoom = Section {
        title: "Zoom",
        rows: vec![
            action_row(bindings, Action::ZoomIn, NOT_BOUND_LABEL),
            action_row(bindings, Action::ZoomOut, NOT_BOUND_LABEL),
            action_row(bindings, Action::ResetZoom, NOT_BOUND_LABEL),
            action_row(bindings, Action::ToggleZoomLock, NOT_BOUND_LABEL),
            row("Middle drag / arrow keys", "Pan view"),
            action_row(bindings, Action::RefreshZoomCapture, NOT_BOUND_LABEL),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_zoom_in),
    };

    let mut action_rows = vec![
        action_row(bindings, Action::ClearCanvas, NOT_BOUND_LABEL),
        action_row(bindings, Action::Undo, NOT_BOUND_LABEL),
        action_row(bindings, Action::ToggleClickHighlight, NOT_BOUND_LABEL),
        row(
            match joined_labels(bindings, &[Action::OpenContextMenu]) {
                Some(label) => format!("Right Click / {label}"),
                None => "Right Click".to_string(),
            },
            action_label(Action::OpenContextMenu),
        )
        .with_action(Action::OpenContextMenu),
        row(
            match (
                bindings.radial_menu_mouse_label(),
                joined_labels(bindings, &[Action::ToggleRadialMenu]),
            ) {
                (Some(mouse), Some(label)) => format!("{mouse} / {label}"),
                (Some(mouse), None) => mouse.to_string(),
                (None, Some(label)) => label,
                (None, None) => NOT_BOUND_LABEL.to_string(),
            },
            action_label(Action::ToggleRadialMenu),
        )
        .with_action(Action::ToggleRadialMenu),
        action_row(bindings, Action::Exit, NOT_BOUND_LABEL),
        action_row(bindings, Action::ToggleHelp, NOT_BOUND_LABEL),
        action_row(bindings, Action::ToggleCommandPalette, NOT_BOUND_LABEL),
        action_row(bindings, Action::ToggleToolbar, NOT_BOUND_LABEL),
        action_row(bindings, Action::CycleToolbarDisplay, NOT_BOUND_LABEL),
        action_row(bindings, Action::TogglePresenterMode, NOT_BOUND_LABEL),
        action_row(bindings, Action::ToggleLightMode, NOT_BOUND_LABEL),
        action_row(bindings, Action::ToggleLightModeDrawing, NOT_BOUND_LABEL),
        action_row(bindings, Action::OpenConfigurator, NOT_BOUND_LABEL),
        action_row(bindings, Action::ToggleStatusBar, NOT_BOUND_LABEL),
    ];
    if frozen_enabled {
        action_rows.push(action_row(
            bindings,
            Action::ToggleFrozenMode,
            NOT_BOUND_LABEL,
        ));
    }
    let actions = Section {
        title: "Actions",
        rows: action_rows,
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_undo),
    };

    let mut screenshot_rows = Vec::new();
    if !context_filter || capture_enabled {
        screenshot_rows.extend([
            row(
                binding_or_fallback(bindings, Action::CaptureClipboardFull, NOT_BOUND_LABEL),
                "Full screen → clipboard",
            )
            .with_action(Action::CaptureClipboardFull),
            row(
                binding_or_fallback(bindings, Action::CaptureFileFull, NOT_BOUND_LABEL),
                "Full screen → file",
            )
            .with_action(Action::CaptureFileFull),
            row(
                binding_or_fallback(bindings, Action::CaptureClipboardSelection, NOT_BOUND_LABEL),
                "Region → clipboard",
            )
            .with_action(Action::CaptureClipboardSelection),
            row(
                binding_or_fallback(bindings, Action::CaptureFileSelection, NOT_BOUND_LABEL),
                "Region → file",
            )
            .with_action(Action::CaptureFileSelection),
            row(
                binding_or_fallback(bindings, Action::CaptureActiveWindow, NOT_BOUND_LABEL),
                "Active window (Hyprland)",
            )
            .with_action(Action::CaptureActiveWindow),
            row(
                binding_or_fallback(bindings, Action::CaptureSelection, NOT_BOUND_LABEL),
                "Selection (capture defaults)",
            )
            .with_action(Action::CaptureSelection),
        ]);
    }
    screenshot_rows.extend([
        action_row(bindings, Action::ExportCanvasClipboard, NOT_BOUND_LABEL),
        action_row(bindings, Action::ExportCanvasFile, NOT_BOUND_LABEL),
        action_row(
            bindings,
            Action::ExportCanvasClipboardAndFile,
            NOT_BOUND_LABEL,
        ),
        action_row(bindings, Action::ExportBoardPdfFile, NOT_BOUND_LABEL),
        action_row(bindings, Action::ExportAllBoardsPdfFile, NOT_BOUND_LABEL),
        action_row(bindings, Action::OpenCaptureFolder, NOT_BOUND_LABEL),
    ]);
    let screenshots = Some(Section {
        title: "Screenshots & Export",
        rows: screenshot_rows,
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_save),
    });

    MainSections {
        board_modes,
        pages,
        drawing,
        selection,
        pen_text,
        zoom,
        actions,
        screenshots,
    }
}
