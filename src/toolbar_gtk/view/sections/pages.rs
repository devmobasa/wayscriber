//! Pages section: Prev / Next / New / Duplicate / Delete command row.

use gtk4::prelude::*;

use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection, model};

use super::super::super::icons::IconPainter;
use super::{SectionCtx, command_row, section_card};

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let group = model::toolbar_pages_model(ctx.snapshot)?;
    let card = section_card(
        ctx,
        ToolbarSideSection::Pages,
        ToolbarSideSection::Pages.label(),
    );
    let row = command_row(
        ctx,
        &group.buttons,
        "Page",
        page_icon,
        model::toolbar_pages_model,
    );
    card.body.append(&row);
    Some(card.root.upcast())
}

fn page_icon(event: &ToolbarEvent) -> IconPainter {
    match event {
        ToolbarEvent::PagePrev => toolbar_icons::draw_icon_chevron_left,
        ToolbarEvent::PageNext => toolbar_icons::draw_icon_chevron_right,
        ToolbarEvent::PageNew => toolbar_icons::draw_icon_plus,
        ToolbarEvent::PageDuplicate => toolbar_icons::draw_icon_copy,
        _ => toolbar_icons::draw_icon_clear,
    }
}
