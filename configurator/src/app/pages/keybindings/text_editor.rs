use relm4::{ComponentSender, gtk};

use gtk::glib::object::IsA;
use gtk::prelude::*;

use crate::messages::Message;
use crate::models::KeybindingField;

use super::super::super::state::ConfiguratorApp;
use super::super::set_text_blocked;
use super::widgets::{field_canceled, ignore_activating_click, set_accessible_label, set_label};

pub(super) struct TextEditorPopover {
    pub popover: gtk::Popover,
    entry: gtk::Entry,
    entry_handler: gtk::glib::SignalHandlerId,
    error: gtk::Label,
}

impl TextEditorPopover {
    pub(super) fn build(
        parent: &impl IsA<gtk::Widget>,
        field: KeybindingField,
        sender: &ComponentSender<ConfiguratorApp>,
    ) -> Self {
        let entry = gtk::Entry::builder()
            .placeholder_text("Ctrl+Shift+X, F5")
            .hexpand(true)
            .width_chars(28)
            .build();
        set_accessible_label(&entry, "Shortcut list");
        let entry_handler = {
            let sender = sender.clone();
            entry.connect_changed(move |entry| {
                sender.input(Message::ShortcutTextEditChanged(entry.text().to_string()));
            })
        };

        let error = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .css_classes(["error", "caption"])
            .build();

        let apply = gtk::Button::builder()
            .label("Apply")
            .css_classes(["suggested-action"])
            .build();
        set_accessible_label(&apply, "Apply shortcut text");
        {
            let sender = sender.clone();
            apply.connect_clicked(move |_| {
                sender.input(Message::ShortcutTextEditApplied);
            });
        }
        let cancel = gtk::Button::builder().label("Cancel").build();
        set_accessible_label(&cancel, "Cancel text editing");
        {
            let sender = sender.clone();
            cancel.connect_clicked(move |_| {
                sender.input(Message::ShortcutTextEditCanceled(field));
            });
        }

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_halign(gtk::Align::End);
        buttons.append(&cancel);
        buttons.append(&apply);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.append(&entry);
        content.append(&error);
        content.append(&buttons);

        let popover = gtk::Popover::builder()
            .autohide(true)
            .child(&content)
            .build();
        popover.set_parent(parent);
        ignore_activating_click(&popover);
        field_canceled(&popover, field, sender, Message::ShortcutTextEditCanceled);

        Self {
            popover,
            entry,
            entry_handler,
            error,
        }
    }

    pub(super) fn refresh(&self, open: bool, text: &str, parse_error: Option<&str>) {
        set_text_blocked(&self.entry, &self.entry_handler, text);
        match parse_error {
            Some(message) => {
                set_label(&self.error, message);
                self.error.set_visible(true);
            }
            None => {
                set_label(&self.error, "");
                self.error.set_visible(false);
            }
        }
        if open {
            if !self.popover.is_visible() {
                self.popover.popup();
                self.entry.grab_focus();
            }
        } else if self.popover.is_visible() {
            self.popover.popdown();
        }
    }
}
