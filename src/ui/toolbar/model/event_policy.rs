use crate::config::{
    Action, ToolbarItemId, ToolbarItemOrderGroup, ToolbarItemVisibilitySetting, ToolbarSectionFlag,
    action_label, action_short_label, section_flag_for_item,
};
use crate::input::Tool;

use super::super::ToolbarEvent;

/// The popover an event belongs to, if any.
///
/// Dismissal used to be five hand-maintained exclusion lists, one per
/// popover, each spelling out the events that must *not* close it. Adding a
/// control meant remembering which list it belonged in, and forgetting closed
/// the popover out from under the pointer the first time the control was
/// used. Declaring the owner once here derives every list instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarPopover {
    /// The overflow flyout and the shapes picker anchored beside it.
    TopOverflow,
    ShapePicker,
    /// The three overflow-anchored menu popovers.
    Canvas,
    Session,
    Settings,
    /// The precise-entry popup.
    PrecisionEntry,
    /// The chrome island's layout-preset menu.
    LayoutMenu,
    /// The style pill's Pen feel panel.
    PenFeel,
    /// The style pill's arrow style menu.
    ArrowStyleMenu,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToolbarEventPolicy {
    pub(crate) persistence: ToolbarPersistence,
    /// The popovers this event operates inside, so it does not dismiss them.
    pub(crate) popovers: &'static [ToolbarPopover],
    pub(crate) backend_route: ToolbarBackendRoute,
    pub(crate) tablet_thickness_sensitive: bool,
}

impl ToolbarEventPolicy {
    pub(crate) fn for_event(event: &ToolbarEvent) -> Self {
        Self {
            persistence: persistence_for_event(event),
            popovers: popovers_for_event(event),
            backend_route: backend_route_for_event(event),
            tablet_thickness_sensitive: matches!(
                event,
                ToolbarEvent::SetThickness(_) | ToolbarEvent::NudgeThickness(_)
            ),
        }
    }
}

