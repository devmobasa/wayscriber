//! Actions section: the History / Zoom / Advanced command groups with the
//! built-in sub-labels, destructive split, and stateful lock/freeze faces.

use gtk4::prelude::*;

use crate::label_format::format_binding_label;
use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection, ToolbarSnapshot, model};

use super::super::super::icons::{IconPainter, IconWidget};
use super::super::super::widgets::{send_event, set_active_class, sized_button};
use super::{SectionCtx, command_row, section_card};

/// Tooltip fallback the built-in passes for action buttons.
const NOUN: &str = "Action";

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let actions_model = model::ToolbarActionsModel::from_snapshot(ctx.snapshot)?;
    let card = section_card(
        ctx,
        ToolbarSideSection::Actions,
        ToolbarSideSection::Actions.label(),
    );
    for group in actions_model.groups() {
        // Sub-label and buttons sit tight together; the body's spacing
        // separates the groups like the built-in's group gap.
        let block = gtk4::Box::new(gtk4::Orientation::Vertical, ctx.px(2.0));
        if let Some(label) = group.kind.sub_label() {
            let sub = gtk4::Label::new(Some(label));
            sub.add_css_class("hint");
            sub.set_xalign(0.0);
            block.append(&sub);
        }
        block.append(&build_group(ctx, group));
        card.body.append(&block);
    }
    Some(card.root.upcast())
}

/// One command group's buttons in the built-in arrangement: History reuses
/// the shared split row in icon mode (Undo/Redo left, destructive Clear
/// right) and stacks full-width rows in text mode; Zoom and Advanced sit
/// in a centered icon row or a two-column text grid, with Zoom's text
/// buttons carrying the accent "active" styling the built-in gives them.
fn build_group(ctx: &mut SectionCtx, group: &model::ToolbarCommandGroup) -> gtk4::Widget {
    let kind = group.kind;
    let history = kind == model::ToolbarCommandGroupKind::History;
    if ctx.use_icons && history {
        return command_row(ctx, &group.buttons, NOUN, history_icon, history_group).upcast();
    }

    let text_active_style = !ctx.use_icons && kind == model::ToolbarCommandGroupKind::Zoom;
    let handles: Vec<ActionHandle> = group
        .buttons
        .iter()
        .map(|button_model| action_button(ctx, button_model, text_active_style))
        .collect();

    let container: gtk4::Widget = if ctx.use_icons {
        // Zoom / Advanced: a single centered row (the built-in centers a
        // five-column grid; both groups hold at most five buttons).
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(6.0));
        row.set_halign(gtk4::Align::Center);
        for handle in &handles {
            row.append(&handle.button);
        }
        row.upcast()
    } else if history {
        // History text mode: one full-width button per row.
        let column = gtk4::Box::new(gtk4::Orientation::Vertical, ctx.px(5.0));
        for handle in &handles {
            column.append(&handle.button);
        }
        column.upcast()
    } else {
        // Zoom / Advanced text mode: two equal-width columns.
        let grid = gtk4::Grid::new();
        grid.set_column_homogeneous(true);
        grid.set_row_spacing(ctx.px(5.0) as u32);
        grid.set_column_spacing(ctx.px(6.0) as u32);
        for (index, handle) in handles.iter().enumerate() {
            grid.attach(&handle.button, (index % 2) as i32, (index / 2) as i32, 1, 1);
        }
        grid.upcast()
    };

    ctx.updaters.push(Box::new(move |snapshot| {
        let Some(group) = group_of(snapshot, kind) else {
            return;
        };
        for (handle, button_model) in handles.iter().zip(group.buttons.iter()) {
            handle.button.set_sensitive(button_model.enabled);
            if text_active_style {
                set_active_class(&handle.button, button_model.enabled);
            }
            // Only the lock/freeze toggles change face with the snapshot.
            if !matches!(
                handle.event,
                ToolbarEvent::ToggleZoomLock | ToolbarEvent::ToggleFreeze
            ) {
                continue;
            }
            match &handle.icon {
                Some(icon) => icon.set_painter(action_icon(snapshot, &handle.event)),
                None => handle
                    .button
                    .set_label(button_model.short_label(snapshot, NOUN)),
            }
            handle.button.set_tooltip_text(Some(&format_binding_label(
                button_model.tooltip_label(snapshot, NOUN),
                button_model.binding_hint(snapshot),
            )));
        }
    }));
    container
}

