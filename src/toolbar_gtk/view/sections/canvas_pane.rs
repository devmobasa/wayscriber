//! Canvas pane popover content: the Boards / Pages / Advanced / Zoom command
//! sections (each gated on its display toggle) and the Step Undo/Redo
//! configuration — the side Canvas pane re-homed into the top strip's Canvas
//! popover without the collapsible-card chrome. Most state (sections, step
//! counts, enabled/glyph faces) rides the popover host's content-key rebuild,
//! but the Step Undo/Redo section registers persistent slider updaters: the
//! host retains them (in `canvas_updaters`) and runs them every apply, so a
//! continuous delay-slider drag reads its live value without rebuilding the
//! subtree (the content key omits the delay values).

use gtk4::prelude::*;

use crate::label_format::format_binding_label;
use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot, model};

use super::super::super::icons::{IconPainter, IconWidget};
use super::super::super::widgets::{send_event, sized_button};
use super::{SectionCtx, step_undo};

/// Extra separation between the evenly distributed safe actions and a
/// destructive action at the right edge of a Canvas command row.
const DESTRUCTIVE_GUARD_GAP: f64 = 12.0;

/// Horizontal inset of the Canvas popover content from the popover frame, so
/// the evenly distributed icon rows don't touch the edge. Mirrors the builtin
/// popover's `MENU_PAD` around the shared Canvas content area.
const CANVAS_CONTENT_PAD_H: f64 = 10.0;

pub(in crate::toolbar_gtk) fn build_popover_content(ctx: &mut SectionCtx) -> gtk4::Box {
    let column = gtk4::Box::new(gtk4::Orientation::Vertical, ctx.px(10.0));
    let pad_h = ctx.px(CANVAS_CONTENT_PAD_H);
    column.set_margin_start(pad_h);
    column.set_margin_end(pad_h);

    if let Some(group) = model::toolbar_boards_model_for_popover(ctx.snapshot) {
        command_section(ctx, &column, "Boards", "Board", &group, board_icon);
    }
    if let Some(group) = model::toolbar_pages_model_for_popover(ctx.snapshot) {
        command_section(ctx, &column, "Pages", "Page", &group, page_icon);
    }
    if let Some(group) = model::toolbar_advanced_group_for_popover(ctx.snapshot) {
        command_section(ctx, &column, "Advanced", "Action", &group, action_icon);
    }
    if let Some(group) = model::toolbar_zoom_group_for_popover(ctx.snapshot) {
        command_section(ctx, &column, "Zoom", "Zoom", &group, action_icon);
    }
    if ctx.snapshot.show_step_section {
        column.append(&section_title(ctx, "Step Undo/Redo"));
        column.append(&step_undo::build_popover_content(ctx));
    }

    column
}

/// A bold section header, styled like the collapsible cards' titles.
fn section_title(ctx: &SectionCtx, title: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(title));
    label.add_css_class("section-title");
    label.set_xalign(0.0);
    label.set_margin_top(ctx.px(2.0));
    label
}

/// One command section: a header over evenly distributed safe actions, with
/// any destructive action isolated at the right edge by a deliberate guard
/// gap. Enabled/glyph state is baked in at build time (the popover host
/// rebuilds on the inputs that change them).
fn command_section(
    ctx: &SectionCtx,
    column: &gtk4::Box,
    title: &str,
    noun: &'static str,
    group: &model::ToolbarCommandGroup,
    icon_for: fn(&ToolbarSnapshot, &ToolbarEvent) -> IconPainter,
) {
    column.append(&section_title(ctx, title));
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(DESTRUCTIVE_GUARD_GAP));
    let safe_actions = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(6.0));
    safe_actions.set_homogeneous(true);
    safe_actions.set_hexpand(true);
    row.append(&safe_actions);
    let btn_h = if ctx.use_icons {
        ctx.sz(32.0)
    } else {
        ctx.sz(24.0)
    };
    for button_model in &group.buttons {
        let is_destructive = button_model.event.is_destructive();
        let button = if ctx.use_icons {
            let handle = sized_button(btn_h, btn_h);
            // Keep the icon itself square and center it in the homogeneous
            // safe-action slot. Stretching the button would turn each slot
            // into an oversized tap target and crowd the isolated Delete.
            handle.set_halign(gtk4::Align::Center);
            let icon = IconWidget::new(icon_for(ctx.snapshot, &button_model.event), ctx.sz(18.0));
            handle.set_child(Some(&icon.area));
            handle
        } else {
            let handle = gtk4::Button::with_label(button_model.short_label(ctx.snapshot, noun));
            handle.set_size_request(-1, btn_h.round() as i32);
            handle
        };
        button.set_hexpand(!ctx.use_icons && !is_destructive);
        button.set_tooltip_text(Some(&format_binding_label(
            button_model.tooltip_label(ctx.snapshot, noun),
            button_model.binding_hint(ctx.snapshot),
        )));
        button.set_sensitive(button_model.enabled);
        if is_destructive {
            button.add_css_class("destructive");
            // The Canvas viewport uses an overlay scrollbar. Leave a small
            // trailing gutter so it never paints over the destructive target.
            button.set_margin_end(ctx.px(6.0));
        }
        let sender = ctx.feedback.clone();
        let event = button_model.event.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        if is_destructive {
            row.append(&button);
        } else {
            safe_actions.append(&button);
        }
    }
    column.append(&row);
}

fn board_icon(_snapshot: &ToolbarSnapshot, event: &ToolbarEvent) -> IconPainter {
    match event {
        ToolbarEvent::ToggleBoardPicker => toolbar_icons::draw_icon_grid,
        ToolbarEvent::BoardPrev => toolbar_icons::draw_icon_chevron_left,
        ToolbarEvent::BoardNext => toolbar_icons::draw_icon_chevron_right,
        ToolbarEvent::BoardNew => toolbar_icons::draw_icon_plus,
        ToolbarEvent::BoardDuplicate => toolbar_icons::draw_icon_copy,
        _ => toolbar_icons::draw_icon_clear,
    }
}

fn page_icon(_snapshot: &ToolbarSnapshot, event: &ToolbarEvent) -> IconPainter {
    match event {
        ToolbarEvent::PagePrev => toolbar_icons::draw_icon_chevron_left,
        ToolbarEvent::PageNext => toolbar_icons::draw_icon_chevron_right,
        ToolbarEvent::PageNew => toolbar_icons::draw_icon_plus,
        ToolbarEvent::PageDuplicate => toolbar_icons::draw_icon_copy,
        _ => toolbar_icons::draw_icon_clear,
    }
}

/// Zoom/Advanced glyphs, with the zoom-lock and freeze faces following the
/// snapshot state (port of the side pane's `action_icon`).
fn action_icon(snapshot: &ToolbarSnapshot, event: &ToolbarEvent) -> IconPainter {
    match event {
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
