use crate::config::ToolbarItemId;
use crate::input::ToolbarDrawerTab;

use super::super::{ToolbarEvent, ToolbarSideSection, ToolbarSnapshot};

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
        if snapshot.side_section_hidden(ToolbarSideSection::Actions) {
            return None;
        }

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
            push_visible_group(
                snapshot,
                &mut groups,
                ToolbarCommandGroupKind::BasicActions,
                vec![
                    ToolbarButtonModel::new(ToolbarEvent::Undo, snapshot.undo_available),
                    ToolbarButtonModel::new(ToolbarEvent::Redo, snapshot.redo_available),
                    ToolbarButtonModel::new(ToolbarEvent::ClearCanvas, true),
                ],
            );
        }

        if show_view_actions {
            push_visible_group(
                snapshot,
                &mut groups,
                ToolbarCommandGroupKind::ViewActions,
                vec![
                    ToolbarButtonModel::new(ToolbarEvent::ZoomIn, true),
                    ToolbarButtonModel::new(ToolbarEvent::ZoomOut, true),
                    ToolbarButtonModel::new(ToolbarEvent::ResetZoom, snapshot.zoom_active),
                    ToolbarButtonModel::new(ToolbarEvent::ToggleZoomLock, snapshot.zoom_active),
                ],
            );
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
            push_visible_group(
                snapshot,
                &mut groups,
                ToolbarCommandGroupKind::AdvancedActions,
                buttons,
            );
        }

        (!groups.is_empty()).then_some(Self { groups })
    }

    pub(crate) fn groups(&self) -> &[ToolbarCommandGroup] {
        &self.groups
    }
}

pub(crate) fn toolbar_pages_model(snapshot: &ToolbarSnapshot) -> Option<ToolbarCommandGroup> {
    if snapshot.side_section_hidden(ToolbarSideSection::Pages)
        || !snapshot.show_pages_section
        || !drawer_view_visible(snapshot)
    {
        return None;
    }

    visible_group(
        snapshot,
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
    )
}

pub(crate) fn toolbar_boards_model(snapshot: &ToolbarSnapshot) -> Option<ToolbarCommandGroup> {
    if snapshot.side_section_hidden(ToolbarSideSection::Boards)
        || !snapshot.show_boards_section
        || !drawer_view_visible(snapshot)
    {
        return None;
    }

    let can_cycle = snapshot.board_count > 1;
    visible_group(
        snapshot,
        ToolbarCommandGroupKind::Boards,
        vec![
            ToolbarButtonModel::new(ToolbarEvent::BoardPrev, can_cycle),
            ToolbarButtonModel::new(ToolbarEvent::BoardNext, can_cycle),
            ToolbarButtonModel::new(ToolbarEvent::BoardNew, true),
            ToolbarButtonModel::new(ToolbarEvent::BoardDuplicate, !snapshot.is_transparent),
            ToolbarButtonModel::new(ToolbarEvent::BoardDelete, true),
        ],
    )
}

fn drawer_view_visible(snapshot: &ToolbarSnapshot) -> bool {
    snapshot.drawer_open && snapshot.drawer_tab == ToolbarDrawerTab::View
}

fn push_visible_group(
    snapshot: &ToolbarSnapshot,
    groups: &mut Vec<ToolbarCommandGroup>,
    kind: ToolbarCommandGroupKind,
    buttons: Vec<ToolbarButtonModel>,
) {
    if let Some(group) = visible_group(snapshot, kind, buttons) {
        groups.push(group);
    }
}

fn visible_group(
    snapshot: &ToolbarSnapshot,
    kind: ToolbarCommandGroupKind,
    buttons: Vec<ToolbarButtonModel>,
) -> Option<ToolbarCommandGroup> {
    let buttons: Vec<_> = buttons
        .into_iter()
        .filter(|button| toolbar_button_visible(snapshot, &button.event))
        .collect();
    (!buttons.is_empty()).then(|| ToolbarCommandGroup::new(kind, buttons))
}

fn toolbar_button_visible(snapshot: &ToolbarSnapshot, event: &ToolbarEvent) -> bool {
    toolbar_button_item_id(event).is_none_or(|id| !snapshot.toolbar_item_hidden(id))
}

fn toolbar_button_item_id(event: &ToolbarEvent) -> Option<ToolbarItemId> {
    Some(ToolbarItemId::from_known(match event {
        ToolbarEvent::Undo => "side.actions.undo",
        ToolbarEvent::Redo => "side.actions.redo",
        ToolbarEvent::ClearCanvas => "side.actions.clear-canvas",
        ToolbarEvent::ZoomIn => "side.actions.zoom-in",
        ToolbarEvent::ZoomOut => "side.actions.zoom-out",
        ToolbarEvent::ResetZoom => "side.actions.reset-zoom",
        ToolbarEvent::ToggleZoomLock => "side.actions.toggle-zoom-lock",
        ToolbarEvent::UndoAll => "side.actions.undo-all",
        ToolbarEvent::RedoAll => "side.actions.redo-all",
        ToolbarEvent::UndoAllDelayed => "side.actions.undo-all-delayed",
        ToolbarEvent::RedoAllDelayed => "side.actions.redo-all-delayed",
        ToolbarEvent::ToggleFreeze => "side.actions.freeze",
        ToolbarEvent::PagePrev => "side.pages.previous",
        ToolbarEvent::PageNext => "side.pages.next",
        ToolbarEvent::PageNew => "side.pages.new",
        ToolbarEvent::PageDuplicate => "side.pages.duplicate",
        ToolbarEvent::PageDelete => "side.pages.delete",
        ToolbarEvent::BoardPrev => "side.boards.previous",
        ToolbarEvent::BoardNext => "side.boards.next",
        ToolbarEvent::BoardNew => "side.boards.new",
        ToolbarEvent::BoardDuplicate => "side.boards.duplicate",
        ToolbarEvent::BoardDelete => "side.boards.delete",
        ToolbarEvent::BoardRename => "side.boards.rename",
        _ => return None,
    }))
}