/// Where an applied toolbar event's value survives to.
///
/// `config.toml` is not a destination for a preference toggle. The file is an
/// authored input, changed only by an explicit user edit action: the
/// configurator's Save, or one of the overlay's three scoped edits — a shortcut
/// rebind, a preset slot, a quick-color swatch — each of which writes its own
/// key through the audited worker path. Flipping a toolbar preference is not
/// one of them.
///
/// A preference the user flips from the overlay — icons, section visibility,
/// layout mode, status bar, badges, click highlight, the input HUD — persists
/// as a `RuntimeUi` override layered over the authored value instead, so it
/// survives a restart while `config.toml` reads exactly as written. The
/// authored value stays the seed the override is measured against, which is
/// why no toggle may write it: a seed that moved with its own override would
/// make the override redundant, and reconciliation would drop it.
///
/// `Ephemeral` is what is left: drawing state the session file owns, and
/// one-shot actions that persist nothing of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarPersistence {
    Ephemeral,
    RuntimeUi(ToolbarRuntimeUiPersistenceTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarRuntimeUiPersistenceTarget {
    TopPinned,
    TopMinimized,
    NamedSection(crate::config::ToolbarSectionFlag),
    LayoutMode,
    ClickHighlight,
    /// Keyboard-only chrome: no toolbar control routes to these, but they
    /// persist through the same targets so a restart restores them alike.
    FloatingBadge,
    ZoomChip,
    ItemVisibility {
        id: ToolbarItemId,
        setting: ToolbarItemVisibilitySetting,
    },
    ItemOrder(ToolbarItemOrderGroup),
    StatusBarInteractive,
    StatusBarItem(crate::config::StatusBarItem),
    StatusBar,
    StatusBoardBadge,
    StatusPageBadge,
    FloatingBadgeAlways,
    ToolbarIcons,
    ToolbarMoreColors,
    ToolbarContextAwareUi,
    ToolbarPresetToasts,
    ToolbarIdleFade,
    ToolbarToolPreview,
    ToolbarDelaySliders,
    HistoryCustomSection,
    InputHud,
    ResetItemVisibility,
    /// Top-strip form (`full`/`micro`). The runtime-only `hidden` rung of the
    /// cycle is folded to `full` when the override is computed.
    TopDisplayMode,
    /// The durable form of the keyboard visibility toggle: both pin flags
    /// persisted as one batched mutation, so a restart shows exactly what the
    /// toggle left on screen. No `ToolbarEvent` maps to it — the pin buttons
    /// keep their single-flag targets; this target serves the keyboard action
    /// path only.
    ToolbarVisibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarBackendRoute {
    ApplyToInput,
    MoveTopToolbar,
}

/// The keyboard action a toolbar event is equivalent to, which gives its
/// control a shortcut badge, a tooltip label, rebind capture, and shortcut
/// coaching. Exhaustive, so a new event has to decide instead of silently
/// going without.
pub(crate) fn action_for_event(event: &ToolbarEvent) -> Option<Action> {
    match event {
        ToolbarEvent::SelectTool(tool) => action_for_tool(*tool),
        ToolbarEvent::SetQuickColor { action, .. } => *action,
        ToolbarEvent::EnterTextMode => Some(Action::EnterTextMode),
        ToolbarEvent::EnterStickyNoteMode => Some(Action::EnterStickyNoteMode),
        ToolbarEvent::ToggleFill(_) => Some(Action::ToggleFill),
        ToolbarEvent::NudgeThickness(delta) if *delta > 0.0 => Some(Action::IncreaseThickness),
        ToolbarEvent::NudgeThickness(delta) if *delta < 0.0 => Some(Action::DecreaseThickness),
        ToolbarEvent::NudgeMarkerOpacity(delta) if *delta > 0.0 => {
            Some(Action::IncreaseMarkerOpacity)
        }
        ToolbarEvent::NudgeMarkerOpacity(delta) if *delta < 0.0 => {
            Some(Action::DecreaseMarkerOpacity)
        }
        ToolbarEvent::SetEraserMode(_) => Some(Action::ToggleEraserMode),
        ToolbarEvent::NudgeFontSize(delta) if *delta > 0.0 => Some(Action::IncreaseFontSize),
        ToolbarEvent::NudgeFontSize(delta) if *delta < 0.0 => Some(Action::DecreaseFontSize),
        ToolbarEvent::Undo => Some(Action::Undo),
        ToolbarEvent::Redo => Some(Action::Redo),
        ToolbarEvent::UndoAll => Some(Action::UndoAll),
        ToolbarEvent::RedoAll => Some(Action::RedoAll),
        ToolbarEvent::UndoAllDelayed => Some(Action::UndoAllDelayed),
        ToolbarEvent::RedoAllDelayed => Some(Action::RedoAllDelayed),
        ToolbarEvent::ClearCanvas { .. } => Some(Action::ClearCanvas),
        ToolbarEvent::CaptureScreenshot => Some(Action::CaptureRegionInteractive),
        ToolbarEvent::CopyTextFromScreen => Some(Action::CopyTextFromScreen),
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
        ToolbarEvent::BoardRename | ToolbarEvent::ToggleBoardPicker => Some(Action::BoardPicker),
        ToolbarEvent::ToggleAllHighlight(_) => Some(Action::ToggleHighlightTool),
        ToolbarEvent::ToggleFreeze => Some(Action::ToggleFrozenMode),
        ToolbarEvent::ZoomIn => Some(Action::ZoomIn),
        ToolbarEvent::ZoomOut => Some(Action::ZoomOut),
        ToolbarEvent::ResetZoom => Some(Action::ResetZoom),
        ToolbarEvent::ResetStepMarkerCounter => Some(Action::ResetStepMarkerCounter),
        ToolbarEvent::ResetArrowLabelCounter => Some(Action::ResetArrowLabelCounter),
        ToolbarEvent::ToggleZoomLock => Some(Action::ToggleZoomLock),
        ToolbarEvent::ApplyPreset(slot) => action_for_apply_preset(*slot),
        ToolbarEvent::SavePreset(slot) => action_for_save_preset(*slot),
        ToolbarEvent::ClearPreset(slot) => action_for_clear_preset(*slot),
        ToolbarEvent::OpenConfigurator => Some(Action::OpenConfigurator),
        ToolbarEvent::OpenAbout => Some(Action::OpenAbout),
        ToolbarEvent::ExitOverlay => Some(Action::Exit),
        ToolbarEvent::OpenCommandPalette => Some(Action::ToggleCommandPalette),
        ToolbarEvent::PickScreenColor => Some(Action::PickScreenColor),
        ToolbarEvent::OpenFontPicker => Some(Action::OpenFontPicker),

        // A zero step changes nothing, so it names no action.
        ToolbarEvent::NudgeThickness(_)
        | ToolbarEvent::NudgeMarkerOpacity(_)
        | ToolbarEvent::NudgeFontSize(_) => None,
        // Steps only the next arrow's style; the keyboard action also
        // restyles selected arrows.
        ToolbarEvent::CycleArrowStyle => None,
        // Values chosen in the control itself, and the popups and panels that
        // choose them. The keyboard steps these, if it reaches them at all.
        ToolbarEvent::SetColor(_)
        | ToolbarEvent::EditQuickColor { .. }
        | ToolbarEvent::SetThickness(_)
        | ToolbarEvent::SetMarkerOpacity(_)
        | ToolbarEvent::SetSpotlightMagnification(_)
        | ToolbarEvent::SetPenSmoothing(_)
        | ToolbarEvent::NudgePenSmoothing(_)
        | ToolbarEvent::SetShapeRecognitionSensitivity(_)
        | ToolbarEvent::NudgeShapeRecognitionSensitivity(_)
        | ToolbarEvent::SetFont(_)
        | ToolbarEvent::SetFontBold(_)
        | ToolbarEvent::SetFontSize(_)
        | ToolbarEvent::SetPolygonSides(_)
        | ToolbarEvent::NudgePolygonSides(_)
        | ToolbarEvent::ToggleArrowLabels(_)
        | ToolbarEvent::SetArrowStyle(_)
        | ToolbarEvent::ToggleHighlightToolRing(_)
        | ToolbarEvent::SetUndoDelay(_)
        | ToolbarEvent::SetRedoDelay(_)
        | ToolbarEvent::SetCustomUndoDelay(_)
        | ToolbarEvent::SetCustomRedoDelay(_)
        | ToolbarEvent::SetCustomUndoSteps(_)
        | ToolbarEvent::SetCustomRedoSteps(_)
        | ToolbarEvent::CustomUndo
        | ToolbarEvent::CustomRedo
        | ToolbarEvent::CopyHexColor
        | ToolbarEvent::PasteHexColor
        | ToolbarEvent::EditHexColor
        | ToolbarEvent::OpenColorPickerPopup
        | ToolbarEvent::OpenPrecisionEntry(_)
        | ToolbarEvent::CommitPrecisionEntry { .. }
        | ToolbarEvent::CancelPrecisionEntry
        | ToolbarEvent::AdjustSelectionProperty { .. }
        | ToolbarEvent::RefreshZoomCapture => None,
        // Sessions, runtime-UI recovery, and the toolbar's own chrome, layout,
        // and Settings preferences: toolbar state rather than drawing actions.
        ToolbarEvent::OpenSession
        | ToolbarEvent::OpenRecentSession(_)
        | ToolbarEvent::OpenHomeSession
        | ToolbarEvent::SaveSessionAs
        | ToolbarEvent::SaveSessionAsConfirm(_)
        | ToolbarEvent::SaveSessionAsCancel
        | ToolbarEvent::SessionInfo
        | ToolbarEvent::ClearSession
        | ToolbarEvent::OpenConfigFile
        | ToolbarEvent::RequestRuntimeUiReset
        | ToolbarEvent::ConfirmUnsupportedRuntimeUiReset
        | ToolbarEvent::CancelUnsupportedRuntimeUiReset
        | ToolbarEvent::RetryRuntimeUiPersistence
        | ToolbarEvent::DiscardPendingRuntimeUiAndAdoptDisk
        | ToolbarEvent::RequestPreserveInvalidRuntimeUiReset
        | ToolbarEvent::ConfirmPreserveInvalidRuntimeUiReset
        | ToolbarEvent::CancelPreserveInvalidRuntimeUiReset
        | ToolbarEvent::CancelRuntimeUiRecovery
        | ToolbarEvent::ToggleInputHud(_)
        | ToolbarEvent::ToggleCustomSection(_)
        | ToolbarEvent::ToggleDelaySliders(_)
        | ToolbarEvent::ToggleTopOverflow(_)
        | ToolbarEvent::ToggleSessionPopover(_)
        | ToolbarEvent::ToggleSettingsPopover(_)
        | ToolbarEvent::ToggleCanvasPopover(_)
        | ToolbarEvent::ScrollTopPopover(_)
        | ToolbarEvent::SetTopMinimized(_)
        | ToolbarEvent::SetTopDisplayMode(_)
        | ToolbarEvent::CloseTopToolbar
        | ToolbarEvent::PinTopToolbar(_)
        | ToolbarEvent::ToggleIconMode(_)
        | ToolbarEvent::ToggleMoreColors(_)
        | ToolbarEvent::ToggleActionsSection(_)
        | ToolbarEvent::ToggleActionsAdvanced(_)
        | ToolbarEvent::ToggleZoomActions(_)
        | ToolbarEvent::TogglePagesSection(_)
        | ToolbarEvent::ToggleBoardsSection(_)
        | ToolbarEvent::TogglePresets(_)
        | ToolbarEvent::ToggleStepSection(_)
        | ToolbarEvent::ToggleTextControls(_)
        | ToolbarEvent::ToggleContextAwareUi(_)
        | ToolbarEvent::TogglePresetToasts(_)
        | ToolbarEvent::ToggleIdleFade(_)
        | ToolbarEvent::ToggleToolPreview(_)
        | ToolbarEvent::ToggleStatusBar(_)
        | ToolbarEvent::SetStatusBarInteractive(_)
        | ToolbarEvent::SetStatusBarItemVisible(_, _)
        | ToolbarEvent::ToggleStatusBoardBadge(_)
        | ToolbarEvent::ToggleStatusPageBadge(_)
        | ToolbarEvent::ToggleFloatingBadgeAlways(_)
        | ToolbarEvent::SetToolbarLayoutMode(_)
        | ToolbarEvent::ToggleLayoutMenu(_)
        | ToolbarEvent::TogglePenFeelPanel(_)
        | ToolbarEvent::ToggleArrowStyleMenu(_)
        | ToolbarEvent::SetToolbarItemHidden(_, _)
        | ToolbarEvent::MoveToolbarItem { .. }
        | ToolbarEvent::StartToolbarItemDrag { .. }
        | ToolbarEvent::DragToolbarItemOver { .. }
        | ToolbarEvent::ResetToolbarItemOrder(_)
        | ToolbarEvent::ResetToolbarItemHiddenOverrides
        | ToolbarEvent::SetToolbarItemCustomizationOpen(_)
        | ToolbarEvent::SetToolbarItemCustomizationGroup(_)
        | ToolbarEvent::SetStatusBarContentsOpen(_)
        | ToolbarEvent::SetSettingsDetailsOpen(_)
        | ToolbarEvent::ToggleShapePicker(_)
        | ToolbarEvent::MoveTopToolbar { .. } => None,
    }
}

pub(crate) fn short_label_for_event(
    event: &ToolbarEvent,
    frozen_active: bool,
    zoom_locked: bool,
    fallback: &'static str,
) -> &'static str {
    match event {
        ToolbarEvent::ToggleFreeze if frozen_active => "Unfreeze",
        ToolbarEvent::ToggleZoomLock if zoom_locked => "Unlock Zoom",
        _ => action_for_event(event)
            .map(action_short_label)
            .unwrap_or(fallback),
    }
}

pub(crate) fn tooltip_label_for_event(
    event: &ToolbarEvent,
    frozen_active: bool,
    zoom_locked: bool,
    fallback: &'static str,
) -> &'static str {
    match event {
        ToolbarEvent::ToggleFreeze if frozen_active => "Unfreeze",
        ToolbarEvent::ToggleZoomLock if zoom_locked => "Unlock Zoom",
        _ => action_for_event(event)
            .map(action_label)
            .unwrap_or(fallback),
    }
}

