use relm4::{ComponentSender, gtk};

use gtk::prelude::*;

use crate::messages::Message;
use crate::models::DaemonAction;

use super::super::super::state::ConfiguratorApp;

pub(super) fn column_box() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .build()
}

pub(super) fn row_box() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build()
}

pub(super) fn body_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .build()
}

pub(super) fn caption_label(text: &str) -> gtk::Label {
    let label = body_label(text);
    label.add_css_class("caption");
    label
}

pub(super) fn hint_label(text: &str) -> gtk::Label {
    let label = caption_label(text);
    label.add_css_class("dim-label");
    label
}

pub(super) fn warning_label(text: &str) -> gtk::Label {
    let label = caption_label(text);
    label.add_css_class("warning");
    label
}

pub(super) fn action_button(
    label: &str,
    action: DaemonAction,
    sender: &ComponentSender<ConfiguratorApp>,
) -> gtk::Button {
    let button = gtk::Button::builder()
        .label(label)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .build();
    let sender = sender.clone();
    button.connect_clicked(move |_| sender.input(Message::DaemonActionRequested(action)));
    button
}

pub(super) fn set_label(label: &gtk::Label, text: &str) {
    if label.label() != text {
        label.set_label(text);
    }
}

pub(super) fn set_button_label(button: &gtk::Button, text: &str) {
    if button.label().as_deref() != Some(text) {
        button.set_label(text);
    }
}

/// Writes the widget's own visibility flag, never `is_visible`: a child of a
/// hidden group reports invisible while its own flag still says otherwise,
/// and skipping the write there would leak the stale state the moment the
/// group comes back.
pub(super) fn set_visible(widget: &impl IsA<gtk::Widget>, visible: bool) {
    if widget.get_visible() != visible {
        widget.set_visible(visible);
    }
}

pub(super) fn set_sensitive(widget: &impl IsA<gtk::Widget>, sensitive: bool) {
    if widget.is_sensitive() != sensitive {
        widget.set_sensitive(sensitive);
    }
}
