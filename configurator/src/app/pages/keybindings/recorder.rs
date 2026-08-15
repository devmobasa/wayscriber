use relm4::{ComponentSender, gtk};

use gtk::glib::object::IsA;
use gtk::prelude::*;

use crate::messages::Message;
use crate::models::KeybindingField;
#[cfg(not(feature = "tablet-input"))]
use crate::models::keybindings::tablet_unavailable_hint;
use crate::models::keybindings::{
    KeyboardModifiers, RecorderDeviceKind, ShortcutRecorderState, super_consumed_hint,
    waiting_prompt,
};

use super::super::super::state::ConfiguratorApp;
use super::widgets::{
    field_canceled, ignore_activating_click, set_accessible_label, set_label, set_sensitive,
    set_visible,
};

pub(super) struct RecorderPopover {
    pub popover: gtk::Popover,
    prompt: gtk::Label,
    finish: gtk::Button,
    remove_last: gtk::Button,
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

        let hint = gtk::Label::builder()
            .label(super_consumed_hint())
            .wrap(true)
            .xalign(0.0)
            .max_width_chars(36)
            .css_classes(["dim-label"])
            .build();
        set_accessible_label(&hint, super_consumed_hint());

        let cancel = gtk::Button::builder().label("Cancel").build();
        set_accessible_label(&cancel, "Cancel recording");
        {
            let sender = sender.clone();
            cancel.connect_clicked(move |_| {
                sender.input(Message::ShortcutRecordingCanceled(field));
            });
        }

        let finish = gtk::Button::builder().label("Finish").build();
        set_accessible_label(&finish, "Finish sequence");
        {
            let sender = sender.clone();
            finish.connect_clicked(move |_| {
                sender.input(Message::ShortcutSequenceFinish);
            });
        }

        let remove_last = gtk::Button::builder().label("Remove Last Step").build();
        set_accessible_label(&remove_last, "Remove last sequence step");
        {
            let sender = sender.clone();
            remove_last.connect_clicked(move |_| {
                sender.input(Message::ShortcutSequenceRemoveLastStep);
            });
        }

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.append(&finish);
        buttons.append(&remove_last);
        buttons.append(&cancel);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.append(&prompt);
        content.append(&hint);
        content.append(&device_hint_label());
        content.append(&buttons);

        let popover = gtk::Popover::builder()
            .autohide(true)
            .child(&content)
            .build();
        popover.set_parent(parent);
        ignore_activating_click(&popover);
        field_canceled(&popover, field, sender, Message::ShortcutRecordingCanceled);
        attach_key_controller(&popover, sender);
        attach_button_controller(&popover, sender);

        Self {
            popover,
            prompt,
            finish,
            remove_last,
        }
    }

    pub(super) fn refresh(&self, recorder: Option<&ShortcutRecorderState>) {
        let recording = recorder.is_some();
        set_label(
            &self.prompt,
            recorder
                .map(|recorder| recorder.prompt.as_str())
                .unwrap_or(""),
        );
        let sequence = recorder.is_some_and(ShortcutRecorderState::is_sequence);
        set_visible(&self.finish, sequence);
        set_visible(&self.remove_last, sequence);
        set_sensitive(
            &self.finish,
            recorder.is_some_and(ShortcutRecorderState::can_finish),
        );
        set_sensitive(
            &self.remove_last,
            recorder.is_some_and(ShortcutRecorderState::can_remove_last),
        );
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

fn device_hint_label() -> gtk::Label {
    let text = device_hint_text();
    let hint = gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .max_width_chars(36)
        .css_classes(["dim-label"])
        .build();
    set_accessible_label(&hint, text);
    hint
}

fn device_hint_text() -> &'static str {
    #[cfg(feature = "tablet-input")]
    {
        "Auxiliary mouse and stylus barrel buttons can be recorded. Left, middle, right, and the stylus tip cannot."
    }
    #[cfg(not(feature = "tablet-input"))]
    {
        tablet_unavailable_hint()
    }
}

fn attach_button_controller(popover: &gtk::Popover, sender: &ComponentSender<ConfiguratorApp>) {
    let ignore = std::rc::Rc::new(std::cell::Cell::new(false));
    {
        let ignore = ignore.clone();
        popover.connect_show(move |_| {
            ignore.set(true);
        });
    }
    let sender = sender.clone();
    let controller = gtk::EventControllerLegacy::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    controller.connect_event(move |_, event| {
        if event.event_type() != gtk::gdk::EventType::ButtonPress {
            return gtk::glib::Propagation::Proceed;
        }
        if ignore.get() {
            ignore.set(false);
            return gtk::glib::Propagation::Stop;
        }
        let Some(button_event) = event.downcast_ref::<gtk::gdk::ButtonEvent>() else {
            return gtk::glib::Propagation::Stop;
        };
        let kind = event
            .device()
            .map(|device| match device.source() {
                gtk::gdk::InputSource::Mouse
                | gtk::gdk::InputSource::Touchpad
                | gtk::gdk::InputSource::Trackpoint => RecorderDeviceKind::Mouse,
                gtk::gdk::InputSource::Pen => RecorderDeviceKind::Pen,
                _ => RecorderDeviceKind::Other,
            })
            .unwrap_or(RecorderDeviceKind::Other);
        let modifiers = event.modifier_state();
        let recorded = KeyboardModifiers {
            ctrl: modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK),
            shift: modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK),
            alt: modifiers.contains(gtk::gdk::ModifierType::ALT_MASK),
            super_held: modifiers.contains(gtk::gdk::ModifierType::SUPER_MASK)
                || modifiers.contains(gtk::gdk::ModifierType::META_MASK)
                || modifiers.contains(gtk::gdk::ModifierType::HYPER_MASK),
        };
        sender.input(Message::ShortcutRecorderButton(
            button_event.button(),
            kind,
            recorded,
        ));
        gtk::glib::Propagation::Stop
    });
    popover.add_controller(controller);
}

fn gdk_keyval(key: gtk::gdk::Key) -> u32 {
    use gtk::glib::translate::IntoGlib;
    key.into_glib()
}
