use std::borrow::Cow;

use crate::config::ToolbarLayoutMode;

use super::super::{ToolbarEvent, ToolbarSnapshot};
use super::activation::{ToolbarActivation, ToolbarControlId, ToolbarDragTarget};
use super::control::{
    ToolbarBoardChipPresentation, ToolbarControl, ToolbarControlKind, ToolbarControlPresentation,
    ToolbarControlRole, ToolbarIcon, ToolbarPresentationPayload, ToolbarSegment,
    ToolbarSegmentedControl, ToolbarSingleControl, ToolbarTooltip,
};
use super::event_policy::full_mode_target;

#[derive(Debug, Clone)]
pub(crate) struct SideHeaderModel {
    pub(crate) drag: ToolbarControl,
    pub(crate) icon_mode: ToolbarControl,
    pub(crate) pin: ToolbarControl,
    pub(crate) close: ToolbarControl,
    pub(crate) layout_mode: ToolbarControl,
    pub(crate) drawer_more: ToolbarControl,
    pub(crate) board_chip: ToolbarControl,
    pub(crate) show_drawer_hint: bool,
}

impl SideHeaderModel {
    pub(crate) fn from_snapshot(snapshot: &ToolbarSnapshot) -> Self {
        Self {
            drag: drag_control(
                ToolbarControlId::DragSide,
                ToolbarDragTarget::MoveSideToolbar,
            ),
            icon_mode: icon_mode_control(snapshot.use_icons),
            pin: single_button(
                ToolbarControlId::PinSide,
                ToolbarEvent::PinSideToolbar(!snapshot.side_pinned),
                if snapshot.side_pinned {
                    "Pinned (click to unpin)"
                } else {
                    "Pin toolbar"
                },
                snapshot.side_pinned,
                ToolbarControlRole::Button,
            ),
            close: single_button(
                ToolbarControlId::CloseSide,
                ToolbarEvent::CloseSideToolbar,
                "Close",
                false,
                ToolbarControlRole::Button,
            ),
            layout_mode: layout_mode_control(snapshot.layout_mode),
            drawer_more: drawer_more_control(snapshot.drawer_open),
            board_chip: board_chip_control(snapshot),
            show_drawer_hint: snapshot.show_drawer_hint,
        }
    }
}

pub(crate) fn board_chip_label(snapshot: &ToolbarSnapshot) -> String {
    let board_index = snapshot.board_index + 1;
    let board_count = snapshot.board_count.max(1);
    let name = snapshot.board_name.trim();
    let board_label = if board_count > 1 {
        if name.is_empty() {
            format!("Board {}/{}", board_index, board_count)
        } else {
            format!("Board {}/{} · {}", board_index, board_count, name)
        }
    } else if name.is_empty() {
        "Board".to_string()
    } else {
        format!("Board · {}", name)
    };
    let pages = snapshot.page_count.max(1);
    if pages > 1 {
        let page = (snapshot.page_index + 1).min(pages);
        format!("{board_label} · p.{page}/{pages}")
    } else {
        board_label
    }
}

fn drag_control(id: ToolbarControlId, target: ToolbarDragTarget) -> ToolbarControl {
    ToolbarControl {
        id,
        kind: ToolbarControlKind::Single(ToolbarSingleControl {
            activation: ToolbarActivation::Drag(target),
            action: None,
        }),
        enabled: true,
        active: false,
        presentation: ToolbarControlPresentation {
            label: Cow::Borrowed("Drag toolbar"),
            tooltip: ToolbarTooltip::text("Drag toolbar"),
            icon: None,
            role: ToolbarControlRole::DragHandle,
            payload: ToolbarPresentationPayload::None,
        },
    }
}

fn icon_mode_control(use_icons: bool) -> ToolbarControl {
    let segments = vec![
        ToolbarSegment {
            id: ToolbarControlId::IconModeIcons,
            label: Cow::Borrowed("Ico"),
            activation: ToolbarActivation::Click(ToolbarEvent::ToggleIconMode(true)),
            action: None,
            tooltip: ToolbarTooltip::text("Icons mode"),
            enabled: true,
        },
        ToolbarSegment {
            id: ToolbarControlId::IconModeText,
            label: Cow::Borrowed("Txt"),
            activation: ToolbarActivation::Click(ToolbarEvent::ToggleIconMode(false)),
            action: None,
            tooltip: ToolbarTooltip::text("Text mode"),
            enabled: true,
        },
    ];
    let active = if use_icons {
        ToolbarControlId::IconModeIcons
    } else {
        ToolbarControlId::IconModeText
    };
    segmented_control(
        ToolbarControlId::IconModeIcons,
        active,
        "Display mode",
        segments,
    )
}

