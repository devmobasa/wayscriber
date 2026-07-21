use crate::config::{
    Action, ToolbarItemId, ToolbarItemOrderGroup, ToolbarSectionFlag, action_label,
    action_short_label,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarPersistence {
    RuntimeOnly,
    Persist(ToolbarPersistenceTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarPersistenceTarget {
    Toolbar(ToolbarConfigPersistenceTarget),
    Ui(ToolbarUiPersistenceTarget),
    History,
    ClickHighlight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarConfigPersistenceTarget {
    LayoutMode,
    ItemVisibility {
        id: ToolbarItemId,
        hidden: bool,
    },
    ResetItemVisibility,
    ItemOrder(ToolbarItemOrderGroup),
    TopPinned,
    SidePinned,
    TopMinimized,
    TopDisplayState,
    SideMinimized,
    SidePane,
    CollapsedSection {
        section: ToolbarSideSection,
        collapsed: bool,
    },
    Icons,
    MoreColors,
    ContextAwareUi,
    PresetToasts,
    ToolPreview,
    DelaySliders,
    TopPosition,
    SidePosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarUiPersistenceTarget {
    StatusBar,
    StatusBoardBadge,
    StatusPageBadge,
    FloatingBadgeAlways,
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
    use ToolbarConfigPersistenceTarget::*;
    use ToolbarPersistenceTarget::*;
    use ToolbarUiPersistenceTarget::*;
    match event {
        ToolbarEvent::PinTopToolbar(_) => ToolbarPersistence::Persist(Toolbar(TopPinned)),
        ToolbarEvent::PinSideToolbar(_) => ToolbarPersistence::Persist(Toolbar(SidePinned)),
        ToolbarEvent::ToggleIconMode(_) => ToolbarPersistence::Persist(Toolbar(Icons)),
        ToolbarEvent::ToggleMoreColors(_) => ToolbarPersistence::Persist(Toolbar(MoreColors)),
        ToolbarEvent::ToggleActionsSection(show) => {
            ToolbarPersistence::Persist(Toolbar(ItemVisibility {
                id: ToolbarSectionFlag::Actions.item_id(),
                hidden: !show,
            }))
        }
        ToolbarEvent::ToggleActionsAdvanced(show) => {
            ToolbarPersistence::Persist(Toolbar(ItemVisibility {
                id: ToolbarSectionFlag::ActionsAdvanced.item_id(),
                hidden: !show,
            }))
        }
        ToolbarEvent::ToggleZoomActions(show) => {
            ToolbarPersistence::Persist(Toolbar(ItemVisibility {
                id: ToolbarSectionFlag::ZoomActions.item_id(),
                hidden: !show,
            }))
        }
        ToolbarEvent::TogglePagesSection(show) => {
            ToolbarPersistence::Persist(Toolbar(ItemVisibility {
                id: ToolbarSectionFlag::Pages.item_id(),
                hidden: !show,
            }))
        }
        ToolbarEvent::ToggleBoardsSection(show) => {
            ToolbarPersistence::Persist(Toolbar(ItemVisibility {
                id: ToolbarSectionFlag::Boards.item_id(),
                hidden: !show,
            }))
        }
        ToolbarEvent::TogglePresets(show) => ToolbarPersistence::Persist(Toolbar(ItemVisibility {
            id: ToolbarSectionFlag::Presets.item_id(),
            hidden: !show,
        })),
        ToolbarEvent::ToggleStepSection(show) => {
            ToolbarPersistence::Persist(Toolbar(ItemVisibility {
                id: ToolbarSectionFlag::StepSection.item_id(),
                hidden: !show,
            }))
        }
        ToolbarEvent::ToggleTextControls(show) => {
            ToolbarPersistence::Persist(Toolbar(ItemVisibility {
                id: ToolbarSectionFlag::TextControls.item_id(),
                hidden: !show,
            }))
        }
        ToolbarEvent::ToggleContextAwareUi(_) => {
            ToolbarPersistence::Persist(Toolbar(ContextAwareUi))
        }
        ToolbarEvent::TogglePresetToasts(_) => ToolbarPersistence::Persist(Toolbar(PresetToasts)),
        ToolbarEvent::ToggleToolPreview(_) => ToolbarPersistence::Persist(Toolbar(ToolPreview)),
        ToolbarEvent::ToggleDelaySliders(_) => ToolbarPersistence::Persist(Toolbar(DelaySliders)),
        ToolbarEvent::SetToolbarLayoutMode(_) => ToolbarPersistence::Persist(Toolbar(LayoutMode)),
        ToolbarEvent::SetSidePane(_) => ToolbarPersistence::Persist(Toolbar(SidePane)),
        ToolbarEvent::SetTopMinimized(_) | ToolbarEvent::CloseTopToolbar => {
            ToolbarPersistence::Persist(Toolbar(TopMinimized))
        }
        ToolbarEvent::SetTopDisplayMode(_) => ToolbarPersistence::Persist(Toolbar(TopDisplayState)),
        ToolbarEvent::SetSideMinimized(_) | ToolbarEvent::CloseSideToolbar => {
            ToolbarPersistence::Persist(Toolbar(SideMinimized))
        }
        ToolbarEvent::ToggleSideSectionCollapsed(section, collapsed) => {
            ToolbarPersistence::Persist(Toolbar(CollapsedSection {
                section: *section,
                collapsed: *collapsed,
            }))
        }
        ToolbarEvent::SetToolbarItemHidden(id, hidden) => {
            ToolbarPersistence::Persist(Toolbar(ItemVisibility {
                id: *id,
                hidden: *hidden,
            }))
        }
        ToolbarEvent::MoveToolbarItem { group, .. }
        | ToolbarEvent::DragToolbarItemOver { group, .. }
        | ToolbarEvent::ResetToolbarItemOrder(group) => {
            ToolbarPersistence::Persist(Toolbar(ItemOrder(*group)))
        }
        ToolbarEvent::ResetToolbarItemHiddenOverrides => {
            ToolbarPersistence::Persist(Toolbar(ResetItemVisibility))
        }
        ToolbarEvent::ToggleCustomSection(_) => ToolbarPersistence::Persist(History),
        ToolbarEvent::ToggleStatusBar(_) => ToolbarPersistence::Persist(Ui(StatusBar)),
        ToolbarEvent::ToggleStatusBoardBadge(_) => {
            ToolbarPersistence::Persist(Ui(StatusBoardBadge))
        }
        ToolbarEvent::ToggleStatusPageBadge(_) => ToolbarPersistence::Persist(Ui(StatusPageBadge)),
        ToolbarEvent::ToggleFloatingBadgeAlways(_) => {
            ToolbarPersistence::Persist(Ui(FloatingBadgeAlways))
        }
        ToolbarEvent::SelectTool(Tool::Highlight)
        | ToolbarEvent::ToggleAllHighlight(_)
        | ToolbarEvent::ToggleHighlightToolRing(_) => ToolbarPersistence::Persist(ClickHighlight),
        ToolbarEvent::SelectTool(_)
        | ToolbarEvent::SetColor(_)
        | ToolbarEvent::SetQuickColor { .. }
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
        | ToolbarEvent::StartToolbarItemDrag { .. }
        | ToolbarEvent::SetToolbarItemCustomizationOpen(_)
        | ToolbarEvent::SetToolbarItemCustomizationGroup(_)
        | ToolbarEvent::ToggleShapePicker(_)
        | ToolbarEvent::MoveTopToolbar { .. }
        | ToolbarEvent::MoveSideToolbar { .. } => ToolbarPersistence::RuntimeOnly,
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
        };
        let duplicate = ToolbarEvent::SetQuickColor {
            color: RED,
            action: Some(Action::SetColorGreen),
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
            }),
            None
        );
    }
}
