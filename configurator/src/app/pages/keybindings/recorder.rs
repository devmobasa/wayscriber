use relm4::{ComponentSender, gtk};

use gtk::glib::object::IsA;
use gtk::prelude::*;

use crate::messages::Message;
use crate::models::KeybindingField;
use crate::models::keybindings::{KeyboardModifiers, waiting_prompt};

use super::super::super::state::ConfiguratorApp;
use super::widgets::{field_canceled, ignore_activating_click, set_accessible_label, set_label};

pub(super) struct RecorderPopover {
    pub popover: gtk::Popover,
    prompt: gtk::Label,
}

impl RecorderPopover {
    pub(super) fn build(
        parent: &impl IsA<gtk::Widget>,
        field: KeybindingField,
        sender: &ComponentSender<ConfiguratorApp>,
    ) -> Self {
        let prompt = gtk::Label::builder()
            .label(waiting_prompt())
            .wrap(true)
            .xalign(0.0)
            .max_width_chars(36)
            .can_focus(true)
            .build();
        set_accessible_label(&prompt, "Shortcut recorder");

        let cancel = gtk::Button::builder().label("Cancel").build();
        set_accessible_label(&cancel, "Cancel recording");
        {
            let sender = sender.clone();
            cancel.connect_clicked(move |_| {
                sender.input(Message::ShortcutRecordingCanceled(field));
            });
        }

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.append(&prompt);
        content.append(&cancel);

        let popover = gtk::Popover::builder()
            .autohide(true)
            .child(&content)
            .build();
        popover.set_parent(parent);
        ignore_activating_click(&popover);
        field_canceled(&popover, field, sender, Message::ShortcutRecordingCanceled);
        attach_key_controller(&popover, sender);

        Self { popover, prompt }
    }

    pub(super) fn refresh(&self, recording: bool, prompt: &str) {
        set_label(&self.prompt, prompt);
        if recording {
            if !self.popover.is_visible() {
                self.popover.popup();
                self.prompt.grab_focus();
            }
        } else if self.popover.is_visible() {
            self.popover.popdown();
        }
    }
}

fn attach_key_controller(popover: &gtk::Popover, sender: &ComponentSender<ConfiguratorApp>) {
    let sender = sender.clone();
    let controller = gtk::EventControllerKey::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    controller.connect_key_pressed(move |_, key, _, modifiers| {
        let keyval = gdk_keyval(key);
        let recorded = KeyboardModifiers {
            ctrl: modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK),
            shift: modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK),
            alt: modifiers.contains(gtk::gdk::ModifierType::ALT_MASK),
            super_held: modifiers.contains(gtk::gdk::ModifierType::SUPER_MASK)
                || modifiers.contains(gtk::gdk::ModifierType::META_MASK)
                || modifiers.contains(gtk::gdk::ModifierType::HYPER_MASK),
        };
        sender.input(Message::ShortcutRecorderKey(keyval, recorded));
        gtk::glib::Propagation::Stop
    });
    popover.add_controller(controller);
}

fn gdk_keyval(key: gtk::gdk::Key) -> u32 {
    use gtk::glib::translate::IntoGlib;
    key.into_glib()
}