fn layout_mode_control(mode: ToolbarLayoutMode) -> ToolbarControl {
    let full_mode = full_mode_target(mode);
    let segments = vec![
        ToolbarSegment {
            id: ToolbarControlId::LayoutModeSimple,
            label: Cow::Borrowed("Simple"),
            activation: ToolbarActivation::Click(ToolbarEvent::SetToolbarLayoutMode(
                ToolbarLayoutMode::Simple,
            )),
            action: None,
            tooltip: ToolbarTooltip::text("Simple mode"),
            enabled: true,
        },
        ToolbarSegment {
            id: ToolbarControlId::LayoutModeFull,
            label: Cow::Borrowed("Full"),
            activation: ToolbarActivation::Click(ToolbarEvent::SetToolbarLayoutMode(full_mode)),
            action: None,
            tooltip: ToolbarTooltip::text("Full mode"),
            enabled: true,
        },
    ];
    let active = if mode == ToolbarLayoutMode::Simple {
        ToolbarControlId::LayoutModeSimple
    } else {
        ToolbarControlId::LayoutModeFull
    };
    segmented_control(
        ToolbarControlId::LayoutModeSimple,
        active,
        "Toolbar layout",
        segments,
    )
}

fn drawer_more_control(drawer_open: bool) -> ToolbarControl {
    ToolbarControl {
        id: ToolbarControlId::DrawerMore,
        kind: ToolbarControlKind::Single(ToolbarSingleControl {
            activation: ToolbarActivation::Click(ToolbarEvent::ToggleDrawer(!drawer_open)),
            action: None,
        }),
        enabled: true,
        active: drawer_open,
        presentation: ToolbarControlPresentation {
            label: Cow::Borrowed("More"),
            tooltip: ToolbarTooltip::text("More options"),
            icon: Some(ToolbarIcon::More),
            role: ToolbarControlRole::Button,
            payload: ToolbarPresentationPayload::None,
        },
    }
}

fn board_chip_control(snapshot: &ToolbarSnapshot) -> ToolbarControl {
    let label = board_chip_label(snapshot);
    ToolbarControl {
        id: ToolbarControlId::BoardChip,
        kind: ToolbarControlKind::Single(ToolbarSingleControl {
            activation: ToolbarActivation::Click(ToolbarEvent::ToggleBoardPicker),
            action: crate::config::Action::BoardPicker.into(),
        }),
        enabled: true,
        active: false,
        presentation: ToolbarControlPresentation {
            label: Cow::Owned(label.clone()),
            tooltip: ToolbarTooltip::text("Board settings"),
            icon: Some(ToolbarIcon::Board),
            role: ToolbarControlRole::BoardChip,
            payload: ToolbarPresentationPayload::BoardChip(ToolbarBoardChipPresentation {
                label,
                color: snapshot.board_color,
                board_index: snapshot.board_index,
                board_count: snapshot.board_count,
                page_count: snapshot.page_count,
            }),
        },
    }
}

fn single_button(
    id: ToolbarControlId,
    event: ToolbarEvent,
    tooltip: &'static str,
    active: bool,
    role: ToolbarControlRole,
) -> ToolbarControl {
    ToolbarControl {
        id,
        kind: ToolbarControlKind::Single(ToolbarSingleControl {
            activation: ToolbarActivation::Click(event),
            action: None,
        }),
        enabled: true,
        active,
        presentation: ToolbarControlPresentation {
            label: Cow::Borrowed(tooltip),
            tooltip: ToolbarTooltip::text(tooltip),
            icon: None,
            role,
            payload: ToolbarPresentationPayload::None,
        },
    }
}

fn segmented_control(
    id: ToolbarControlId,
    active: ToolbarControlId,
    label: &'static str,
    segments: Vec<ToolbarSegment>,
) -> ToolbarControl {
    ToolbarControl {
        id,
        kind: ToolbarControlKind::Segmented(
            ToolbarSegmentedControl::try_new(Some(active), segments)
                .expect("static segmented toolbar control is valid"),
        ),
        enabled: true,
        active: true,
        presentation: ToolbarControlPresentation {
            label: Cow::Borrowed(label),
            tooltip: ToolbarTooltip::None,
            icon: None,
            role: ToolbarControlRole::Segmented,
            payload: ToolbarPresentationPayload::None,
        },
    }
}
