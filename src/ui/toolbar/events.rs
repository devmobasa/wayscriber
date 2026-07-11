use std::path::PathBuf;

use crate::config::{Action, ToolbarItemId, ToolbarItemOrderGroup, ToolbarLayoutMode};
use crate::draw::{Color, FontDescriptor};
use crate::input::{EraserMode, Tool};

use super::ToolbarSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolbarItemCustomizeGroup {
    TopTools,
    TopControls,
    SideSections,
    Actions,
    Pages,
    Boards,
    Presets,
    ToolOptions,
    Sessions,
}

impl ToolbarItemCustomizeGroup {
    pub const fn label(self) -> &'static str {
        match self {
            Self::TopTools => "Top tools",
            Self::TopControls => "Top controls",
            Self::SideSections => "Side sections",
            Self::Actions => "Actions",
            Self::Pages => "Pages",
            Self::Boards => "Boards",
            Self::Presets => "Presets",
            Self::ToolOptions => "Tool options",
            Self::Sessions => "Sessions",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolbarSideSection {
    Colors,
    Presets,
    Thickness,
    EraserMode,
    PolygonSides,
    ArrowLabels,
    StepMarkers,
    MarkerOpacity,
    TextSize,
    Font,
    Actions,
    Boards,
    Pages,
    StepUndo,
    Session,
    Settings,
}

impl ToolbarSideSection {
    /// Stable id used in config.toml (`ui.toolbar.collapsed_sections`).
    pub fn config_id(self) -> &'static str {
        match self {
            Self::Colors => "colors",
            Self::Presets => "presets",
            Self::Thickness => "thickness",
            Self::EraserMode => "eraser-mode",
            Self::PolygonSides => "polygon-sides",
            Self::ArrowLabels => "arrow-labels",
            Self::StepMarkers => "step-markers",
            Self::MarkerOpacity => "marker-opacity",
            Self::TextSize => "text-size",
            Self::Font => "font",
            Self::Actions => "actions",
            Self::Boards => "boards",
            Self::Pages => "pages",
            Self::StepUndo => "step-undo",
            Self::Session => "session",
            Self::Settings => "settings",
        }
    }

    pub fn from_config_id(value: &str) -> Option<Self> {
        let value = value.trim().to_ascii_lowercase();
        [
            Self::Colors,
            Self::Presets,
            Self::Thickness,
            Self::EraserMode,
            Self::PolygonSides,
            Self::ArrowLabels,
            Self::StepMarkers,
            Self::MarkerOpacity,
            Self::TextSize,
            Self::Font,
            Self::Actions,
            Self::Boards,
            Self::Pages,
            Self::StepUndo,
            Self::Session,
            Self::Settings,
        ]
        .into_iter()
        .find(|section| section.config_id() == value)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Colors => "Colors",
            Self::Presets => "Presets",
            Self::Thickness => "Thickness",
            Self::EraserMode => "Eraser mode",
            Self::PolygonSides => "Sides",
            Self::ArrowLabels => "Arrow labels",
            Self::StepMarkers => "Step markers",
            Self::MarkerOpacity => "Marker opacity",
            Self::TextSize => "Text size",
            Self::Font => "Font",
            Self::Actions => "Actions",
            Self::Boards => "Boards",
            Self::Pages => "Pages",
            Self::StepUndo => "Step Undo/Redo",
            Self::Session => "Session",
            Self::Settings => "Settings",
        }
    }
}

/// The four fixed side-palette panes, selected by the nav row under the
/// header. Panes replace the old open/closed drawer with four tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidePane {
    /// Contextual drawing properties for the active tool.
    #[default]
    Draw,
    /// Canvas management: history, zoom, boards, pages.
    Canvas,
    /// Session persistence.
    Session,
    /// The single customization surface.
    Settings,
}

impl SidePane {
    pub const ALL: [Self; 4] = [Self::Draw, Self::Canvas, Self::Session, Self::Settings];

    pub fn label(self) -> &'static str {
        match self {
            Self::Draw => "Draw",
            Self::Canvas => "Canvas",
            Self::Session => "Session",
            Self::Settings => "Settings",
        }
    }

    /// Stable id used in config.toml (`ui.toolbar.side_active_pane`).
    pub fn config_id(self) -> &'static str {
        match self {
            Self::Draw => "draw",
            Self::Canvas => "canvas",
            Self::Session => "session",
            Self::Settings => "settings",
        }
    }

    pub fn from_config_id(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "draw" => Some(Self::Draw),
            "canvas" => Some(Self::Canvas),
            "session" => Some(Self::Session),
            "settings" => Some(Self::Settings),
            _ => None,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Draw => 0,
            Self::Canvas => 1,
            Self::Session => 2,
            Self::Settings => 3,
        }
    }
}

