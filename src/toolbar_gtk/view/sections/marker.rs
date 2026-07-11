//! Marker opacity section: minus / drag-safe slider / plus with a "{n}%"
//! readout, mirroring the built-in nudge step and range.

use gtk4::prelude::*;

use crate::toolbar_icons;
use crate::ui::toolbar::model::ToolbarSliderSpec;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection};

use super::super::super::widgets::{SliderRow, icon_button, send_event};
use super::{SectionCtx, section_card};

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let card = section_card(
        ctx,
        ToolbarSideSection::MarkerOpacity,
        ToolbarSideSection::MarkerOpacity.label(),
    );

    let spec = ToolbarSliderSpec::MARKER_OPACITY;
    let step = spec.step.unwrap_or(0.05);
    // Built-in SIDE_NUDGE_SIZE / SIDE_NUDGE_ICON_SIZE.
    let btn = ctx.sz(24.0);
    let icon = ctx.sz(14.0);
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(6.0));

    let minus = icon_button(
        toolbar_icons::draw_icon_minus,
        (btn, btn),
        icon,
        "Decrease opacity",
    );
    let sender = ctx.feedback.clone();
    minus.button.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::NudgeMarkerOpacity(-step));
    });
    row.append(&minus.button);

    let sender = ctx.feedback.clone();
    let slider = SliderRow::new(
        ctx.scale,
        (spec.min, spec.max),
        ctx.snapshot.marker_opacity,
        format_percent,
        move |value| {
            send_event(&sender, ToolbarEvent::SetMarkerOpacity(value));
        },
    );
    slider.root.set_hexpand(true);
    row.append(&slider.root);

    let plus = icon_button(
        toolbar_icons::draw_icon_plus,
        (btn, btn),
        icon,
        "Increase opacity",
    );
    let sender = ctx.feedback.clone();
    plus.button.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::NudgeMarkerOpacity(step));
    });
    row.append(&plus.button);

    card.body.append(&row);
    ctx.updaters.push(Box::new(move |snapshot| {
        slider.set_value(snapshot.marker_opacity);
    }));
    Some(card.root.upcast())
}

fn format_percent(value: f64) -> String {
    format!("{:.0}%", value * 100.0)
}
