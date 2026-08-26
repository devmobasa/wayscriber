use std::path::PathBuf;

use crate::config::{
    Action, StatusBarItem, ToolbarItemId, ToolbarItemOrderGroup, ToolbarLayoutMode,
};
use crate::draw::{Color, FontDescriptor};
use crate::input::{EraserMode, Tool};

use super::ToolbarSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolbarItemCustomizeGroup {
    TopTools,
    TopControls,
    Actions,
    Pages,
    Boards,
    Presets,
    Sessions,
}

impl ToolbarItemCustomizeGroup {
    pub const fn label(self) -> &'static str {
        match self {
            Self::TopTools => "Top tools",
            Self::TopControls => "Top controls",
            Self::Actions => "Actions",
            Self::Pages => "Pages",
            Self::Boards => "Boards",
            Self::Presets => "Presets",
            Self::Sessions => "Sessions",
        }
    }
}

/// Value targeted by the style pill's precise-entry popup (opened from
/// the pill's live numeral buttons).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecisionEntryTarget {
    /// Stroke thickness (the snapshot already routes eraser/marker sizes
    /// through the thickness slider, so one target covers all px numerals).
    Thickness,
    /// Text size in points.
    FontSize,
}

impl PrecisionEntryTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Thickness => "Thickness",
            Self::FontSize => "Text size",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::Thickness => "px",
            Self::FontSize => "pt",
        }
    }
}

