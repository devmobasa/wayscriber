use crate::config::{ToolbarItemId, toolbar_item_ids as ids};

use super::super::{ToolbarEvent, ToolbarSideSection, ToolbarSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarCommandGroupKind {
    History,
    Zoom,
    AdvancedActions,
    Pages,
    Boards,
}

impl ToolbarCommandGroupKind {
    /// Label drawn above the group inside the Canvas pane's Actions card;
    /// Pages/Boards carry their own section headers instead.
    pub(crate) fn sub_label(self) -> Option<&'static str> {
        match self {
            Self::History => Some("History"),
            Self::Zoom => Some("Zoom"),
            Self::AdvancedActions => Some("Advanced"),
            Self::Pages | Self::Boards => None,
        }
    }
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

        if !canvas_pane_active(snapshot) {
            return None;
        }
        let show_advanced = snapshot.show_actions_advanced;
        let show_view_actions = snapshot.show_zoom_actions
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
                ToolbarCommandGroupKind::History,
                vec![
                    ToolbarButtonModel::new(ToolbarEvent::Undo, snapshot.undo_available),
                    ToolbarButtonModel::new(ToolbarEvent::Redo, snapshot.redo_available),
                    ToolbarButtonModel::new(ToolbarEvent::ClearCanvas { instant: false }, true),
                ],
            );
        }

        if show_view_actions {
            push_visible_group(
                snapshot,
                &mut groups,
                ToolbarCommandGroupKind::Zoom,
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

fn pages_buttons(snapshot: &ToolbarSnapshot) -> Vec<ToolbarButtonModel> {
    vec![
        ToolbarButtonModel::new(ToolbarEvent::PagePrev, snapshot.page_index > 0),
        ToolbarButtonModel::new(
            ToolbarEvent::PageNext,
            snapshot.page_index + 1 < snapshot.page_count,
        ),
        ToolbarButtonModel::new(ToolbarEvent::PageNew, true),
        ToolbarButtonModel::new(ToolbarEvent::PageDuplicate, true),
        ToolbarButtonModel::new(ToolbarEvent::PageDelete, true),
    ]
}

fn boards_buttons(snapshot: &ToolbarSnapshot) -> Vec<ToolbarButtonModel> {
    let can_cycle = snapshot.board_count > 1;
    vec![
        ToolbarButtonModel::new(ToolbarEvent::BoardPrev, can_cycle),
        ToolbarButtonModel::new(ToolbarEvent::BoardNext, can_cycle),
        ToolbarButtonModel::new(ToolbarEvent::BoardNew, true),
        ToolbarButtonModel::new(ToolbarEvent::BoardDuplicate, !snapshot.is_transparent),
        ToolbarButtonModel::new(ToolbarEvent::BoardDelete, true),
    ]
}

fn zoom_buttons(snapshot: &ToolbarSnapshot) -> Vec<ToolbarButtonModel> {
    vec![
        ToolbarButtonModel::new(ToolbarEvent::ZoomIn, true),
        ToolbarButtonModel::new(ToolbarEvent::ZoomOut, true),
        ToolbarButtonModel::new(ToolbarEvent::ResetZoom, snapshot.zoom_active),
        ToolbarButtonModel::new(ToolbarEvent::ToggleZoomLock, snapshot.zoom_active),
    ]
}

fn advanced_buttons(snapshot: &ToolbarSnapshot) -> Vec<ToolbarButtonModel> {
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
    buttons
}

pub(crate) fn toolbar_pages_model(snapshot: &ToolbarSnapshot) -> Option<ToolbarCommandGroup> {
    if snapshot.side_section_hidden(ToolbarSideSection::Pages)
        || !snapshot.show_pages_section
        || !canvas_pane_active(snapshot)
    {
        return None;
    }

    visible_group(
        snapshot,
        ToolbarCommandGroupKind::Pages,
        pages_buttons(snapshot),
    )
}

pub(crate) fn toolbar_boards_model(snapshot: &ToolbarSnapshot) -> Option<ToolbarCommandGroup> {
    if snapshot.side_section_hidden(ToolbarSideSection::Boards)
        || !snapshot.show_boards_section
        || !canvas_pane_active(snapshot)
    {
        return None;
    }

    visible_group(
        snapshot,
        ToolbarCommandGroupKind::Boards,
        boards_buttons(snapshot),
    )
}

/// Boards command group for the top strip's Canvas popover: gated on the
/// Boards display toggle and per-button hidden overrides only. Like the
/// Session/Settings popovers it ignores the side palette's pane selection,
/// so it renders under `side_layout = "pill"` where no Canvas pane exists.
pub(crate) fn toolbar_boards_model_for_popover(
    snapshot: &ToolbarSnapshot,
) -> Option<ToolbarCommandGroup> {
    if !snapshot.show_boards_section {
        return None;
    }
    // The top strip has no side pane, so the Canvas popover is the only place
    // the board picker (the full board grid) is reachable from the top bar. It
    // leads the row as the section's "browse all boards" entry, ahead of the
    // linear nav/create buttons; the side palette keeps its own header trigger.
    let mut buttons = Vec::with_capacity(6);
    buttons.push(ToolbarButtonModel::new(
        ToolbarEvent::ToggleBoardPicker,
        true,
    ));
    buttons.extend(boards_buttons(snapshot));
    visible_group(snapshot, ToolbarCommandGroupKind::Boards, buttons)
}

/// Pages command group for the Canvas popover; see
/// [`toolbar_boards_model_for_popover`] for the gating rationale.
pub(crate) fn toolbar_pages_model_for_popover(
    snapshot: &ToolbarSnapshot,
) -> Option<ToolbarCommandGroup> {
    if !snapshot.show_pages_section {
        return None;
    }
    visible_group(
        snapshot,
        ToolbarCommandGroupKind::Pages,
        pages_buttons(snapshot),
    )
}

/// Zoom command group for the Canvas popover, gated on the Zoom display
/// toggle only (the popover splits it out from the side pane's Actions card).
pub(crate) fn toolbar_zoom_group_for_popover(
    snapshot: &ToolbarSnapshot,
) -> Option<ToolbarCommandGroup> {
    if !snapshot.show_zoom_actions {
        return None;
    }
    visible_group(
        snapshot,
        ToolbarCommandGroupKind::Zoom,
        zoom_buttons(snapshot),
    )
}

/// Advanced-actions command group for the Canvas popover (undo-all,
/// redo-all, their timed variants when delay actions are enabled, and
/// Freeze), gated on the "Advanced actions" display toggle only.
pub(crate) fn toolbar_advanced_group_for_popover(
    snapshot: &ToolbarSnapshot,
) -> Option<ToolbarCommandGroup> {
    if !snapshot.show_actions_advanced {
        return None;
    }
    visible_group(
        snapshot,
        ToolbarCommandGroupKind::AdvancedActions,
        advanced_buttons(snapshot),
    )
}

fn canvas_pane_active(snapshot: &ToolbarSnapshot) -> bool {
    snapshot.active_side_pane == crate::ui::toolbar::SidePane::Canvas
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
    Some(match event {
        ToolbarEvent::Undo => ids::SIDE_ACTIONS_UNDO,
        ToolbarEvent::Redo => ids::SIDE_ACTIONS_REDO,
        ToolbarEvent::ClearCanvas { .. } => ids::SIDE_ACTIONS_CLEAR_CANVAS,
        ToolbarEvent::ZoomIn => ids::SIDE_ACTIONS_ZOOM_IN,
        ToolbarEvent::ZoomOut => ids::SIDE_ACTIONS_ZOOM_OUT,
        ToolbarEvent::ResetZoom => ids::SIDE_ACTIONS_RESET_ZOOM,
        ToolbarEvent::ToggleZoomLock => ids::SIDE_ACTIONS_TOGGLE_ZOOM_LOCK,
        ToolbarEvent::UndoAll => ids::SIDE_ACTIONS_UNDO_ALL,
        ToolbarEvent::RedoAll => ids::SIDE_ACTIONS_REDO_ALL,
        ToolbarEvent::UndoAllDelayed => ids::SIDE_ACTIONS_UNDO_ALL_DELAYED,
        ToolbarEvent::RedoAllDelayed => ids::SIDE_ACTIONS_REDO_ALL_DELAYED,
        ToolbarEvent::ToggleFreeze => ids::SIDE_ACTIONS_FREEZE,
        ToolbarEvent::PagePrev => ids::SIDE_PAGES_PREVIOUS,
        ToolbarEvent::PageNext => ids::SIDE_PAGES_NEXT,
        ToolbarEvent::PageNew => ids::SIDE_PAGES_NEW,
        ToolbarEvent::PageDuplicate => ids::SIDE_PAGES_DUPLICATE,
        ToolbarEvent::PageDelete => ids::SIDE_PAGES_DELETE,
        ToolbarEvent::ToggleBoardPicker => ids::SIDE_BOARDS_PICKER,
        ToolbarEvent::BoardPrev => ids::SIDE_BOARDS_PREVIOUS,
        ToolbarEvent::BoardNext => ids::SIDE_BOARDS_NEXT,
        ToolbarEvent::BoardNew => ids::SIDE_BOARDS_NEW,
        ToolbarEvent::BoardDuplicate => ids::SIDE_BOARDS_DUPLICATE,
        ToolbarEvent::BoardDelete => ids::SIDE_BOARDS_DELETE,
        ToolbarEvent::BoardRename => ids::SIDE_BOARDS_RENAME,
        _ => return None,
    })
}
