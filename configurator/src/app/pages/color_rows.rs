//! Shared color-editing row.
//!
//! Every color the configurator edits routes through the model's hex path:
//! the entry row sends `ColorPickerHexChanged` exactly as the Iced picker's
//! hex field did, and the native color dialog feeds the same message with a
//! formatted hex string. The update layer already parses, applies, and
//! keeps `color_picker_hex` as the per-picker editing text, so this row has
//! no color state of its own.
//!
//! A refresh writes the widgets with their handlers blocked. Without that a
//! programmatic write reports itself as a user edit, and the hex path is
//! lossy in that direction: it re-derives every component from the 8-bit hex,
//! so simply showing a loaded config would quantize float colors and mark the
//! draft dirty.

use relm4::{adw, gtk};

use adw::prelude::*;
use gtk::glib::SignalHandlerId;

use crate::messages::Message;
use crate::models::ColorPickerId;
use crate::models::color::hex_field_error;

use super::super::state::ConfiguratorApp;
use super::{PageBuilder, set_text_blocked};

/// Resolved RGBA for the swatch/dialog seed, `0.0..=1.0` channels.
pub(crate) type ResolvedColor = Option<(f64, f64, f64, f64)>;

/// Formats a dialog selection the way the hex handler expects: `#rrggbb`,
/// with an alpha byte only when the color is actually translucent.
pub(crate) fn dialog_hex(rgba: &gtk::gdk::RGBA) -> String {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b, a) = (
        channel(rgba.red()),
        channel(rgba.green()),
        channel(rgba.blue()),
        channel(rgba.alpha()),
    );
    if a == u8::MAX {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

/// Formats a selection for a four-component field, retaining `FF` so choosing
/// fully opaque replaces any translucent alpha already in the draft.
fn dialog_hex_with_alpha(rgba: &gtk::gdk::RGBA) -> String {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b, a) = (
        channel(rgba.red()),
        channel(rgba.green()),
        channel(rgba.blue()),
        channel(rgba.alpha()),
    );
    format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
}

/// Marks a color field as holding text the save gate refuses.
pub(crate) fn mark_hex_error(widget: &impl IsA<gtk::Widget>, text: &str) {
    let has_error_class = widget.has_css_class("error");
    match hex_field_error(text) {
        Some(message) => {
            if !has_error_class {
                widget.add_css_class("error");
            }
            if widget.tooltip_text().as_deref() != Some(message) {
                widget.set_tooltip_text(Some(message));
            }
        }
        None => {
            if has_error_class {
                widget.remove_css_class("error");
                widget.set_tooltip_text(None);
            }
        }
    }
}

/// Seeds a color dialog button from the model without reporting it as a pick.
///
/// The comparison runs through the emitted hex so f32/f64 rounding cannot
/// ping-pong the dialog button and the model.
pub(crate) fn set_swatch_blocked(
    button: &gtk::ColorDialogButton,
    handler: &SignalHandlerId,
    rgba: &gtk::gdk::RGBA,
) {
    if dialog_hex(&button.rgba()) == dialog_hex(rgba) {
        return;
    }
    button.block_signal(handler);
    button.set_rgba(rgba);
    button.unblock_signal(handler);
}

/// Adds a color row to the current group: a hex entry (the model's editing
/// text for this picker) with a native color-dialog button as suffix.
/// Returns the row so callers can gate its visibility.
pub(crate) fn color_row(
    page: &mut PageBuilder,
    title: &str,
    id: ColorPickerId,
    resolved: impl Fn(&ConfiguratorApp) -> ResolvedColor + 'static,
) -> adw::EntryRow {
    let row = adw::EntryRow::builder().title(title).build();
    let row_handler = {
        let sender = page.sender();
        row.connect_changed(move |row| {
            sender.input(Message::ColorPickerHexChanged(id, row.text().to_string()));
        })
    };

    let uses_alpha = id.uses_alpha();
    let dialog_button = gtk::ColorDialogButton::new(Some(
        gtk::ColorDialog::builder().with_alpha(uses_alpha).build(),
    ));
    dialog_button.set_valign(gtk::Align::Center);
    let button_handler = {
        let sender = page.sender();
        dialog_button.connect_rgba_notify(move |button| {
            sender.input(Message::ColorPickerHexChanged(
                id,
                if uses_alpha {
                    dialog_hex_with_alpha(&button.rgba())
                } else {
                    dialog_hex(&button.rgba())
                },
            ));
        })
    };
    row.add_suffix(&dialog_button);

    page.custom(&row);
    let handle = row.clone();
    page.bind(move |app, _summary| {
        let text = app.color_picker_hex.get(&id).cloned().unwrap_or_default();
        set_text_blocked(&row, &row_handler, &text);
        mark_hex_error(&row, &text);
        if let Some((r, g, b, a)) = resolved(app) {
            let rgba = gtk::gdk::RGBA::new(r as f32, g as f32, b as f32, a as f32);
            set_swatch_blocked(&dialog_button, &button_handler, &rgba);
        }
    });
    handle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_dialog_hex_keeps_an_explicit_opaque_byte() {
        let rgba = gtk::gdk::RGBA::new(18.0 / 255.0, 52.0 / 255.0, 86.0 / 255.0, 1.0);

        assert_eq!(dialog_hex_with_alpha(&rgba), "#123456FF");
        assert_eq!(dialog_hex(&rgba), "#123456");
    }
}
