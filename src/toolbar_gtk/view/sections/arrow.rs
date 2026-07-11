//! Arrow labels section: "Auto-number" toggle, a "Next: N" header hint,
//! and a Reset button shown only while numbering is enabled.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection};

use super::super::super::widgets::send_event;
use super::{SectionCard, SectionCtx, section_card};

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let card = section_card(
        ctx,
        ToolbarSideSection::ArrowLabels,
        ToolbarSideSection::ArrowLabels.label(),
    );
    let hint = attach_header_hint(
        &card,
        &format!("Next: {}", ctx.snapshot.arrow_label_next),
        ctx.snapshot.arrow_label_enabled,
    );

    let check = gtk4::CheckButton::with_label("Auto-number");
    check.add_css_class("mini");
    check.set_tooltip_text(Some("Auto-number arrows 1, 2, 3."));
    check.set_active(ctx.snapshot.arrow_label_enabled);
    let sender = ctx.feedback.clone();
    let syncing = Rc::new(Cell::new(false));
    let toggle_sync = syncing.clone();
    check.connect_toggled(move |check| {
        if !toggle_sync.get() {
            send_event(&sender, ToolbarEvent::ToggleArrowLabels(check.is_active()));
        }
    });
    card.body.append(&check);

    let reset = gtk4::Button::with_label("Reset");
    reset.set_size_request(-1, ctx.px(24.0));
    reset.set_tooltip_text(Some("Reset numbering to 1."));
    reset.set_visible(ctx.snapshot.arrow_label_enabled);
    let sender = ctx.feedback.clone();
    reset.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::ResetArrowLabelCounter);
    });
    card.body.append(&reset);

    ctx.updaters.push(Box::new(move |snapshot| {
        if check.is_active() != snapshot.arrow_label_enabled {
            syncing.set(true);
            check.set_active(snapshot.arrow_label_enabled);
            syncing.set(false);
        }
        reset.set_visible(snapshot.arrow_label_enabled);
        if let Some(hint) = &hint {
            hint.set_text(&format!("Next: {}", snapshot.arrow_label_next));
            hint.set_visible(snapshot.arrow_label_enabled);
        }
    }));
    Some(card.root.upcast())
}

/// Right-aligned counter hint on the card's header row, kept visible even
/// while the section is collapsed like the built-in palette draws it.
fn attach_header_hint(card: &SectionCard, text: &str, visible: bool) -> Option<gtk4::Label> {
    let header = card.root.first_child().and_downcast::<gtk4::Button>()?;
    let row = header.child().and_downcast::<gtk4::Box>()?;
    let title = row.first_child()?;
    let hint = gtk4::Label::new(Some(text));
    hint.add_css_class("hint");
    hint.set_visible(visible);
    row.insert_child_after(&hint, Some(&title));
    Some(hint)
}
