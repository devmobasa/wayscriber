use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::config::{Action, action_label};
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;

use super::super::super::widgets::*;
use super::TopStripLayout;

pub(super) fn draw_utility_row(
    layout: &mut TopStripLayout,
    mut x: f64,
    y: f64,
    btn_size: f64,
    icon_size: f64,
    gap: f64,
    is_simple: bool,
) -> f64 {
    let snapshot = layout.snapshot;
    let hover = layout.hover;

    let is_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
        .unwrap_or(false);
    draw_button(
        layout.ctx,
        x,
        y,
        btn_size,
        btn_size,
        snapshot.text_active,
        is_hover,
    );
    set_icon_color(layout.ctx, is_hover);
    toolbar_icons::draw_icon_text(
        layout.ctx,
        x + (btn_size - icon_size) / 2.0,
        y + (btn_size - icon_size) / 2.0,
        icon_size,
    );
    layout.hits.push(HitRegion {
        rect: (x, y, btn_size, btn_size),
        event: ToolbarEvent::EnterTextMode,
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            action_label(Action::EnterTextMode),
            snapshot.binding_hints.binding_for_action(Action::EnterTextMode),
        )),
    });
    x += btn_size + gap;

    let note_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
        .unwrap_or(false);
    draw_button(
        layout.ctx,
        x,
        y,
        btn_size,
        btn_size,
        snapshot.note_active,
        note_hover,
    );
    set_icon_color(layout.ctx, note_hover);
    toolbar_icons::draw_icon_note(
        layout.ctx,
        x + (btn_size - icon_size) / 2.0,
        y + (btn_size - icon_size) / 2.0,
        icon_size,
    );
    layout.hits.push(HitRegion {
        rect: (x, y, btn_size, btn_size),
        event: ToolbarEvent::EnterStickyNoteMode,
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            action_label(Action::EnterStickyNoteMode),
            snapshot.binding_hints
                .binding_for_action(Action::EnterStickyNoteMode),
        )),
    });
    x += btn_size + gap;

    if !is_simple {
        let clear_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
            .unwrap_or(false);
        draw_button(layout.ctx, x, y, btn_size, btn_size, false, clear_hover);
        set_icon_color(layout.ctx, clear_hover);
        toolbar_icons::draw_icon_clear(
            layout.ctx,
            x + (btn_size - icon_size) / 2.0,
            y + (btn_size - icon_size) / 2.0,
            icon_size,
        );
        layout.hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::ClearCanvas,
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(Action::ClearCanvas),
                snapshot.binding_hints.binding_for_action(Action::ClearCanvas),
            )),
        });
        x += btn_size + gap;

        let highlight_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
            .unwrap_or(false);
        draw_button(
            layout.ctx,
            x,
            y,
            btn_size,
            btn_size,
            snapshot.any_highlight_active,
            highlight_hover,
        );
        set_icon_color(layout.ctx, highlight_hover);
        toolbar_icons::draw_icon_highlight(
            layout.ctx,
            x + (btn_size - icon_size) / 2.0,
            y + (btn_size - icon_size) / 2.0,
            icon_size,
        );
        layout.hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(Action::ToggleHighlightTool),
                snapshot
                    .binding_hints
                    .binding_for_action(Action::ToggleHighlightTool),
            )),
        });
        x += btn_size + gap;
    }

    x
}
