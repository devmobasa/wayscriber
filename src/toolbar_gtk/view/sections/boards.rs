//! Boards section: Prev / Next / New / Duplicate / Delete command row,
//! plus the board-picker keybinding hint the built-in shows.

use gtk4::prelude::*;

use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection, model};

use super::super::super::icons::IconPainter;
use super::{SectionCtx, command_row, section_card};

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let group = model::toolbar_boards_model(ctx.snapshot)?;
    let card = section_card(
        ctx,
        ToolbarSideSection::Boards,
        ToolbarSideSection::Boards.label(),
    );
    let row = command_row(
        ctx,
        &group.buttons,
        "Board",
        board_icon,
        model::toolbar_boards_model,
    );
    card.body.append(&row);
    // The built-in right-aligns the board-picker binding inside the section
    // header; the GTK header is owned by `section_card`, so the hint sits
    // right-aligned under the buttons instead.
    if let Some(binding) = ctx
        .snapshot
        .binding_hints
        .binding_for_event(&ToolbarEvent::ToggleBoardPicker)
    {
        let hint = gtk4::Label::new(Some(binding));
        hint.add_css_class("hint");
        hint.set_xalign(1.0);
        card.body.append(&hint);
    }
    Some(card.root.upcast())
}

fn board_icon(event: &ToolbarEvent) -> IconPainter {
    match event {
        ToolbarEvent::BoardPrev => toolbar_icons::draw_icon_chevron_left,
        ToolbarEvent::BoardNext => toolbar_icons::draw_icon_chevron_right,
        ToolbarEvent::BoardNew => toolbar_icons::draw_icon_plus,
        ToolbarEvent::BoardDuplicate => toolbar_icons::draw_icon_copy,
        _ => toolbar_icons::draw_icon_clear,
    }
}
