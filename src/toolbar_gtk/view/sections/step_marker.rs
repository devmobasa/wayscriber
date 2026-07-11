//! Step markers section: a "Next: N" header hint plus a Reset button.

use gtk4::prelude::*;

use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection};

use super::super::super::widgets::send_event;
use super::{SectionCard, SectionCtx, section_card};

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let card = section_card(
        ctx,
        ToolbarSideSection::StepMarkers,
        ToolbarSideSection::StepMarkers.label(),
    );
    let hint = attach_header_hint(&card, &format!("Next: {}", ctx.snapshot.step_marker_next));

    let reset = gtk4::Button::with_label("Reset");
    reset.set_size_request(-1, ctx.px(24.0));
    reset.set_tooltip_text(Some("Reset numbering to 1."));
    let sender = ctx.feedback.clone();
    reset.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::ResetStepMarkerCounter);
    });
    card.body.append(&reset);

    ctx.updaters.push(Box::new(move |snapshot| {
        if let Some(hint) = &hint {
            hint.set_text(&format!("Next: {}", snapshot.step_marker_next));
        }
    }));
    Some(card.root.upcast())
}

/// Right-aligned counter hint on the card's header row, kept visible even
/// while the section is collapsed like the built-in palette draws it.
fn attach_header_hint(card: &SectionCard, text: &str) -> Option<gtk4::Label> {
    let header = card.root.first_child().and_downcast::<gtk4::Button>()?;
    let row = header.child().and_downcast::<gtk4::Box>()?;
    let title = row.first_child()?;
    let hint = gtk4::Label::new(Some(text));
    hint.add_css_class("hint");
    row.insert_child_after(&hint, Some(&title));
    Some(hint)
}
