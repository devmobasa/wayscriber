use crate::config::{Action, QuickColorSlot, action_label};
use crate::label_format::NOT_BOUND_LABEL;
use crate::toolbar_icons;

use super::super::super::types::{Badge, Section, row};
use super::super::bindings::{
    HelpOverlayBindings, binding_or_fallback, bindings_compact_or_fallback, bindings_or_fallback,
    joined_labels, primary_or_fallback,
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
            row(
                binding_or_fallback(bindings, Action::ToggleWhiteboard, NOT_BOUND_LABEL),
                action_label(Action::ToggleWhiteboard),
            ),
            row(
                binding_or_fallback(bindings, Action::ToggleBlackboard, NOT_BOUND_LABEL),
                action_label(Action::ToggleBlackboard),
            ),
            row(
                binding_or_fallback(bindings, Action::ReturnToTransparent, NOT_BOUND_LABEL),
                action_label(Action::ReturnToTransparent),
            ),
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
            row(
                binding_or_fallback(bindings, Action::BoardNew, NOT_BOUND_LABEL),
                action_label(Action::BoardNew),
            ),
            row(
                binding_or_fallback(bindings, Action::BoardDelete, NOT_BOUND_LABEL),
                action_label(Action::BoardDelete),
            ),
            row(
                binding_or_fallback(bindings, Action::BoardPicker, NOT_BOUND_LABEL),
                action_label(Action::BoardPicker),
            ),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_settings),
    });

    let pages = Section {
        title: "Pages",
        rows: vec![
            row(
                binding_or_fallback(bindings, Action::PagePrev, NOT_BOUND_LABEL),
                action_label(Action::PagePrev),
            ),
            row(
                binding_or_fallback(bindings, Action::PageNext, NOT_BOUND_LABEL),
                action_label(Action::PageNext),
            ),
            row(
                binding_or_fallback(bindings, Action::PageNew, NOT_BOUND_LABEL),
                action_label(Action::PageNew),
            ),
            row(
                binding_or_fallback(bindings, Action::PageDuplicate, NOT_BOUND_LABEL),
                action_label(Action::PageDuplicate),
            ),
            row(
                binding_or_fallback(bindings, Action::PageDelete, NOT_BOUND_LABEL),
                action_label(Action::PageDelete),
            ),
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
            row(
                binding_or_fallback(bindings, Action::SelectPenTool, NOT_BOUND_LABEL),
                action_label(Action::SelectPenTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectLineTool, "Shift+Drag"),
                action_label(Action::SelectLineTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectRectTool, "Ctrl+Drag"),
                action_label(Action::SelectRectTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectEllipseTool, "Tab+Drag"),
                action_label(Action::SelectEllipseTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectArrowTool, "Ctrl+Shift+Drag"),
                action_label(Action::SelectArrowTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectBlurTool, NOT_BOUND_LABEL),
                action_label(Action::SelectBlurTool),
            ),
            row(
                binding_or_fallback(bindings, Action::ToggleHighlightTool, NOT_BOUND_LABEL),
                action_label(Action::ToggleHighlightTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectMarkerTool, NOT_BOUND_LABEL),
                action_label(Action::SelectMarkerTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectStepMarkerTool, NOT_BOUND_LABEL),
                action_label(Action::SelectStepMarkerTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectEraserTool, NOT_BOUND_LABEL),
                action_label(Action::SelectEraserTool),
            ),
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
            row(
                binding_or_fallback(bindings, Action::SelectSelectionTool, NOT_BOUND_LABEL),
                action_label(Action::SelectSelectionTool),
            ),
            row("Drag", "Selection tool"),
            row(
                binding_or_fallback(bindings, Action::SelectAll, NOT_BOUND_LABEL),
                action_label(Action::SelectAll),
            ),
            row(
                binding_or_fallback(bindings, Action::DuplicateSelection, NOT_BOUND_LABEL),
                action_label(Action::DuplicateSelection),
            ),
            row(
                binding_or_fallback(bindings, Action::CopySelection, NOT_BOUND_LABEL),
                action_label(Action::CopySelection),
            ),
            row(
                binding_or_fallback(bindings, Action::PasteSelection, NOT_BOUND_LABEL),
                action_label(Action::PasteSelection),
            ),
            row(
                binding_or_fallback(bindings, Action::DeleteSelection, NOT_BOUND_LABEL),
                action_label(Action::DeleteSelection),
            ),
            row(
                binding_or_fallback(bindings, Action::ToggleSelectionProperties, NOT_BOUND_LABEL),
                action_label(Action::ToggleSelectionProperties),
            ),
            row(
                binding_or_fallback(bindings, Action::IncreaseFontSize, NOT_BOUND_LABEL),
                action_label(Action::IncreaseFontSize),
            ),
            row(
                binding_or_fallback(bindings, Action::DecreaseFontSize, NOT_BOUND_LABEL),
                action_label(Action::DecreaseFontSize),
            ),
        ],
        badges: color_badges.clone(),
        icon: Some(toolbar_icons::draw_icon_select),
    };

    let pen_text = Section {
        title: "Pen & Text",
        rows: vec![
            row(
                binding_or_fallback(bindings, Action::EnterTextMode, NOT_BOUND_LABEL),
                action_label(Action::EnterTextMode),
            ),
            row(
                binding_or_fallback(bindings, Action::EnterStickyNoteMode, NOT_BOUND_LABEL),
                action_label(Action::EnterStickyNoteMode),
            ),
            row(
                binding_or_fallback(bindings, Action::IncreaseFontSize, NOT_BOUND_LABEL),
                action_label(Action::IncreaseFontSize),
            ),
            row(
                binding_or_fallback(bindings, Action::DecreaseFontSize, NOT_BOUND_LABEL),
                action_label(Action::DecreaseFontSize),
            ),
            row(
                binding_or_fallback(bindings, Action::ToggleFill, NOT_BOUND_LABEL),
                action_label(Action::ToggleFill),
            ),
            row("Selection properties panel", "Text background"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_text),
    };

    let zoom = Section {
        title: "Zoom",
        rows: vec![
            row(
                binding_or_fallback(bindings, Action::ZoomIn, NOT_BOUND_LABEL),
                action_label(Action::ZoomIn),
            ),
            row(
                binding_or_fallback(bindings, Action::ZoomOut, NOT_BOUND_LABEL),
                action_label(Action::ZoomOut),
            ),
            row(
                binding_or_fallback(bindings, Action::ResetZoom, NOT_BOUND_LABEL),
                action_label(Action::ResetZoom),
            ),
            row(
                binding_or_fallback(bindings, Action::ToggleZoomLock, NOT_BOUND_LABEL),
                action_label(Action::ToggleZoomLock),
            ),
            row("Middle drag / arrow keys", "Pan view"),
            row(
                binding_or_fallback(bindings, Action::RefreshZoomCapture, NOT_BOUND_LABEL),
                action_label(Action::RefreshZoomCapture),
            ),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_zoom_in),
    };

    let mut action_rows = vec![
        row(
            binding_or_fallback(bindings, Action::ClearCanvas, NOT_BOUND_LABEL),
            action_label(Action::ClearCanvas),
        ),
        row(
            binding_or_fallback(bindings, Action::Undo, NOT_BOUND_LABEL),
            action_label(Action::Undo),
        ),
        row(
            binding_or_fallback(bindings, Action::ToggleClickHighlight, NOT_BOUND_LABEL),
            action_label(Action::ToggleClickHighlight),
        ),
        row(
            match joined_labels(bindings, &[Action::OpenContextMenu]) {
                Some(label) => format!("Right Click / {label}"),
                None => "Right Click".to_string(),
            },
            action_label(Action::OpenContextMenu),
        ),
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
        ),
        row(
            binding_or_fallback(bindings, Action::Exit, NOT_BOUND_LABEL),
            action_label(Action::Exit),
        ),
        row(
            binding_or_fallback(bindings, Action::ToggleHelp, NOT_BOUND_LABEL),
            action_label(Action::ToggleHelp),
        ),
        row(
            binding_or_fallback(bindings, Action::ToggleCommandPalette, NOT_BOUND_LABEL),
            action_label(Action::ToggleCommandPalette),
        ),
        row(
            binding_or_fallback(bindings, Action::ToggleToolbar, NOT_BOUND_LABEL),
            action_label(Action::ToggleToolbar),
        ),
        row(
            binding_or_fallback(bindings, Action::TogglePresenterMode, NOT_BOUND_LABEL),
            action_label(Action::TogglePresenterMode),
        ),
        row(
            binding_or_fallback(bindings, Action::ToggleLightMode, NOT_BOUND_LABEL),
            action_label(Action::ToggleLightMode),
        ),
        row(
            binding_or_fallback(bindings, Action::ToggleLightModeDrawing, NOT_BOUND_LABEL),
            action_label(Action::ToggleLightModeDrawing),
        ),
        row(
            binding_or_fallback(bindings, Action::OpenConfigurator, NOT_BOUND_LABEL),
            action_label(Action::OpenConfigurator),
        ),
        row(
            binding_or_fallback(bindings, Action::ToggleStatusBar, NOT_BOUND_LABEL),
            action_label(Action::ToggleStatusBar),
        ),
    ];
    if frozen_enabled {
        action_rows.push(row(
            binding_or_fallback(bindings, Action::ToggleFrozenMode, NOT_BOUND_LABEL),
            action_label(Action::ToggleFrozenMode),
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
            ),
            row(
                binding_or_fallback(bindings, Action::CaptureFileFull, NOT_BOUND_LABEL),
                "Full screen → file",
            ),
            row(
                binding_or_fallback(bindings, Action::CaptureClipboardSelection, NOT_BOUND_LABEL),
                "Region → clipboard",
            ),
            row(
                binding_or_fallback(bindings, Action::CaptureFileSelection, NOT_BOUND_LABEL),
                "Region → file",
            ),
            row(
                binding_or_fallback(bindings, Action::CaptureActiveWindow, NOT_BOUND_LABEL),
                "Active window (Hyprland)",
            ),
            row(
                binding_or_fallback(bindings, Action::CaptureSelection, NOT_BOUND_LABEL),
                "Selection (capture defaults)",
            ),
        ]);
    }
    screenshot_rows.extend([
        row(
            binding_or_fallback(bindings, Action::ExportCanvasClipboard, NOT_BOUND_LABEL),
            action_label(Action::ExportCanvasClipboard),
        ),
        row(
            binding_or_fallback(bindings, Action::ExportCanvasFile, NOT_BOUND_LABEL),
            action_label(Action::ExportCanvasFile),
        ),
        row(
            binding_or_fallback(
                bindings,
                Action::ExportCanvasClipboardAndFile,
                NOT_BOUND_LABEL,
            ),
            action_label(Action::ExportCanvasClipboardAndFile),
        ),
        row(
            binding_or_fallback(bindings, Action::ExportBoardPdfFile, NOT_BOUND_LABEL),
            action_label(Action::ExportBoardPdfFile),
        ),
        row(
            binding_or_fallback(bindings, Action::ExportAllBoardsPdfFile, NOT_BOUND_LABEL),
            action_label(Action::ExportAllBoardsPdfFile),
        ),
        row(
            binding_or_fallback(bindings, Action::OpenCaptureFolder, NOT_BOUND_LABEL),
            action_label(Action::OpenCaptureFolder),
        ),
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
