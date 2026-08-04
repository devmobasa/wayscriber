use super::rows::{connect_button, plain_row, row_content_box, side_label};
use relm4::{ComponentSender, gtk};

use gtk::glib::SignalHandlerId;
use gtk::prelude::*;

use crate::messages::Message;
use crate::models::color::parse_hex;
use crate::models::{ColorPickerId, RenderProfileMappingSide};

use super::super::super::state::ConfiguratorApp;
use super::super::color_rows::{dialog_hex, mark_hex_error, set_swatch_blocked};
use super::super::set_text_blocked;

/// One side of a mapping: the hex field the Iced view had, plus the native
/// color dialog standing in for its popup picker.
pub(super) struct ColorField {
    hex: gtk::Entry,
    hex_handler: SignalHandlerId,
    swatch: gtk::ColorDialogButton,
    swatch_handler: SignalHandlerId,
}

impl ColorField {
    pub(super) fn refresh(&self, hex: &str) {
        set_text_blocked(&self.hex, &self.hex_handler, hex);
        // The same predicate the save gate counts with, so a field styled
        // clean can never be one the save refuses.
        mark_hex_error(&self.hex, hex);

        let Some((rgb, _)) = parse_hex(hex) else {
            // Half-typed hex: leave the swatch on the last color that parsed
            // rather than flashing it to black on every keystroke.
            return;
        };
        let rgba = gtk::gdk::RGBA::new(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.0);
        set_swatch_blocked(&self.swatch, &self.swatch_handler, &rgba);
    }
}

pub(super) struct MappingRow {
    pub(super) row: gtk::ListBoxRow,
    pub(super) from: ColorField,
    pub(super) to: ColorField,
}

pub(super) fn build_mapping_row(
    index: usize,
    mapping: usize,
    sender: &ComponentSender<ConfiguratorApp>,
) -> MappingRow {
    let content = row_content_box();

    content.append(&side_label("From"));
    let from = build_color_field(index, mapping, RenderProfileMappingSide::From, sender);
    content.append(&from.hex);
    content.append(&from.swatch);

    content.append(&side_label("\u{2192}"));
    content.append(&side_label("To"));
    let to = build_color_field(index, mapping, RenderProfileMappingSide::To, sender);
    content.append(&to.hex);
    content.append(&to.swatch);

    let remove = gtk::Button::builder()
        .label("Remove")
        .valign(gtk::Align::Center)
        .halign(gtk::Align::End)
        .hexpand(true)
        .build();
    connect_button(
        &remove,
        Message::RenderProfileMappingRemove(index, mapping),
        sender,
    );
    content.append(&remove);

    MappingRow {
        row: plain_row(&content),
        from,
        to,
    }
}

fn build_color_field(
    index: usize,
    mapping: usize,
    side: RenderProfileMappingSide,
    sender: &ComponentSender<ConfiguratorApp>,
) -> ColorField {
    let hex = gtk::Entry::builder()
        .placeholder_text("#RRGGBB")
        .width_chars(9)
        .max_width_chars(9)
        .build();
    let hex_handler = {
        let sender = sender.clone();
        hex.connect_changed(move |entry| {
            sender.input(Message::RenderProfileMappingColorChanged(
                index,
                mapping,
                side,
                entry.text().to_string(),
            ));
        })
    };

    let swatch =
        gtk::ColorDialogButton::new(Some(gtk::ColorDialog::builder().with_alpha(false).build()));
    swatch.set_valign(gtk::Align::Center);
    let id = match side {
        RenderProfileMappingSide::From => ColorPickerId::RenderProfileMappingFrom(index, mapping),
        RenderProfileMappingSide::To => ColorPickerId::RenderProfileMappingTo(index, mapping),
    };
    let swatch_handler = {
        let sender = sender.clone();
        swatch.connect_rgba_notify(move |button| {
            sender.input(Message::ColorPickerHexChanged(
                id,
                dialog_hex(&button.rgba()),
            ));
        })
    };

    ColorField {
        hex,
        hex_handler,
        swatch,
        swatch_handler,
    }
}

// ---- Add-mapping row ---------------------------------------------------

pub(super) fn build_add_mapping_row(
    index: usize,
    sender: &ComponentSender<ConfiguratorApp>,
) -> gtk::ListBoxRow {
    let content = row_content_box();
    let button = gtk::Button::builder()
        .label("Add mapping")
        .halign(gtk::Align::Start)
        .build();
    connect_button(&button, Message::RenderProfileMappingAdd(index), sender);
    content.append(&button);
    plain_row(&content)
}