pub(crate) fn action_for_tool(tool: Tool) -> Option<Action> {
    tool.action()
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

/// The popovers an event operates inside, and so must not dismiss.
///
/// One table replacing five per-popover exclusion lists. A control declares
/// where it lives and every dismissal rule follows, instead of each popover
/// separately remembering not to close on it - which is how a control added
/// to a popover could close that popover the first time it was used.
///
/// A slice rather than one owner because some controls genuinely belong to
/// two: the shapes picker is anchored beside the overflow flyout so their
/// toggles spare each other, the three overflow-anchored menus spare each
/// other's toggles and the shared scrollbar (switching between them is one
/// gesture), and Open Configurator is reachable from both Session and
/// Settings.
///
/// Exhaustive, with the events hosted in no popover listed too, so a new
/// event has to declare where it lives rather than fall through to closing
/// everything.
pub(crate) fn popovers_for_event(event: &ToolbarEvent) -> &'static [ToolbarPopover] {
    use ToolbarPopover as P;
    const TOP_MENUS: &[ToolbarPopover] = &[P::Canvas, P::Session, P::Settings];

    match event {
        // The overflow flyout and the shapes picker anchored beside it.
        ToolbarEvent::ToggleTopOverflow(_) | ToolbarEvent::ToggleShapePicker(_) => {
            &[P::TopOverflow, P::ShapePicker]
        }
        // Choosing a preset is the menu's purpose, so it closes the menu;
        // only the toggle itself spares it.
        ToolbarEvent::ToggleLayoutMenu(_) => &[P::LayoutMenu],
        // Likewise choosing an arrow style (`SetArrowStyle`) closes its menu.
        ToolbarEvent::ToggleArrowStyleMenu(_) => &[P::ArrowStyleMenu],
        // The Pen feel panel is adjusted in place: its meters change the
        // levels live and the panel stays up until dismissed.
        ToolbarEvent::TogglePenFeelPanel(_)
        | ToolbarEvent::SetPenSmoothing(_)
        | ToolbarEvent::NudgePenSmoothing(_)
        | ToolbarEvent::SetShapeRecognitionSensitivity(_)
        | ToolbarEvent::NudgeShapeRecognitionSensitivity(_) => &[P::PenFeel],
        // Shapes hosts its own inline options. `SetPolygonSides` is the
        // absolute form of the sides stepper there.
        ToolbarEvent::ToggleFill(_)
        | ToolbarEvent::NudgePolygonSides(_)
        | ToolbarEvent::SetPolygonSides(_) => &[P::ShapePicker],

        ToolbarEvent::OpenPrecisionEntry(_)
        | ToolbarEvent::CommitPrecisionEntry { .. }
        | ToolbarEvent::CancelPrecisionEntry => &[P::PrecisionEntry],

        // Switching between the menus is one gesture, and the switch itself
        // is what closes the previous one; the scrollbar is shared.
        ToolbarEvent::ToggleCanvasPopover(_)
        | ToolbarEvent::ToggleSessionPopover(_)
        | ToolbarEvent::ToggleSettingsPopover(_)
        | ToolbarEvent::ScrollTopPopover(_) => TOP_MENUS,

        // Reachable from both menus that offer it.
        ToolbarEvent::OpenConfigurator => &[P::Session, P::Settings],

        // Canvas hosts the board/page/zoom/advanced/step controls.
        ToolbarEvent::BoardPrev
        | ToolbarEvent::BoardNext
        | ToolbarEvent::BoardNew
        | ToolbarEvent::BoardDuplicate
        | ToolbarEvent::BoardDelete
        | ToolbarEvent::PagePrev
        | ToolbarEvent::PageNext
        | ToolbarEvent::PageNew
        | ToolbarEvent::PageDuplicate
        | ToolbarEvent::PageDelete
        | ToolbarEvent::ZoomIn
        | ToolbarEvent::ZoomOut
        | ToolbarEvent::ResetZoom
        | ToolbarEvent::ToggleZoomLock
        | ToolbarEvent::Undo
        | ToolbarEvent::Redo
        | ToolbarEvent::ClearCanvas { .. }
        | ToolbarEvent::UndoAll
        | ToolbarEvent::RedoAll
        | ToolbarEvent::UndoAllDelayed
        | ToolbarEvent::RedoAllDelayed
        | ToolbarEvent::ToggleFreeze
        | ToolbarEvent::ToggleCustomSection(_)
        | ToolbarEvent::ToggleDelaySliders(_)
        | ToolbarEvent::SetCustomUndoSteps(_)
        | ToolbarEvent::SetCustomRedoSteps(_)
        | ToolbarEvent::CustomUndo
        | ToolbarEvent::CustomRedo
        | ToolbarEvent::SetCustomUndoDelay(_)
        | ToolbarEvent::SetCustomRedoDelay(_)
        | ToolbarEvent::SetUndoDelay(_)
        | ToolbarEvent::SetRedoDelay(_) => &[P::Canvas],

        // Session hosts the session controls.
        ToolbarEvent::OpenSession
        | ToolbarEvent::OpenRecentSession(_)
        | ToolbarEvent::OpenHomeSession
        | ToolbarEvent::SaveSessionAs
        | ToolbarEvent::SaveSessionAsConfirm(_)
        | ToolbarEvent::SaveSessionAsCancel
        | ToolbarEvent::SessionInfo
        | ToolbarEvent::ClearSession => &[P::Session],

        // Settings hosts the full Settings pane, including its customization
        // sub-panel.
        ToolbarEvent::SetToolbarLayoutMode(_)
        | ToolbarEvent::ToggleContextAwareUi(_)
        | ToolbarEvent::ToggleIconMode(_)
        | ToolbarEvent::ToggleTextControls(_)
        | ToolbarEvent::ToggleStatusBar(_)
        | ToolbarEvent::SetStatusBarInteractive(_)
        | ToolbarEvent::SetStatusBarItemVisible(_, _)
        | ToolbarEvent::ToggleStatusBoardBadge(_)
        | ToolbarEvent::ToggleStatusPageBadge(_)
        | ToolbarEvent::ToggleFloatingBadgeAlways(_)
        | ToolbarEvent::TogglePresetToasts(_)
        | ToolbarEvent::ToggleIdleFade(_)
        | ToolbarEvent::ToggleInputHud(_)
        | ToolbarEvent::TogglePresets(_)
        | ToolbarEvent::ToggleActionsSection(_)
        | ToolbarEvent::ToggleZoomActions(_)
        | ToolbarEvent::ToggleActionsAdvanced(_)
        | ToolbarEvent::ToggleBoardsSection(_)
        | ToolbarEvent::TogglePagesSection(_)
        | ToolbarEvent::ToggleStepSection(_)
        | ToolbarEvent::SetToolbarItemCustomizationOpen(_)
        | ToolbarEvent::SetToolbarItemCustomizationGroup(_)
        | ToolbarEvent::SetStatusBarContentsOpen(_)
        | ToolbarEvent::SetSettingsDetailsOpen(_)
        | ToolbarEvent::SetToolbarItemHidden(_, _)
        | ToolbarEvent::MoveToolbarItem { .. }
        | ToolbarEvent::StartToolbarItemDrag { .. }
        | ToolbarEvent::DragToolbarItemOver { .. }
        | ToolbarEvent::ResetToolbarItemOrder(_)
        | ToolbarEvent::ResetToolbarItemHiddenOverrides
        | ToolbarEvent::OpenCommandPalette
        | ToolbarEvent::OpenConfigFile
        | ToolbarEvent::OpenAbout
        | ToolbarEvent::RequestRuntimeUiReset
        | ToolbarEvent::ConfirmUnsupportedRuntimeUiReset
        | ToolbarEvent::CancelUnsupportedRuntimeUiReset
        | ToolbarEvent::RetryRuntimeUiPersistence
        | ToolbarEvent::DiscardPendingRuntimeUiAndAdoptDisk
        | ToolbarEvent::RequestPreserveInvalidRuntimeUiReset
        | ToolbarEvent::ConfirmPreserveInvalidRuntimeUiReset
        | ToolbarEvent::CancelPreserveInvalidRuntimeUiReset
        | ToolbarEvent::CancelRuntimeUiRecovery => &[P::Settings],

        // Hosted in no popover: each of these dismisses whatever is open.
        //
        // The board picker sits in the Canvas popover's Boards row, but it
        // opens a modal over the overlay, so the popover that launched it
        // closes; opening the command palette closes Settings the same way.
        ToolbarEvent::ToggleBoardPicker | ToolbarEvent::BoardRename => &[],
        // The style pill, the tool strip, and the strip's own chrome. A strip
        // control dropped into the overflow flyout closes it when used, like
        // any menu item, and choosing a shape is the shapes popover's purpose.
        ToolbarEvent::SelectTool(_)
        | ToolbarEvent::SetColor(_)
        | ToolbarEvent::SetQuickColor { .. }
        | ToolbarEvent::EditQuickColor { .. }
        | ToolbarEvent::SetThickness(_)
        | ToolbarEvent::NudgeThickness(_)
        | ToolbarEvent::SetMarkerOpacity(_)
        | ToolbarEvent::NudgeMarkerOpacity(_)
        | ToolbarEvent::SetSpotlightMagnification(_)
        | ToolbarEvent::SetEraserMode(_)
        | ToolbarEvent::SetFont(_)
        | ToolbarEvent::SetFontBold(_)
        | ToolbarEvent::OpenFontPicker
        | ToolbarEvent::SetFontSize(_)
        | ToolbarEvent::NudgeFontSize(_)
        | ToolbarEvent::ToggleArrowLabels(_)
        | ToolbarEvent::CycleArrowStyle
        | ToolbarEvent::SetArrowStyle(_)
        | ToolbarEvent::ResetArrowLabelCounter
        | ToolbarEvent::ResetStepMarkerCounter
        | ToolbarEvent::CopyHexColor
        | ToolbarEvent::PasteHexColor
        | ToolbarEvent::EditHexColor
        | ToolbarEvent::OpenColorPickerPopup
        | ToolbarEvent::AdjustSelectionProperty { .. }
        | ToolbarEvent::PickScreenColor
        | ToolbarEvent::CaptureScreenshot
        | ToolbarEvent::CopyTextFromScreen
        | ToolbarEvent::EnterTextMode
        | ToolbarEvent::EnterStickyNoteMode
        | ToolbarEvent::ToggleAllHighlight(_)
        | ToolbarEvent::ToggleHighlightToolRing(_)
        | ToolbarEvent::ApplyPreset(_)
        | ToolbarEvent::SavePreset(_)
        | ToolbarEvent::ClearPreset(_)
        | ToolbarEvent::ExitOverlay
        | ToolbarEvent::SetTopMinimized(_)
        | ToolbarEvent::SetTopDisplayMode(_)
        | ToolbarEvent::CloseTopToolbar
        | ToolbarEvent::PinTopToolbar(_)
        | ToolbarEvent::ToggleMoreColors(_)
        | ToolbarEvent::ToggleToolPreview(_)
        | ToolbarEvent::RefreshZoomCapture
        | ToolbarEvent::MoveTopToolbar { .. } => &[],
    }
}