/// Events emitted by the floating toolbar UI.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolbarEvent {
    SelectTool(Tool),
    SetColor(Color),
    /// Set the color from the side palette's HSV picker. Carries the full
    /// triple so hue survives gray colors (where RGB alone loses it).
    SetColorHsv {
        h: f64,
        s: f64,
        v: f64,
    },
    SetThickness(f64),
    NudgeThickness(f64),
    SetMarkerOpacity(f64),
    NudgeMarkerOpacity(f64),
    SetEraserMode(EraserMode),
    SetFont(FontDescriptor),
    SetFontSize(f64),
    ToggleFill(bool),
    SetPolygonSides(u8),
    NudgePolygonSides(i8),
    ToggleArrowLabels(bool),
    ResetArrowLabelCounter,
    ResetStepMarkerCounter,
    SetUndoDelay(f64),
    SetRedoDelay(f64),
    UndoAll,
    RedoAll,
    UndoAllDelayed,
    RedoAllDelayed,
    Undo,
    Redo,
    ClearCanvas,
    CaptureScreenshot,
    PagePrev,
    PageNext,
    PageNew,
    PageDuplicate,
    PageDelete,
    BoardPrev,
    BoardNext,
    BoardNew,
    BoardDelete,
    BoardDuplicate,
    #[allow(dead_code)]
    BoardRename,
    ToggleBoardPicker,
    EnterTextMode,
    EnterStickyNoteMode,
    /// Toggle both highlight tool and click highlight together
    ToggleAllHighlight(bool),
    /// Toggle highlight tool ring visibility while the highlight tool is active
    ToggleHighlightToolRing(bool),
    ToggleFreeze,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    ToggleZoomLock,
    #[allow(dead_code)]
    RefreshZoomCapture,
    ApplyPreset(usize),
    SavePreset(usize),
    ClearPreset(usize),
    OpenSession,
    OpenRecentSession(PathBuf),
    SaveSessionAs,
    SaveSessionAsConfirm(PathBuf),
    SaveSessionAsCancel,
    SessionInfo,
    ClearSession,
    OpenConfigurator,
    OpenConfigFile,
    ToggleCustomSection(bool),
    ToggleDelaySliders(bool),
    SetCustomUndoDelay(f64),
    SetCustomRedoDelay(f64),
    SetCustomUndoSteps(usize),
    SetCustomRedoSteps(usize),
    CustomUndo,
    CustomRedo,
    /// Open/close the top strip's overflow menu (width-dropped items)
    ToggleTopOverflow(bool),
    /// Minimize the top strip to a small edge tab (click restores), or
    /// restore it. Replaces closing: there is always a way back on screen.
    SetTopMinimized(bool),
    /// Minimize the side palette to a small edge tab, or restore it.
    SetSideMinimized(bool),
    /// Deprecated alias for `SetTopMinimized(true)`; kept so external
    /// callers and old code paths keep working.
    #[allow(dead_code)]
    CloseTopToolbar,
    /// Deprecated alias for `SetSideMinimized(true)`.
    #[allow(dead_code)]
    CloseSideToolbar,
    /// Pin/unpin the top toolbar (saves to config)
    PinTopToolbar(bool),
    /// Pin/unpin the side toolbar (saves to config)
    PinSideToolbar(bool),
    /// Toggle between icon mode and text mode
    ToggleIconMode(bool),
    /// Toggle extended color palette
    ToggleMoreColors(bool),
    /// Copy current color as hex to clipboard
    CopyHexColor,
    /// Paste hex color from clipboard
    PasteHexColor,
    /// Open the color picker popup with the hex field focused for typing
    EditHexColor,
    /// Open the color picker popup
    OpenColorPickerPopup,
    /// Toggle Actions section visibility (undo all, redo all, etc.)
    ToggleActionsSection(bool),
    /// Toggle advanced action buttons
    ToggleActionsAdvanced(bool),
    /// Toggle zoom action buttons
    ToggleZoomActions(bool),
    /// Toggle Pages section visibility
    TogglePagesSection(bool),
    /// Toggle Boards section visibility
    ToggleBoardsSection(bool),
    /// Toggle presets section visibility
    TogglePresets(bool),
    /// Toggle Step Undo/Redo section visibility
    ToggleStepSection(bool),
    /// Toggle persistent text controls visibility
    ToggleTextControls(bool),
    /// Toggle context-aware UI (show/hide controls based on active tool)
    ToggleContextAwareUi(bool),
    /// Toggle preset action toast notifications
    TogglePresetToasts(bool),
    /// Toggle cursor tool preview bubble
    #[allow(dead_code)]
    ToggleToolPreview(bool),
    /// Toggle status bar visibility
    ToggleStatusBar(bool),
    /// Toggle board label in the status bar
    ToggleStatusBoardBadge(bool),
    /// Toggle page counter in the status bar
    ToggleStatusPageBadge(bool),
    /// Toggle the board/page badge when the status bar is visible
    /// (renamed from TogglePageBadgeWithStatusBar for clarity)
    ToggleFloatingBadgeAlways(bool),
    /// Switch the active side-palette pane
    SetSidePane(SidePane),
    /// Set the side-palette scroll offset for the active pane (absolute,
    /// logical pixels; emitted by the scrollbar drag)
    ScrollSidePane(f64),
    /// Collapse/expand a section in the side drawer
    ToggleSideSectionCollapsed(ToolbarSideSection, bool),
    /// Set toolbar layout mode
    SetToolbarLayoutMode(ToolbarLayoutMode),
    /// Hide or show a known toolbar item override.
    SetToolbarItemHidden(ToolbarItemId, bool),
    /// Move an orderable toolbar item by a relative row delta.
    MoveToolbarItem {
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
        delta: isize,
    },
    /// Begin dragging an orderable toolbar item in the customization panel.
    StartToolbarItemDrag {
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
    },
    /// Move the active dragged toolbar item over a target row.
    DragToolbarItemOver {
        group: ToolbarItemOrderGroup,
        target_index: usize,
    },
    /// Reset known order overrides for one toolbar item group.
    ResetToolbarItemOrder(ToolbarItemOrderGroup),
    /// Clear known hidden toolbar item overrides, preserving unknown/future IDs.
    ResetToolbarItemHiddenOverrides,
    /// Show or hide the Settings drawer toolbar-item customization sub-panel.
    SetToolbarItemCustomizationOpen(bool),
    /// Select the Settings drawer toolbar-item customization group.
    SetToolbarItemCustomizationGroup(Option<ToolbarItemCustomizeGroup>),
    /// Toggle the simple-mode shape picker
    ToggleShapePicker(bool),
    /// Drag handle for top toolbar (toolbar coords; screen coords when inline toolbars are active)
    MoveTopToolbar {
        x: f64,
        y: f64,
    },
    /// Drag handle for side toolbar (toolbar coords; screen coords when inline toolbars are active)
    MoveSideToolbar {
        x: f64,
        y: f64,
    },
}

