use std::path::PathBuf;

use wayscriber::config::{ConfigDocument, ShortcutTrigger, ToolbarItemId, ToolbarItemOrderGroup};

use crate::models::{
    BoardBackgroundOption, BoardItemTextField, BoardItemToggleField, ColorMode, ColorPickerId,
    DaemonAction, DaemonActionResult, DaemonRuntimeStatus, DragColorOption, DragMouseButton,
    DragToolField, DragToolOption, EraserModeOption, FontStyleOption, FontWeightOption,
    InputHudModeOption, InputHudPositionOption, KeybindingField, KeybindingsTabId,
    KeyboardModifiers, NamedColorOption, OverrideOption, PdfFitModeOption,
    PdfLabelContentModeOption, PdfLabelPositionOption, PdfOrientationOption, PdfPageSizeOption,
    PdfTransparentBackgroundOption, PresenterToolBehaviorOption, PresenterToolbarModeOption,
    PresetEraserKindOption, PresetEraserModeOption, PresetTextField, PresetToggleField,
    RecorderDeviceKind, ReducedMotionOption, RenderProfileExportOption, RenderProfileMappingSide,
    RenderProfileTextField, SessionCatalogActionResult, SessionCatalogItem,
    SessionCompressionOption, SessionStorageModeOption, StatusPositionOption, TabId, TextField,
    ToggleField, ToolOption, ToolbarLayoutModeOption, ToolbarOverrideField,
    ToolbarRebindModifierOption, UiTabId, UiThemeOption, ZoomChipDisplayOption,
};
#[cfg(feature = "tablet-input")]
use crate::models::{PressureThicknessEditModeOption, PressureThicknessEntryModeOption};

/// What a finished write reports back: the document it produced and any backup
/// it made, or why nothing was written.
///
/// Both arms carry a document, because the model gave its only copy away to
/// start the write and needs one back. The failure arm's is `None` in exactly
/// one case: a blocking job that never returned took the document it was
/// holding with it, leaving a reload as the way forward.
pub type ConfigSaveResult =
    Result<(Option<PathBuf>, Box<ConfigDocument>), (Option<Box<ConfigDocument>>, String)>;

/// What a finished `Effect` reports back.
///
/// Separate from [`Message`] because none of these can come from a widget:
/// they are the results of jobs the update layer asked for, they carry values
/// (the config document above all) that are moved rather than copied, and
/// nothing may clone them.
#[derive(Debug)]
pub enum CommandMessage {
    ConfigLoaded(Result<(Box<ConfigDocument>, Option<String>), String>),
    ConfigSaved(ConfigSaveResult),
    DaemonStatusLoaded(u64, Result<DaemonRuntimeStatus, String>),
    DaemonActionCompleted(Result<DaemonActionResult, String>),
    SessionCatalogLoaded(Result<Vec<SessionCatalogItem>, String>),
    SessionCatalogActionCompleted(Result<SessionCatalogActionResult, String>),
}