fn persistence_for_event(event: &ToolbarEvent) -> ToolbarPersistence {
    use ToolbarRuntimeUiPersistenceTarget as Runtime;
    match event {
        ToolbarEvent::PinTopToolbar(_) => ToolbarPersistence::RuntimeUi(Runtime::TopPinned),
        ToolbarEvent::SetTopMinimized(_) | ToolbarEvent::CloseTopToolbar => {
            ToolbarPersistence::RuntimeUi(Runtime::TopMinimized)
        }
        ToolbarEvent::SetTopDisplayMode(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::TopDisplayMode)
        }
        // Status-bar content is chrome the user arranges from the overlay, so
        // it persists the same way the toolbars do: as a runtime override
        // layered over the configured value, never by writing config.toml.
        ToolbarEvent::ToggleActionsSection(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::NamedSection(ToolbarSectionFlag::Actions))
        }
        ToolbarEvent::ToggleActionsAdvanced(_) => ToolbarPersistence::RuntimeUi(
            Runtime::NamedSection(ToolbarSectionFlag::ActionsAdvanced),
        ),
        ToolbarEvent::ToggleZoomActions(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::NamedSection(ToolbarSectionFlag::ZoomActions))
        }
        ToolbarEvent::TogglePagesSection(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::NamedSection(ToolbarSectionFlag::Pages))
        }
        ToolbarEvent::ToggleBoardsSection(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::NamedSection(ToolbarSectionFlag::Boards))
        }
        ToolbarEvent::TogglePresets(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::NamedSection(ToolbarSectionFlag::Presets))
        }
        ToolbarEvent::ToggleStepSection(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::NamedSection(ToolbarSectionFlag::StepSection))
        }
        ToolbarEvent::ToggleTextControls(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::NamedSection(ToolbarSectionFlag::TextControls))
        }
        // A section row in the customization list is that section's
        // visibility, not an individual item override.
        ToolbarEvent::SetToolbarItemHidden(id, hidden) => {
            if let Some(flag) = section_flag_for_item(*id) {
                ToolbarPersistence::RuntimeUi(Runtime::NamedSection(flag))
            } else {
                ToolbarPersistence::RuntimeUi(Runtime::ItemVisibility {
                    id: *id,
                    setting: if *hidden {
                        ToolbarItemVisibilitySetting::Hidden
                    } else {
                        ToolbarItemVisibilitySetting::Default
                    },
                })
            }
        }
        ToolbarEvent::ToggleStatusBar(_) => ToolbarPersistence::RuntimeUi(Runtime::StatusBar),
        ToolbarEvent::ToggleStatusBoardBadge(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::StatusBoardBadge)
        }
        ToolbarEvent::ToggleStatusPageBadge(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::StatusPageBadge)
        }
        ToolbarEvent::ToggleFloatingBadgeAlways(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::FloatingBadgeAlways)
        }
        ToolbarEvent::ToggleIconMode(_) => ToolbarPersistence::RuntimeUi(Runtime::ToolbarIcons),
        ToolbarEvent::ToggleMoreColors(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::ToolbarMoreColors)
        }
        ToolbarEvent::ToggleContextAwareUi(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::ToolbarContextAwareUi)
        }
        ToolbarEvent::TogglePresetToasts(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::ToolbarPresetToasts)
        }
        ToolbarEvent::ToggleIdleFade(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::ToolbarIdleFade)
        }
        ToolbarEvent::ToggleToolPreview(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::ToolbarToolPreview)
        }
        ToolbarEvent::ToggleDelaySliders(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::ToolbarDelaySliders)
        }
        ToolbarEvent::ToggleCustomSection(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::HistoryCustomSection)
        }
        ToolbarEvent::ToggleInputHud(_) => ToolbarPersistence::RuntimeUi(Runtime::InputHud),
        ToolbarEvent::SetStatusBarInteractive(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::StatusBarInteractive)
        }
        ToolbarEvent::SetStatusBarItemVisible(item, _) => {
            ToolbarPersistence::RuntimeUi(Runtime::StatusBarItem(*item))
        }
        ToolbarEvent::MoveToolbarItem { group, .. }
        | ToolbarEvent::StartToolbarItemDrag { group, .. }
        | ToolbarEvent::ResetToolbarItemOrder(group) => {
            ToolbarPersistence::RuntimeUi(Runtime::ItemOrder(*group))
        }
        ToolbarEvent::ResetToolbarItemHiddenOverrides => {
            ToolbarPersistence::RuntimeUi(Runtime::ResetItemVisibility)
        }
        ToolbarEvent::SetToolbarLayoutMode(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::LayoutMode)
        }
        // Picking the highlight tool switches the click highlight on as a
        // side effect, so it persists the same choice the explicit toggles do.
        ToolbarEvent::SelectTool(Tool::Highlight)
        | ToolbarEvent::ToggleAllHighlight(_)
        | ToolbarEvent::ToggleHighlightToolRing(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::ClickHighlight)
        }
        // Ephemeral below: drawing state that belongs to the session file, or
        // one-shot actions that persist nothing of their own.
        ToolbarEvent::SelectTool(_)
        | ToolbarEvent::SetColor(_)
        | ToolbarEvent::SetQuickColor { .. }
        // Opening the recolor popup persists nothing; accepting it writes the
        // palette through the popup's own pending-edit path.
        | ToolbarEvent::EditQuickColor { .. }
        | ToolbarEvent::SetThickness(_)
        | ToolbarEvent::NudgeThickness(_)
        | ToolbarEvent::SetMarkerOpacity(_)
        | ToolbarEvent::NudgeMarkerOpacity(_)
        | ToolbarEvent::SetSpotlightMagnification(_)
        | ToolbarEvent::SetPenSmoothing(_)
        | ToolbarEvent::NudgePenSmoothing(_)
        | ToolbarEvent::SetShapeRecognitionSensitivity(_)
        | ToolbarEvent::NudgeShapeRecognitionSensitivity(_)
        | ToolbarEvent::SetEraserMode(_)
        | ToolbarEvent::SetFont(_)
        | ToolbarEvent::SetFontBold(_)
        | ToolbarEvent::SetFontSize(_)
        | ToolbarEvent::NudgeFontSize(_)
        | ToolbarEvent::ToggleFill(_)
        | ToolbarEvent::SetPolygonSides(_)
        | ToolbarEvent::NudgePolygonSides(_)
        | ToolbarEvent::ToggleArrowLabels(_)
        | ToolbarEvent::CycleArrowStyle
        | ToolbarEvent::SetArrowStyle(_)
        | ToolbarEvent::ResetArrowLabelCounter
        | ToolbarEvent::ResetStepMarkerCounter
        | ToolbarEvent::SetUndoDelay(_)
        | ToolbarEvent::SetRedoDelay(_)
        | ToolbarEvent::UndoAll
        | ToolbarEvent::RedoAll
        | ToolbarEvent::UndoAllDelayed
        | ToolbarEvent::RedoAllDelayed
        | ToolbarEvent::Undo
        | ToolbarEvent::Redo
        | ToolbarEvent::ClearCanvas { .. }
        | ToolbarEvent::CaptureScreenshot
        | ToolbarEvent::CopyTextFromScreen
        | ToolbarEvent::PagePrev
        | ToolbarEvent::PageNext
        | ToolbarEvent::PageNew
        | ToolbarEvent::PageDuplicate
        | ToolbarEvent::PageDelete
        | ToolbarEvent::BoardPrev
        | ToolbarEvent::BoardNext
        | ToolbarEvent::BoardNew
        | ToolbarEvent::BoardDelete
        | ToolbarEvent::BoardDuplicate
        | ToolbarEvent::BoardRename
        | ToolbarEvent::ToggleBoardPicker
        | ToolbarEvent::EnterTextMode
        | ToolbarEvent::EnterStickyNoteMode
        | ToolbarEvent::ToggleFreeze
        | ToolbarEvent::ZoomIn
        | ToolbarEvent::ZoomOut
        | ToolbarEvent::ResetZoom
        | ToolbarEvent::ToggleZoomLock
        | ToolbarEvent::RefreshZoomCapture
        | ToolbarEvent::ApplyPreset(_)
        | ToolbarEvent::SavePreset(_)
        | ToolbarEvent::ClearPreset(_)
        | ToolbarEvent::OpenSession
        | ToolbarEvent::OpenRecentSession(_)
        | ToolbarEvent::OpenHomeSession
        | ToolbarEvent::SaveSessionAs
        | ToolbarEvent::SaveSessionAsConfirm(_)
        | ToolbarEvent::SaveSessionAsCancel
        | ToolbarEvent::SessionInfo
        | ToolbarEvent::ClearSession
        | ToolbarEvent::OpenConfigurator
        | ToolbarEvent::OpenConfigFile
        | ToolbarEvent::OpenAbout
        | ToolbarEvent::ExitOverlay
        | ToolbarEvent::RequestRuntimeUiReset
        | ToolbarEvent::ConfirmUnsupportedRuntimeUiReset
        | ToolbarEvent::CancelUnsupportedRuntimeUiReset
        | ToolbarEvent::RetryRuntimeUiPersistence
        | ToolbarEvent::DiscardPendingRuntimeUiAndAdoptDisk
        | ToolbarEvent::RequestPreserveInvalidRuntimeUiReset
        | ToolbarEvent::ConfirmPreserveInvalidRuntimeUiReset
        | ToolbarEvent::CancelPreserveInvalidRuntimeUiReset
        | ToolbarEvent::CancelRuntimeUiRecovery
        | ToolbarEvent::OpenCommandPalette
        | ToolbarEvent::SetCustomUndoDelay(_)
        | ToolbarEvent::SetCustomRedoDelay(_)
        | ToolbarEvent::SetCustomUndoSteps(_)
        | ToolbarEvent::SetCustomRedoSteps(_)
        | ToolbarEvent::CustomUndo
        | ToolbarEvent::CustomRedo
        | ToolbarEvent::ToggleTopOverflow(_)
        | ToolbarEvent::ToggleLayoutMenu(_)
        | ToolbarEvent::TogglePenFeelPanel(_)
        | ToolbarEvent::ToggleArrowStyleMenu(_)
        | ToolbarEvent::ToggleSessionPopover(_)
        | ToolbarEvent::ToggleSettingsPopover(_)
        | ToolbarEvent::ToggleCanvasPopover(_)
        | ToolbarEvent::ScrollTopPopover(_)
        | ToolbarEvent::CopyHexColor
        | ToolbarEvent::PasteHexColor
        | ToolbarEvent::EditHexColor
        | ToolbarEvent::OpenColorPickerPopup
        | ToolbarEvent::OpenFontPicker
        | ToolbarEvent::OpenPrecisionEntry(_)
        | ToolbarEvent::CommitPrecisionEntry { .. }
        | ToolbarEvent::CancelPrecisionEntry
        | ToolbarEvent::AdjustSelectionProperty { .. }
        | ToolbarEvent::PickScreenColor
        | ToolbarEvent::DragToolbarItemOver { .. }
        | ToolbarEvent::SetToolbarItemCustomizationOpen(_)
        | ToolbarEvent::SetToolbarItemCustomizationGroup(_)
        | ToolbarEvent::SetStatusBarContentsOpen(_)
        | ToolbarEvent::SetSettingsDetailsOpen(_)
        | ToolbarEvent::ToggleShapePicker(_)
        | ToolbarEvent::MoveTopToolbar { .. } => ToolbarPersistence::Ephemeral,
    }
}

