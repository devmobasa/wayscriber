use crate::input::ToolbarDrawerTab;

use super::super::{ToolbarEvent, ToolbarSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarCommandGroupKind {
    BasicActions,
    ViewActions,
    AdvancedActions,
    Pages,
    Boards,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarButtonModel {
    pub(crate) event: ToolbarEvent,
    pub(crate) enabled: bool,
}

impl ToolbarButtonModel {
    pub(crate) fn new(event: ToolbarEvent, enabled: bool) -> Self {
        Self { event, enabled }
    }

    pub(crate) fn short_label(
        &self,
        snapshot: &ToolbarSnapshot,
        fallback: &'static str,
    ) -> &'static str {
        self.event.short_label(snapshot, fallback)
    }

    pub(crate) fn tooltip_label(
        &self,
        snapshot: &ToolbarSnapshot,
        fallback: &'static str,
    ) -> &'static str {
        self.event.tooltip_label(snapshot, fallback)
    }

    pub(crate) fn binding_hint<'a>(&self, snapshot: &'a ToolbarSnapshot) -> Option<&'a str> {
        snapshot.binding_hints.binding_for_event(&self.event)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarCommandGroup {
    pub(crate) kind: ToolbarCommandGroupKind,
    pub(crate) buttons: Vec<ToolbarButtonModel>,
}

impl ToolbarCommandGroup {
    fn new(kind: ToolbarCommandGroupKind, buttons: Vec<ToolbarButtonModel>) -> Self {
        Self { kind, buttons }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarActionsModel {
    groups: Vec<ToolbarCommandGroup>,
}

impl ToolbarActionsModel {
    pub(crate) fn from_snapshot(snapshot: &ToolbarSnapshot) -> Option<Self> {
        let show_drawer_view = drawer_view_visible(snapshot);
        let show_advanced = snapshot.show_actions_advanced && show_drawer_view;
        let show_view_actions = show_drawer_view
            && snapshot.show_zoom_actions
            && (snapshot.show_actions_section || snapshot.show_actions_advanced);
        let show_actions = snapshot.show_actions_section || show_advanced;

        if !show_actions {
            return None;
        }

        let mut groups = Vec::with_capacity(3);
        if snapshot.show_actions_section {
            groups.push(ToolbarCommandGroup::new(
                ToolbarCommandGroupKind::BasicActions,
                vec![
                    ToolbarButtonModel::new(ToolbarEvent::Undo, snapshot.undo_available),
                    ToolbarButtonModel::new(ToolbarEvent::Redo, snapshot.redo_available),
                    ToolbarButtonModel::new(ToolbarEvent::ClearCanvas, true),
                ],
            ));
        }

        if show_view_actions {
            groups.push(ToolbarCommandGroup::new(
                ToolbarCommandGroupKind::ViewActions,
                vec![
                    ToolbarButtonModel::new(ToolbarEvent::ZoomIn, true),
                    ToolbarButtonModel::new(ToolbarEvent::ZoomOut, true),
                    ToolbarButtonModel::new(ToolbarEvent::ResetZoom, snapshot.zoom_active),
                    ToolbarButtonModel::new(ToolbarEvent::ToggleZoomLock, snapshot.zoom_active),
                ],
            ));
        }

        if show_advanced {
            let mut buttons = Vec::with_capacity(5);
            buttons.push(ToolbarButtonModel::new(
                ToolbarEvent::UndoAll,
                snapshot.undo_available,
            ));
            buttons.push(ToolbarButtonModel::new(
                ToolbarEvent::RedoAll,
                snapshot.redo_available,
            ));
            if snapshot.delay_actions_enabled {
                buttons.push(ToolbarButtonModel::new(
                    ToolbarEvent::UndoAllDelayed,
                    snapshot.undo_available,
                ));
                buttons.push(ToolbarButtonModel::new(
                    ToolbarEvent::RedoAllDelayed,
                    snapshot.redo_available,
                ));
            }
            buttons.push(ToolbarButtonModel::new(ToolbarEvent::ToggleFreeze, true));
            groups.push(ToolbarCommandGroup::new(
                ToolbarCommandGroupKind::AdvancedActions,
                buttons,
            ));
        }

        Some(Self { groups })
    }

    pub(crate) fn groups(&self) -> &[ToolbarCommandGroup] {
        &self.groups
    }
}

pub(crate) fn toolbar_pages_model(snapshot: &ToolbarSnapshot) -> Option<ToolbarCommandGroup> {
    if !snapshot.show_pages_section || !drawer_view_visible(snapshot) {
        return None;
    }

    Some(ToolbarCommandGroup::new(
        ToolbarCommandGroupKind::Pages,
        vec![
            ToolbarButtonModel::new(ToolbarEvent::PagePrev, snapshot.page_index > 0),
            ToolbarButtonModel::new(
                ToolbarEvent::PageNext,
                snapshot.page_index + 1 < snapshot.page_count,
            ),
            ToolbarButtonModel::new(ToolbarEvent::PageNew, true),
            ToolbarButtonModel::new(ToolbarEvent::PageDuplicate, true),
            ToolbarButtonModel::new(ToolbarEvent::PageDelete, true),
        ],
    ))
}

pub(crate) fn toolbar_boards_model(snapshot: &ToolbarSnapshot) -> Option<ToolbarCommandGroup> {
    if !snapshot.show_boards_section || !drawer_view_visible(snapshot) {
        return None;
    }

    let can_cycle = snapshot.board_count > 1;
    Some(ToolbarCommandGroup::new(
        ToolbarCommandGroupKind::Boards,
        vec![
            ToolbarButtonModel::new(ToolbarEvent::BoardPrev, can_cycle),
            ToolbarButtonModel::new(ToolbarEvent::BoardNext, can_cycle),
            ToolbarButtonModel::new(ToolbarEvent::BoardNew, true),
            ToolbarButtonModel::new(ToolbarEvent::BoardDuplicate, !snapshot.is_transparent),
            ToolbarButtonModel::new(ToolbarEvent::BoardDelete, true),
        ],
    ))
}

fn drawer_view_visible(snapshot: &ToolbarSnapshot) -> bool {
    snapshot.drawer_open && snapshot.drawer_tab == ToolbarDrawerTab::View
}