/// Events emitted by the floating toolbar UI.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolbarEvent {
    SelectTool(Tool),
    #[cfg_attr(not(feature = "toolbar-gtk"), allow(dead_code))]
    SetColor(Color),
    /// Set a configured quick-color slot while preserving which binding the
    /// clicked swatch represents, even when multiple slots share an RGB value.
    /// `index` is the palette slot the swatch renders, which secondary-click
    /// recoloring needs because slots past the eighth carry no action.
    SetQuickColor {
        color: Color,
        action: Option<Action>,
        index: usize,
    },
    /// Recolor a quick-color slot: opens the color picker popup bound to that
    /// palette slot, so accepting it rewrites the swatch (and `config.toml`)
    /// instead of only the active tool's color. Emitted by secondary-clicking
    /// a swatch in the style pill.
    EditQuickColor {
        index: usize,
    },
    SetThickness(f64),
    NudgeThickness(f64),
    SetMarkerOpacity(f64),
    NudgeMarkerOpacity(f64),
    SetSpotlightMagnification(f64),
    /// Smoothing passes applied to freehand and marker strokes on release.
    SetPenSmoothing(u8),
    SetEraserMode(EraserMode),
    SetFont(FontDescriptor),
    /// Turn bold on or off for selected text, or for the next label typed.
    SetFontBold(bool),
    /// Open the overlay's system font picker from the current-family button.
    OpenFontPicker,
    SetFontSize(f64),
    NudgeFontSize(f64),
    ToggleFill(bool),
    SetPolygonSides(u8),
    NudgePolygonSides(i8),
    ToggleArrowLabels(bool),
    /// Step the next arrow's style through the four arrow styles.
    CycleArrowStyle,
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
    /// Clear the canvas. The default mouse path (`instant: false`) offers a
    /// short "Cleared — Undo?" toast; Shift+click and the keyboard action use
    /// the instant variant with no toast.
    ClearCanvas {
        instant: bool,
    },
    CaptureScreenshot,
    /// Select a screen region and copy the text recognized in it.
    CopyTextFromScreen,
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
    /// Toggle the on-screen input HUD (keystrokes and clicks)
    ToggleInputHud(bool),
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
    /// Open the standalone About dialog. The overlay exits first: it is a
    /// layer-shell surface, so an About toplevel underneath would be hidden.
    OpenAbout,
    /// Reset generated runtime UI preferences. Supported state resets
    /// immediately; newer unsupported state first requests confirmation.
    RequestRuntimeUiReset,
    ConfirmUnsupportedRuntimeUiReset,
    CancelUnsupportedRuntimeUiReset,
    RetryRuntimeUiPersistence,
    DiscardPendingRuntimeUiAndAdoptDisk,
    RequestPreserveInvalidRuntimeUiReset,
    ConfirmPreserveInvalidRuntimeUiReset,
    CancelPreserveInvalidRuntimeUiReset,
    CancelRuntimeUiRecovery,
    /// Open (toggle) the command palette overlay.
    OpenCommandPalette,
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
    /// Open/close the Session popover anchored to the top strip's overflow
    /// toggle. Opening it closes the Settings popover and the overflow menu.
    ToggleSessionPopover(bool),
    /// Open/close the Settings popover anchored to the top strip's overflow
    /// toggle. Opening it closes the Session popover and the overflow menu.
    ToggleSettingsPopover(bool),
    /// Open/close the Canvas popover anchored to the top strip's overflow
    /// toggle. Opening it closes the Session/Settings popovers and the
    /// overflow menu.
    ToggleCanvasPopover(bool),
    /// Set the internal scroll offset of the open Canvas/Session/Settings
    /// popover (absolute, logical pixels; emitted by the popover scrollbar
    /// drag).
    ScrollTopPopover(f64),
    /// Minimize the top strip to a small edge tab (click restores), or
    /// restore it. Replaces closing: there is always a way back on screen.
    SetTopMinimized(bool),
    /// Set the top strip's display form (full strip / micro chip / hidden).
    /// The micro chip's click emits `SetTopDisplayMode(Full)`.
    SetTopDisplayMode(crate::config::TopDisplayMode),
    /// Deprecated alias for `SetTopMinimized(true)`; kept so external
    /// callers and old code paths keep working.
    #[allow(dead_code)]
    CloseTopToolbar,
    /// Pin/unpin the top toolbar (saves to config)
    PinTopToolbar(bool),
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
    /// Open the precise numeric entry popup for a pill numeral. The popup
    /// renders on the overlay (the same Cairo keyboard surface as the
    /// color popup's hex field) and commits/cancels via the events below.
    OpenPrecisionEntry(PrecisionEntryTarget),
    /// Commit a typed value from the precise-entry popup; the apply arm
    /// clamps it to the target's slider range.
    CommitPrecisionEntry {
        target: PrecisionEntryTarget,
        value: f64,
    },
    /// Dismiss the precise-entry popup without applying.
    CancelPrecisionEntry,
    /// Adjust one property of the current selection from the style pill's
    /// docked selection controls. Routes through the same apply machinery
    /// as the overlay properties popup; cycle controls use `direction = 1`,
    /// steppers -1/+1.
    AdjustSelectionProperty {
        kind: crate::input::SelectionPropertyKind,
        direction: i32,
    },
    /// Pick a color from the displayed desktop image.
    PickScreenColor,
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
    /// Toggle top-strip idle fade
    ToggleIdleFade(bool),
    /// Toggle cursor tool preview bubble
    #[allow(dead_code)]
    ToggleToolPreview(bool),
    /// Toggle status bar visibility
    ToggleStatusBar(bool),
    /// Allow or reject interaction with visible status-bar segments.
    SetStatusBarInteractive(bool),
    /// Show or hide one independently configurable status-bar item.
    SetStatusBarItemVisible(StatusBarItem, bool),
    /// Toggle board label in the status bar
    ToggleStatusBoardBadge(bool),
    /// Toggle page counter in the status bar
    ToggleStatusPageBadge(bool),
    /// Toggle the board/page badge when the status bar is visible
    /// (renamed from TogglePageBadgeWithStatusBar for clarity)
    ToggleFloatingBadgeAlways(bool),
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
    /// Show or hide the Settings drawer status-bar content sub-panel.
    SetStatusBarContentsOpen(bool),
    /// Toggle the simple-mode shape picker
    ToggleShapePicker(bool),
    /// Drag handle for top toolbar (toolbar coords; screen coords when inline toolbars are active)
    MoveTopToolbar {
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
            ToolbarEvent::ClearCanvas { .. }
                | ToolbarEvent::UndoAll
                | ToolbarEvent::UndoAllDelayed
                | ToolbarEvent::BoardDelete
                | ToolbarEvent::PageDelete
                | ToolbarEvent::ClearSession
                | ToolbarEvent::RequestRuntimeUiReset
                | ToolbarEvent::ConfirmUnsupportedRuntimeUiReset
                | ToolbarEvent::DiscardPendingRuntimeUiAndAdoptDisk
                | ToolbarEvent::ConfirmPreserveInvalidRuntimeUiReset
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