/// Handles the group updater needs to keep one button in sync.
struct ActionHandle {
    button: gtk4::Button,
    icon: Option<IconWidget>,
    event: ToolbarEvent,
}

fn action_button(
    ctx: &SectionCtx,
    button_model: &model::ToolbarButtonModel,
    text_active_style: bool,
) -> ActionHandle {
    let snapshot = ctx.snapshot;
    let (button, icon) = if ctx.use_icons {
        let size = ctx.sz(32.0);
        let button = sized_button(size, size);
        let icon = IconWidget::new(action_icon(snapshot, &button_model.event), ctx.sz(18.0));
        button.set_child(Some(&icon.area));
        (button, Some(icon))
    } else {
        let button = gtk4::Button::with_label(button_model.short_label(snapshot, NOUN));
        button.set_size_request(-1, ctx.px(24.0));
        button.set_hexpand(true);
        (button, None)
    };
    button.set_tooltip_text(Some(&format_binding_label(
        button_model.tooltip_label(snapshot, NOUN),
        button_model.binding_hint(snapshot),
    )));
    button.set_sensitive(button_model.enabled);
    if button_model.event.is_destructive() {
        button.add_css_class("destructive");
    }
    if text_active_style && button_model.enabled {
        set_active_class(&button, true);
    }
    let sender = ctx.feedback.clone();
    let event = button_model.event.clone();
    button.connect_clicked(move |_| {
        send_event(&sender, event.clone());
    });
    ActionHandle {
        button,
        icon,
        event: button_model.event.clone(),
    }
}

/// Re-derive one group from a later snapshot; looked up by kind so a group
/// set change between snapshots never cross-wires the updaters.
fn group_of(
    snapshot: &ToolbarSnapshot,
    kind: model::ToolbarCommandGroupKind,
) -> Option<model::ToolbarCommandGroup> {
    model::ToolbarActionsModel::from_snapshot(snapshot)?
        .groups()
        .iter()
        .find(|group| group.kind == kind)
        .cloned()
}

fn history_group(snapshot: &ToolbarSnapshot) -> Option<model::ToolbarCommandGroup> {
    group_of(snapshot, model::ToolbarCommandGroupKind::History)
}

fn history_icon(event: &ToolbarEvent) -> IconPainter {
    match event {
        ToolbarEvent::Undo => toolbar_icons::draw_icon_undo,
        ToolbarEvent::Redo => toolbar_icons::draw_icon_redo,
        _ => toolbar_icons::draw_icon_clear,
    }
}

/// Port of the built-in `icon_for_button`: the zoom-lock and freeze
/// buttons swap their glyph with the snapshot state.
fn action_icon(snapshot: &ToolbarSnapshot, event: &ToolbarEvent) -> IconPainter {
    match event {
        ToolbarEvent::Undo => toolbar_icons::draw_icon_undo,
        ToolbarEvent::Redo => toolbar_icons::draw_icon_redo,
        ToolbarEvent::ClearCanvas { .. } => toolbar_icons::draw_icon_clear,
        ToolbarEvent::ZoomIn => toolbar_icons::draw_icon_zoom_in,
        ToolbarEvent::ZoomOut => toolbar_icons::draw_icon_zoom_out,
        ToolbarEvent::ResetZoom => toolbar_icons::draw_icon_zoom_reset,
        ToolbarEvent::ToggleZoomLock => {
            if snapshot.zoom_locked {
                toolbar_icons::draw_icon_lock
            } else {
                toolbar_icons::draw_icon_unlock
            }
        }
        ToolbarEvent::UndoAll => toolbar_icons::draw_icon_undo_all,
        ToolbarEvent::RedoAll => toolbar_icons::draw_icon_redo_all,
        ToolbarEvent::UndoAllDelayed => toolbar_icons::draw_icon_undo_all_delay,
        ToolbarEvent::RedoAllDelayed => toolbar_icons::draw_icon_redo_all_delay,
        ToolbarEvent::ToggleFreeze => {
            if snapshot.frozen_active {
                toolbar_icons::draw_icon_unfreeze
            } else {
                toolbar_icons::draw_icon_freeze
            }
        }
        _ => toolbar_icons::draw_icon_clear,
    }
}
