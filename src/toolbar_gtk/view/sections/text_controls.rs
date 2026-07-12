//! Text block: font-size slider and the Sans / Monospace font row.

use gtk4::prelude::*;

use crate::draw::FontDescriptor;
use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection, model};

use super::super::super::widgets::{SliderRow, icon_button, send_event, set_active_class};
use super::{SectionCtx, section_card};

/// Spec-unit sizes from the built-in side palette (`SIDE_NUDGE_SIZE`,
/// `SIDE_NUDGE_ICON_SIZE`, `SIDE_FONT_BUTTON_HEIGHT`, `SIDE_FONT_BUTTON_GAP`).
const NUDGE_BUTTON_SIZE: f64 = 24.0;
const NUDGE_ICON_SIZE: f64 = 14.0;
const FONT_BUTTON_HEIGHT: f64 = 24.0;
const FONT_BUTTON_GAP: f64 = 8.0;

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let snapshot = ctx.snapshot;
    let block = gtk4::Box::new(gtk4::Orientation::Vertical, ctx.px(12.0));

    if !snapshot.side_section_hidden(ToolbarSideSection::TextSize) {
        block.append(&text_size_card(ctx));
    }
    if !snapshot.side_section_hidden(ToolbarSideSection::Font) {
        block.append(&font_card(ctx));
    }
    block.first_child()?;
    Some(block.upcast())
}

fn text_size_card(ctx: &mut SectionCtx) -> gtk4::Widget {
    let snapshot = ctx.snapshot;
    let card = section_card(ctx, ToolbarSideSection::TextSize, "Text size");
    if snapshot.side_section_collapsed(ToolbarSideSection::TextSize) {
        return card.root.upcast();
    }

    let spec = model::ToolbarSliderSpec::FONT_SIZE;
    let step = spec.step.unwrap_or(2.0);
    let (min, max) = (spec.min, spec.max);
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(6.0));
    let btn = ctx.sz(NUDGE_BUTTON_SIZE);
    let icon = ctx.sz(NUDGE_ICON_SIZE);

    let minus = icon_button(
        toolbar_icons::draw_icon_minus,
        (btn, btn),
        icon,
        "Decrease font size",
    );
    let sender = ctx.feedback.clone();
    minus.button.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::NudgeFontSize(-step));
    });
    row.append(&minus.button);

    let sender = ctx.feedback.clone();
    let slider = SliderRow::new(
        ctx.scale,
        (min, max),
        snapshot.font_size,
        format_pt,
        move |value| {
            send_event(&sender, ToolbarEvent::SetFontSize(value));
        },
    );
    slider.root.set_hexpand(true);
    row.append(&slider.root);

    let plus = icon_button(
        toolbar_icons::draw_icon_plus,
        (btn, btn),
        icon,
        "Increase font size",
    );
    let sender = ctx.feedback.clone();
    plus.button.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::NudgeFontSize(step));
    });
    row.append(&plus.button);

    card.body.append(&row);
    ctx.updaters.push(Box::new(move |snapshot| {
        slider.set_value(snapshot.font_size);
    }));
    card.root.upcast()
}

fn format_pt(value: f64) -> String {
    format!("{value:.0}pt")
}

fn font_card(ctx: &mut SectionCtx) -> gtk4::Widget {
    let snapshot = ctx.snapshot;
    let card = section_card(ctx, ToolbarSideSection::Font, "Font");
    if snapshot.side_section_collapsed(ToolbarSideSection::Font) {
        return card.root.upcast();
    }

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(FONT_BUTTON_GAP));
    row.set_homogeneous(true);
    let fonts = [
        FontDescriptor::new("Sans".to_string(), "bold".to_string(), "normal".to_string()),
        FontDescriptor::new(
            "Monospace".to_string(),
            "normal".to_string(),
            "normal".to_string(),
        ),
    ];
    let mut handles: Vec<(gtk4::Button, String)> = Vec::new();
    for font in fonts {
        let button = gtk4::Button::with_label(&font.family);
        button.set_size_request(-1, ctx.px(FONT_BUTTON_HEIGHT));
        set_active_class(&button, font.family == snapshot.font.family);
        let sender = ctx.feedback.clone();
        let family = font.family.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::SetFont(font.clone()));
        });
        row.append(&button);
        handles.push((button, family));
    }
    card.body.append(&row);
    ctx.updaters.push(Box::new(move |snapshot| {
        for (button, family) in &handles {
            set_active_class(button, *family == snapshot.font.family);
        }
    }));
    card.root.upcast()
}