impl ToolbarEvent {
    pub fn action(&self) -> Option<Action> {
        super::model::action_for_event(self)
    }

    /// Events that permanently discard user content; rendered with the
    /// destructive (red-accent) button treatment.
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            ToolbarEvent::ClearCanvas
                | ToolbarEvent::UndoAll
                | ToolbarEvent::UndoAllDelayed
                | ToolbarEvent::BoardDelete
                | ToolbarEvent::PageDelete
                | ToolbarEvent::ClearSession
        )
    }

    pub fn short_label(&self, snapshot: &ToolbarSnapshot, fallback: &'static str) -> &'static str {
        super::model::short_label_for_event(
            self,
            snapshot.frozen_active,
            snapshot.zoom_locked,
            fallback,
        )
    }

    pub fn tooltip_label(
        &self,
        snapshot: &ToolbarSnapshot,
        fallback: &'static str,
    ) -> &'static str {
        super::model::tooltip_label_for_event(
            self,
            snapshot.frozen_active,
            snapshot.zoom_locked,
            fallback,
        )
    }
}

pub(crate) fn action_for_apply_preset(slot: usize) -> Option<Action> {
    super::model::action_for_apply_preset(slot)
}

pub(crate) fn action_for_save_preset(slot: usize) -> Option<Action> {
    super::model::action_for_save_preset(slot)
}

pub(crate) fn action_for_clear_preset(slot: usize) -> Option<Action> {
    super::model::action_for_clear_preset(slot)
}

#[cfg(test)]
mod tests {
    use super::{SidePane, ToolbarSideSection};

    #[test]
    fn side_pane_config_ids_round_trip() {
        for pane in SidePane::ALL {
            assert_eq!(SidePane::from_config_id(pane.config_id()), Some(pane));
        }
        assert_eq!(SidePane::from_config_id(" Canvas "), Some(SidePane::Canvas));
        assert_eq!(SidePane::from_config_id("bogus"), None);
    }

    #[test]
    fn side_section_config_ids_round_trip() {
        let sections = [
            ToolbarSideSection::Colors,
            ToolbarSideSection::Presets,
            ToolbarSideSection::Thickness,
            ToolbarSideSection::EraserMode,
            ToolbarSideSection::PolygonSides,
            ToolbarSideSection::ArrowLabels,
            ToolbarSideSection::StepMarkers,
            ToolbarSideSection::MarkerOpacity,
            ToolbarSideSection::TextSize,
            ToolbarSideSection::Font,
            ToolbarSideSection::Actions,
            ToolbarSideSection::Boards,
            ToolbarSideSection::Pages,
            ToolbarSideSection::StepUndo,
            ToolbarSideSection::Session,
            ToolbarSideSection::Settings,
        ];
        for section in sections {
            assert_eq!(
                ToolbarSideSection::from_config_id(section.config_id()),
                Some(section),
                "config id round trip for {section:?}"
            );
        }
        assert_eq!(ToolbarSideSection::from_config_id("bogus"), None);
    }
}