/// What the user asked for, always from a widget.
///
/// Cloned by the page builders that hand one message to a button closure, so
/// every payload here stays cheap to copy.
#[derive(Debug, Clone)]
pub enum Message {
    ReloadRequested,
    /// Asks for the reset and arms the confirmation; it never replaces the
    /// draft on its own.
    ResetToDefaultsRequested,
    /// Answers an armed confirmation with yes. Applying belongs to this
    /// message alone, so the control that asks and the control that answers
    /// are separate in every channel.
    ResetToDefaultsConfirmed,
    /// Answers an armed confirmation with no.
    ResetToDefaultsCanceled,
    SaveRequested,
    MigrationApplyRequested,
    MigrationDismissed,
    DaemonShortcutInputChanged(String),
    DaemonActionRequested(DaemonAction),
    SessionCatalogRefreshRequested,
    SessionCatalogForgetRequested(String),
    SessionCatalogRenameInputChanged(String, String),
    SessionCatalogRenameRequested(String),
    SessionCatalogDuplicateInputChanged(String, String),
    SessionCatalogDuplicateRequested(String),
    SessionCatalogMoveInputChanged(String, String),
    SessionCatalogMoveRequested(String),
    SessionCatalogRevealRequested(String),
    SessionCatalogClearToolStateRequested(String),
    SessionCatalogClearRequested(String),
    SessionCatalogClearConfirmed(String),
    SessionCatalogClearCanceled(String),
    SearchChanged(String),
    SearchCleared,
    SearchFocusRequested,
    /// The user clicked, tapped, or Tab-navigated before the initial config
    /// load landed; any still-pending startup search focus stands down.
    StartupInteractionObserved,
    TabSelected(TabId),
    UiTabSelected(UiTabId),
    KeybindingsTabSelected(KeybindingsTabId),
    ToggleChanged(ToggleField, bool),
    TextChanged(TextField, String),
    ColorPickerHexChanged(ColorPickerId, String),
    ColorModeChanged(ColorMode),
    NamedColorSelected(NamedColorOption),
    QuickColorAdded,
    QuickColorRemoved(usize),
    QuickColorMoved(usize, isize),
    QuickColorModeChanged(usize, ColorMode),
    QuickNamedColorSelected(usize, NamedColorOption),
    EraserModeChanged(EraserModeOption),
    DrawingDragMappingSectionToggled(DragMouseButton),
    DrawingMouseDragToolChanged(DragMouseButton, DragToolField, DragToolOption),
    DrawingMouseDragColorChanged(DragMouseButton, DragToolField, DragColorOption),
    StatusPositionChanged(StatusPositionOption),
    InputHudModeChanged(InputHudModeOption),
    InputHudPositionChanged(InputHudPositionOption),
    UiThemeChanged(UiThemeOption),
    UiReducedMotionChanged(ReducedMotionOption),
    ToolbarLayoutModeChanged(ToolbarLayoutModeOption),
    ToolbarZoomChipDisplayChanged(ZoomChipDisplayOption),
    ToolbarRebindModifierChanged(ToolbarRebindModifierOption),
    ToolbarOverrideModeChanged(ToolbarLayoutModeOption),
    ToolbarOverrideChanged(ToolbarOverrideField, OverrideOption),
    ToolbarItemVisibilityChanged(ToolbarItemId, bool),
    ToolbarItemMoveRequested(ToolbarItemOrderGroup, ToolbarItemId, isize),
    ToolbarItemOrderReset(ToolbarItemOrderGroup),
    BoardsAddItem,
    BoardsRemoveItem(usize),
    BoardsMoveItemUp(usize),
    BoardsMoveItemDown(usize),
    BoardsDuplicateItem(usize),
    BoardsCollapseToggled(usize),
    BoardsDefaultChanged(String),
    BoardsItemTextChanged(usize, BoardItemTextField, String),
    BoardsBackgroundKindChanged(usize, BoardBackgroundOption),
    BoardsBackgroundColorChanged(usize, usize, String),
    BoardsDefaultPenEnabledChanged(usize, bool),
    BoardsDefaultPenColorChanged(usize, usize, String),
    BoardsItemToggleChanged(usize, BoardItemToggleField, bool),
    RenderProfileAdd,
    RenderProfileRemove(usize),
    RenderProfileDuplicate(usize),
    RenderProfileTextChanged(usize, RenderProfileTextField, String),
    RenderProfileActiveChanged(String),
    RenderProfileExportChanged(RenderProfileExportOption),
    RenderProfileExportProfileChanged(String),
    RenderProfileApplyCanvasChanged(bool),
    RenderProfileApplyUiChanged(bool),
    RenderProfileMappingAdd(usize),
    RenderProfileMappingRemove(usize, usize),
    RenderProfileMappingColorChanged(usize, usize, RenderProfileMappingSide, String),
    SessionStorageModeChanged(SessionStorageModeOption),
    SessionCompressionChanged(SessionCompressionOption),
    PresenterToolBehaviorChanged(PresenterToolBehaviorOption),
    PresenterToolbarModeChanged(PresenterToolbarModeOption),
    ExportPdfPageSizeChanged(PdfPageSizeOption),
    ExportPdfOrientationChanged(PdfOrientationOption),
    ExportPdfFitChanged(PdfFitModeOption),
    ExportPdfTransparentBackgroundChanged(PdfTransparentBackgroundOption),
    ExportPdfLabelPositionChanged(PdfLabelPositionOption),
    ExportPdfLabelContentChanged(PdfLabelContentModeOption),
    BufferCountChanged(u32),
    ShortcutRecordingStarted(KeybindingField),
    ShortcutRecordingCanceled(KeybindingField),
    ShortcutRecorderKey(u32, KeyboardModifiers),
    ShortcutRecorderButton(u32, RecorderDeviceKind, KeyboardModifiers),
    ShortcutRemoved(KeybindingField, ShortcutTrigger),
    ShortcutResetRequested(KeybindingField),
    ShortcutTextEditStarted(KeybindingField),
    ShortcutTextEditChanged(String),
    ShortcutTextEditApplied,
    ShortcutTextEditCanceled(KeybindingField),
    ShortcutConflictReplaceConfirmed,
    ShortcutConflictCanceled,
    /// Window-level Escape: cancel a confirmation unless a recorder owns the key.
    WindowEscapePressed,
    FontStyleOptionSelected(FontStyleOption),
    FontWeightOptionSelected(FontWeightOption),
    #[cfg(feature = "tablet-input")]
    TabletPressureEditModeChanged(PressureThicknessEditModeOption),
    #[cfg(feature = "tablet-input")]
    TabletPressureEntryModeChanged(PressureThicknessEntryModeOption),
    PresetSlotCountChanged(usize),
    PresetSlotEnabledChanged(usize, bool),
    PresetCollapseToggled(usize),
    PresetResetSlot(usize),
    PresetDuplicateSlot(usize),
    PresetToolChanged(usize, ToolOption),
    PresetColorModeChanged(usize, ColorMode),
    PresetNamedColorSelected(usize, NamedColorOption),
    PresetColorComponentChanged(usize, usize, String),
    PresetTextChanged(usize, PresetTextField, String),
    PresetToggleOptionChanged(usize, PresetToggleField, OverrideOption),
    PresetEraserKindChanged(usize, PresetEraserKindOption),
    PresetEraserModeChanged(usize, PresetEraserModeOption),
}
