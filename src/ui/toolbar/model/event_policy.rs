use crate::config::{Action, action_label, action_short_label};
use crate::input::Tool;

use super::super::ToolbarEvent;

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
    Toolbar,
    Ui(ToolbarUiPersistenceTarget),
    History,
    ClickHighlight,
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
        ToolbarEvent::EnterTextMode => Some(Action::EnterTextMode),
        ToolbarEvent::EnterStickyNoteMode => Some(Action::EnterStickyNoteMode),
        ToolbarEvent::ToggleFill(_) => Some(Action::ToggleFill),
        ToolbarEvent::Undo => Some(Action::Undo),
        ToolbarEvent::Redo => Some(Action::Redo),
        ToolbarEvent::UndoAll => Some(Action::UndoAll),
        ToolbarEvent::RedoAll => Some(Action::RedoAll),
        ToolbarEvent::UndoAllDelayed => Some(Action::UndoAllDelayed),
        ToolbarEvent::RedoAllDelayed => Some(Action::RedoAllDelayed),
        ToolbarEvent::ClearCanvas => Some(Action::ClearCanvas),
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
        ToolbarEvent::ToggleZoomLock => Some(Action::ToggleZoomLock),
        ToolbarEvent::ApplyPreset(slot) => action_for_apply_preset(*slot),
        ToolbarEvent::SavePreset(slot) => action_for_save_preset(*slot),
        ToolbarEvent::ClearPreset(slot) => action_for_clear_preset(*slot),
        ToolbarEvent::OpenConfigurator => Some(Action::OpenConfigurator),
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
    use ToolbarPersistenceTarget::*;
    use ToolbarUiPersistenceTarget::*;
    match event {
        ToolbarEvent::PinTopToolbar(_)
        | ToolbarEvent::PinSideToolbar(_)
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
        | ToolbarEvent::ToggleToolPreview(_)
        | ToolbarEvent::ToggleDelaySliders(_)
        | ToolbarEvent::SetToolbarLayoutMode(_)
        | ToolbarEvent::SetSidePane(_)
        | ToolbarEvent::SetTopMinimized(_)
        | ToolbarEvent::SetSideMinimized(_)
        | ToolbarEvent::CloseTopToolbar
        | ToolbarEvent::CloseSideToolbar
        | ToolbarEvent::ToggleSideSectionCollapsed(_, _)
        | ToolbarEvent::SetToolbarItemHidden(_, _)
        | ToolbarEvent::MoveToolbarItem { .. }
        | ToolbarEvent::DragToolbarItemOver { .. }
        | ToolbarEvent::ResetToolbarItemOrder(_)
        | ToolbarEvent::ResetToolbarItemHiddenOverrides => ToolbarPersistence::Persist(Toolbar),
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
        _ => ToolbarPersistence::RuntimeOnly,
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
