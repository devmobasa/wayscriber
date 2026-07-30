use crate::config::{
    Action, ToolbarItemId, ToolbarItemOrderGroup, ToolbarItemVisibilitySetting, action_label,
    action_short_label, section_flag_for_item,
};
use crate::input::Tool;

use super::super::{ToolbarEvent, ToolbarSideSection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToolbarEventPolicy {
    pub(crate) persistence: ToolbarPersistence,
    pub(crate) backend_route: ToolbarBackendRoute,
    pub(crate) pre_apply_effects: Vec<ToolbarPreApplyEffect>,
    pub(crate) tablet_thickness_sensitive: bool,
}

impl ToolbarEventPolicy {
    pub(crate) fn for_event(event: &ToolbarEvent) -> Self {
        Self {
            persistence: persistence_for_event(event),
            backend_route: backend_route_for_event(event),
            pre_apply_effects: pre_apply_effects_for_event(event),
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
/// one of them. An event that changes an authored preference — icons, section
/// visibility, layout mode, status bar, badges, click highlight, the input HUD
/// — is `Ephemeral`: it changes the effective value for this run and the next
/// start reads the configured one back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarPersistence {
    Ephemeral,
    RuntimeUi(ToolbarRuntimeUiPersistenceTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarRuntimeUiPersistenceTarget {
    TopPinned,
    SidePinned,
    TopMinimized,
    SideMinimized,
    SidePane,
    CollapsedSection(ToolbarSideSection),
    ItemVisibility {
        id: ToolbarItemId,
        setting: ToolbarItemVisibilitySetting,
    },
    ItemOrder(ToolbarItemOrderGroup),
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
    MoveSideToolbar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarPreApplyEffect {
    RecordDrawerHintShown,
}

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
        ToolbarEvent::CaptureScreenshot => Some(Action::CaptureSelection),
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
        ToolbarEvent::OpenCommandPalette => Some(Action::ToggleCommandPalette),
        ToolbarEvent::PickScreenColor => Some(Action::PickScreenColor),
        _ => None,
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

fn persistence_for_event(event: &ToolbarEvent) -> ToolbarPersistence {
    use ToolbarRuntimeUiPersistenceTarget as Runtime;
    match event {
        ToolbarEvent::PinTopToolbar(_) => ToolbarPersistence::RuntimeUi(Runtime::TopPinned),
        ToolbarEvent::PinSideToolbar(_) => ToolbarPersistence::RuntimeUi(Runtime::SidePinned),
        ToolbarEvent::SetSidePane(_) => ToolbarPersistence::RuntimeUi(Runtime::SidePane),
        ToolbarEvent::SetTopMinimized(_) | ToolbarEvent::CloseTopToolbar => {
            ToolbarPersistence::RuntimeUi(Runtime::TopMinimized)
        }
        ToolbarEvent::SetTopDisplayMode(_) => {
            ToolbarPersistence::RuntimeUi(Runtime::TopDisplayMode)
        }
        ToolbarEvent::SetSideMinimized(_) | ToolbarEvent::CloseSideToolbar => {
            ToolbarPersistence::RuntimeUi(Runtime::SideMinimized)
        }
        ToolbarEvent::ToggleSideSectionCollapsed(section, _) => {
            ToolbarPersistence::RuntimeUi(Runtime::CollapsedSection(*section))
        }
        // A section row is an authored preference, not runtime-UI item state:
        // `runtime_seeds_from_config` deliberately grows no seed for it, so
        // hiding one changes the effective config for this run and comes back
        // from `config.toml` on the next start. Every other item keeps its
        // runtime-UI override.
        ToolbarEvent::SetToolbarItemHidden(id, hidden) => {
            if section_flag_for_item(*id).is_some() {
                ToolbarPersistence::Ephemeral
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
        ToolbarEvent::MoveToolbarItem { group, .. }
        | ToolbarEvent::StartToolbarItemDrag { group, .. }
        | ToolbarEvent::ResetToolbarItemOrder(group) => {
            ToolbarPersistence::RuntimeUi(Runtime::ItemOrder(*group))
        }
        ToolbarEvent::ResetToolbarItemHiddenOverrides => {
            ToolbarPersistence::RuntimeUi(Runtime::ResetItemVisibility)
        }
        // Authored preferences below: applying them updates the effective
        // config for this run only (see `ToolbarPersistence`).
        ToolbarEvent::ToggleIconMode(_)
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
        | ToolbarEvent::ToggleToolPreview(_)
        | ToolbarEvent::ToggleDelaySliders(_)
        | ToolbarEvent::SetToolbarLayoutMode(_)
        | ToolbarEvent::ToggleCustomSection(_)
        | ToolbarEvent::ToggleStatusBar(_)
        | ToolbarEvent::SetStatusBarInteractive(_)
        | ToolbarEvent::SetStatusBarItemVisible(_, _)
        | ToolbarEvent::ToggleStatusBoardBadge(_)
        | ToolbarEvent::ToggleStatusPageBadge(_)
        | ToolbarEvent::ToggleFloatingBadgeAlways(_)
        | ToolbarEvent::ToggleAllHighlight(_)
        | ToolbarEvent::ToggleHighlightToolRing(_)
        | ToolbarEvent::ToggleInputHud(_)
        | ToolbarEvent::SelectTool(_)
        | ToolbarEvent::SetColor(_)
        | ToolbarEvent::SetQuickColor { .. }
        // Opening the recolor popup persists nothing; accepting it writes the
        // palette through the popup's own pending-edit path.
        | ToolbarEvent::EditQuickColor { .. }
        | ToolbarEvent::SetColorHsv { .. }
        | ToolbarEvent::SetThickness(_)
        | ToolbarEvent::NudgeThickness(_)
        | ToolbarEvent::SetMarkerOpacity(_)
        | ToolbarEvent::NudgeMarkerOpacity(_)
        | ToolbarEvent::SetEraserMode(_)
        | ToolbarEvent::SetFont(_)
        | ToolbarEvent::SetFontSize(_)
        | ToolbarEvent::NudgeFontSize(_)
        | ToolbarEvent::ToggleFill(_)
        | ToolbarEvent::SetPolygonSides(_)
        | ToolbarEvent::NudgePolygonSides(_)
        | ToolbarEvent::ToggleArrowLabels(_)
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
        | ToolbarEvent::SaveSessionAs
        | ToolbarEvent::SaveSessionAsConfirm(_)
        | ToolbarEvent::SaveSessionAsCancel
        | ToolbarEvent::SessionInfo
        | ToolbarEvent::ClearSession
        | ToolbarEvent::OpenConfigurator
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
        | ToolbarEvent::CancelRuntimeUiRecovery
        | ToolbarEvent::OpenCommandPalette
        | ToolbarEvent::SetCustomUndoDelay(_)
        | ToolbarEvent::SetCustomRedoDelay(_)
        | ToolbarEvent::SetCustomUndoSteps(_)
        | ToolbarEvent::SetCustomRedoSteps(_)
        | ToolbarEvent::CustomUndo
        | ToolbarEvent::CustomRedo
        | ToolbarEvent::ToggleTopOverflow(_)
        | ToolbarEvent::ToggleSessionPopover(_)
        | ToolbarEvent::ToggleSettingsPopover(_)
        | ToolbarEvent::ToggleCanvasPopover(_)
        | ToolbarEvent::ScrollTopPopover(_)
        | ToolbarEvent::CopyHexColor
        | ToolbarEvent::PasteHexColor
        | ToolbarEvent::EditHexColor
        | ToolbarEvent::OpenColorPickerPopup
        | ToolbarEvent::OpenPrecisionEntry(_)
        | ToolbarEvent::CommitPrecisionEntry { .. }
        | ToolbarEvent::CancelPrecisionEntry
        | ToolbarEvent::AdjustSelectionProperty { .. }
        | ToolbarEvent::PickScreenColor
        | ToolbarEvent::ScrollSidePane(_)
        | ToolbarEvent::DragToolbarItemOver { .. }
        | ToolbarEvent::SetToolbarItemCustomizationOpen(_)
        | ToolbarEvent::SetToolbarItemCustomizationGroup(_)
        | ToolbarEvent::SetStatusBarContentsOpen(_)
        | ToolbarEvent::ToggleShapePicker(_)
        | ToolbarEvent::MoveTopToolbar { .. }
        | ToolbarEvent::MoveSideToolbar { .. } => ToolbarPersistence::Ephemeral,
    }
}

fn backend_route_for_event(event: &ToolbarEvent) -> ToolbarBackendRoute {
    match event {
        ToolbarEvent::MoveTopToolbar { .. } => ToolbarBackendRoute::MoveTopToolbar,
        ToolbarEvent::MoveSideToolbar { .. } => ToolbarBackendRoute::MoveSideToolbar,
        _ => ToolbarBackendRoute::ApplyToInput,
    }
}

fn pre_apply_effects_for_event(event: &ToolbarEvent) -> Vec<ToolbarPreApplyEffect> {
    if matches!(
        event,
        ToolbarEvent::SetSidePane(pane) if *pane != crate::ui::toolbar::SidePane::Draw
    ) {
        vec![ToolbarPreApplyEffect::RecordDrawerHintShown]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::color::RED;

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
}
