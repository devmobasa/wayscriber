use crate::input::ToolbarDrawerTab;

use super::{ToolbarEvent, ToolbarSnapshot};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::{ToolbarBindingHints, ToolbarSnapshot};

    fn snapshot() -> ToolbarSnapshot {
        let mut state = make_test_input_state();
        state.toolbar_drawer_open = true;
        state.toolbar_drawer_tab = ToolbarDrawerTab::View;
        state.show_actions_section = true;
        state.show_actions_advanced = false;
        state.show_zoom_actions = true;
        state.show_pages_section = true;
        state.show_boards_section = true;
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default())
    }

    #[test]
    fn actions_model_keeps_advanced_actions_in_view_drawer() {
        let mut snapshot = snapshot();
        snapshot.show_actions_section = false;
        snapshot.show_actions_advanced = true;
        snapshot.delay_actions_enabled = true;

        let model = ToolbarActionsModel::from_snapshot(&snapshot).expect("actions model");
        assert_eq!(model.groups().len(), 2);
        assert_eq!(model.groups()[0].kind, ToolbarCommandGroupKind::ViewActions);
        assert_eq!(
            model.groups()[1].kind,
            ToolbarCommandGroupKind::AdvancedActions
        );
        assert_eq!(model.groups()[1].buttons.len(), 5);

        snapshot.drawer_open = false;
        assert!(ToolbarActionsModel::from_snapshot(&snapshot).is_none());
    }

    #[test]
    fn page_and_board_models_report_disabled_navigation() {
        let mut snapshot = snapshot();
        snapshot.page_count = 2;
        snapshot.board_count = 1;
        snapshot.is_transparent = true;

        let pages = toolbar_pages_model(&snapshot).expect("pages model");
        assert!(!pages.buttons[0].enabled);
        assert!(pages.buttons[1].enabled);

        let boards = toolbar_boards_model(&snapshot).expect("boards model");
        assert!(!boards.buttons[0].enabled);
        assert!(!boards.buttons[1].enabled);
        assert!(!boards.buttons[3].enabled);
    }

    #[test]
    fn dynamic_toolbar_labels_live_with_the_event_model() {
        let mut snapshot = snapshot();
        snapshot.frozen_active = true;
        snapshot.zoom_locked = true;

        let freeze = ToolbarButtonModel::new(ToolbarEvent::ToggleFreeze, true);
        let zoom_lock = ToolbarButtonModel::new(ToolbarEvent::ToggleZoomLock, true);

        assert_eq!(freeze.short_label(&snapshot, "Action"), "Unfreeze");
        assert_eq!(zoom_lock.tooltip_label(&snapshot, "Action"), "Unlock Zoom");
        assert_eq!(zoom_lock.binding_hint(&snapshot), None);
    }
}