fn backend_route_for_event(event: &ToolbarEvent) -> ToolbarBackendRoute {
    match event {
        ToolbarEvent::MoveTopToolbar { .. } => ToolbarBackendRoute::MoveTopToolbar,
        _ => ToolbarBackendRoute::ApplyToInput,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::color::RED;

    #[test]
    fn next_arrow_style_event_has_no_selection_aware_action_equivalent() {
        assert_eq!(action_for_event(&ToolbarEvent::CycleArrowStyle), None);
    }

    #[test]
    fn duplicate_quick_colors_keep_the_clicked_binding_identity() {
        let first = ToolbarEvent::SetQuickColor {
            color: RED,
            action: Some(Action::SetColorRed),
            index: 0,
        };
        let duplicate = ToolbarEvent::SetQuickColor {
            color: RED,
            action: Some(Action::SetColorGreen),
            index: 1,
        };

        assert_eq!(action_for_event(&first), Some(Action::SetColorRed));
        assert_eq!(action_for_event(&duplicate), Some(Action::SetColorGreen));
    }

    #[test]
    fn unbound_quick_color_slot_stays_unbound() {
        assert_eq!(
            action_for_event(&ToolbarEvent::SetQuickColor {
                color: RED,
                action: None,
                index: 9,
            }),
            None
        );
    }

    #[test]
    fn hiding_a_section_row_persists_as_section_visibility() {
        let id = ToolbarSectionFlag::Actions.item_id();
        assert_eq!(
            persistence_for_event(&ToolbarEvent::SetToolbarItemHidden(id, true)),
            ToolbarPersistence::RuntimeUi(ToolbarRuntimeUiPersistenceTarget::NamedSection(
                ToolbarSectionFlag::Actions
            )),
        );
        assert_eq!(
            persistence_for_event(&ToolbarEvent::SetToolbarItemHidden(id, false)),
            ToolbarPersistence::RuntimeUi(ToolbarRuntimeUiPersistenceTarget::NamedSection(
                ToolbarSectionFlag::Actions
            )),
        );
    }

    #[test]
    fn hiding_a_tool_item_persists_as_item_visibility() {
        let id = "top.tool.pen".parse::<ToolbarItemId>().expect("item id");
        assert_eq!(
            persistence_for_event(&ToolbarEvent::SetToolbarItemHidden(id, true)),
            ToolbarPersistence::RuntimeUi(ToolbarRuntimeUiPersistenceTarget::ItemVisibility {
                id,
                setting: ToolbarItemVisibilitySetting::Hidden,
            }),
        );
        assert_eq!(
            persistence_for_event(&ToolbarEvent::SetToolbarItemHidden(id, false)),
            ToolbarPersistence::RuntimeUi(ToolbarRuntimeUiPersistenceTarget::ItemVisibility {
                id,
                setting: ToolbarItemVisibilitySetting::Default,
            }),
        );
    }
}

#[cfg(test)]
mod popover_affinity_tests {
    use super::*;

    /// Every event that starts an item drag must carry the order group its
    /// move is persisted under. The backend used to assert this with
    /// `unreachable!`, taking the overlay down if the two ever disagreed;
    /// it now refuses the drag, so the pairing is pinned here instead.
    #[test]
    fn an_item_drag_start_names_the_group_it_reorders() {
        use crate::config::{ToolbarItemId, ToolbarItemOrderGroup};

        for group in [
            ToolbarItemOrderGroup::TopTools,
            ToolbarItemOrderGroup::TopControls,
        ] {
            let event = ToolbarEvent::StartToolbarItemDrag {
                group,
                id: "top.tool.pen".parse::<ToolbarItemId>().expect("item id"),
            };
            assert_eq!(
                persistence_for_event(&event),
                ToolbarPersistence::RuntimeUi(ToolbarRuntimeUiPersistenceTarget::ItemOrder(group)),
                "a drag start must persist under the group it reorders"
            );
        }
    }

    /// The controls a popover hosts keep it open; anything else closes it.
    #[test]
    fn a_popover_survives_its_own_controls_and_nothing_elses() {
        use ToolbarPopover as P;

        // Hosted controls.
        assert!(popovers_for_event(&ToolbarEvent::ZoomIn).contains(&P::Canvas));
        for event in [
            ToolbarEvent::Undo,
            ToolbarEvent::Redo,
            ToolbarEvent::ClearCanvas { instant: false },
        ] {
            assert!(
                popovers_for_event(&event).contains(&P::Canvas),
                "Canvas action {event:?} must keep its owning popover open"
            );
        }
        assert!(popovers_for_event(&ToolbarEvent::SessionInfo).contains(&P::Session));
        assert!(popovers_for_event(&ToolbarEvent::ToggleIconMode(true)).contains(&P::Settings));
        assert!(popovers_for_event(&ToolbarEvent::ToggleFill(true)).contains(&P::ShapePicker));

        // Foreign controls close it.
        assert!(!popovers_for_event(&ToolbarEvent::ZoomIn).contains(&P::Settings));
        assert!(!popovers_for_event(&ToolbarEvent::SessionInfo).contains(&P::Canvas));

        // The pairs that genuinely belong to two popovers.
        let configurator = popovers_for_event(&ToolbarEvent::OpenConfigurator);
        assert!(configurator.contains(&P::Session) && configurator.contains(&P::Settings));
        let shapes = popovers_for_event(&ToolbarEvent::ToggleShapePicker(true));
        assert!(shapes.contains(&P::ShapePicker) && shapes.contains(&P::TopOverflow));
    }

    /// Both forms of the polygon-sides option live in the shapes popover's
    /// option row, like Fill beside them.
    #[test]
    fn the_shapes_popover_spares_its_option_rows() {
        use ToolbarPopover as P;

        for event in [
            ToolbarEvent::ToggleFill(true),
            ToolbarEvent::NudgePolygonSides(1),
            ToolbarEvent::SetPolygonSides(6),
        ] {
            assert_eq!(popovers_for_event(&event), &[P::ShapePicker], "{event:?}");
        }
    }

    /// Every Canvas popover control keeps Canvas open, except the board
    /// picker: it opens a modal over the overlay, so the popover that
    /// launched it closes.
    #[test]
    fn the_canvas_popover_spares_every_control_it_hosts_but_the_board_picker() {
        use super::super::{
            toolbar_actions_model_for_popover, toolbar_advanced_group_for_popover,
            toolbar_boards_model_for_popover, toolbar_pages_model_for_popover,
            toolbar_zoom_group_for_popover,
        };
        use crate::input::state::test_support::make_test_input_state;
        use crate::ui::toolbar::{ToolbarBindingHints, ToolbarSnapshot};
        use ToolbarPopover as P;

        let mut snapshot = ToolbarSnapshot::from_input_with_bindings(
            &make_test_input_state(),
            ToolbarBindingHints::default(),
        );
        snapshot.show_actions_section = true;
        snapshot.show_actions_advanced = true;
        snapshot.show_boards_section = true;
        snapshot.show_pages_section = true;
        snapshot.show_zoom_actions = true;
        snapshot.delay_actions_enabled = true;
        let groups = [
            toolbar_actions_model_for_popover(&snapshot),
            toolbar_advanced_group_for_popover(&snapshot),
            toolbar_boards_model_for_popover(&snapshot),
            toolbar_pages_model_for_popover(&snapshot),
            toolbar_zoom_group_for_popover(&snapshot),
        ];

        let events: Vec<_> = groups
            .iter()
            .flatten()
            .flat_map(|group| group.buttons.iter().map(|button| &button.event))
            .collect();
        assert!(events.contains(&&ToolbarEvent::ToggleBoardPicker));
        for event in events {
            let spared = popovers_for_event(event).contains(&P::Canvas);
            assert_eq!(
                spared,
                *event != ToolbarEvent::ToggleBoardPicker,
                "{event:?}"
            );
        }
    }

    /// Clicks and wheel steps inside the Pen feel panel adjust the levels
    /// with the panel still open; anything else closes it.
    #[test]
    fn the_pen_feel_panel_spares_its_own_controls() {
        use ToolbarPopover as P;

        for event in [
            ToolbarEvent::TogglePenFeelPanel(false),
            ToolbarEvent::SetPenSmoothing(4),
            ToolbarEvent::NudgePenSmoothing(2),
            ToolbarEvent::SetShapeRecognitionSensitivity(1),
            ToolbarEvent::NudgeShapeRecognitionSensitivity(-1),
        ] {
            assert_eq!(popovers_for_event(&event), &[P::PenFeel], "{event:?}");
        }
        for event in [
            ToolbarEvent::SelectTool(Tool::Pen),
            ToolbarEvent::ToggleLayoutMenu(true),
            ToolbarEvent::OpenColorPickerPopup,
            ToolbarEvent::SetThickness(4.0),
        ] {
            assert!(
                !popovers_for_event(&event).contains(&P::PenFeel),
                "{event:?} closes the panel"
            );
        }
        assert_eq!(
            persistence_for_event(&ToolbarEvent::TogglePenFeelPanel(true)),
            ToolbarPersistence::Ephemeral
        );
    }

    /// Choosing a style is the arrow menu's purpose, so it closes the menu;
    /// only the toggle spares it.
    #[test]
    fn choosing_an_arrow_style_closes_its_menu() {
        use ToolbarPopover as P;

        assert_eq!(
            popovers_for_event(&ToolbarEvent::ToggleArrowStyleMenu(false)),
            &[P::ArrowStyleMenu]
        );
        for event in [
            ToolbarEvent::SetArrowStyle(crate::draw::ArrowStyle::Pointy),
            ToolbarEvent::CycleArrowStyle,
            ToolbarEvent::TogglePenFeelPanel(true),
        ] {
            assert!(
                !popovers_for_event(&event).contains(&P::ArrowStyleMenu),
                "{event:?} closes the menu"
            );
        }
        assert_eq!(
            persistence_for_event(&ToolbarEvent::ToggleArrowStyleMenu(true)),
            ToolbarPersistence::Ephemeral
        );
        assert_eq!(
            persistence_for_event(&ToolbarEvent::SetArrowStyle(
                crate::draw::ArrowStyle::Curved
            )),
            persistence_for_event(&ToolbarEvent::CycleArrowStyle),
            "picking a style persists like cycling to it"
        );
    }
}
