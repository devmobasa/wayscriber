use relm4::{ComponentSender, gtk};

use gtk::glib::object::IsA;
use gtk::prelude::*;

use crate::messages::Message;
use crate::models::KeybindingField;

use super::super::super::state::ConfiguratorApp;

pub(super) fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .has_frame(false)
        .css_classes(["flat"])
        .build();
    set_accessible_label(&button, tooltip);
    button
}

pub(super) fn set_accessible_label(widget: &impl gtk::prelude::AccessibleExt, label: &str) {
    widget.update_property(&[gtk::accessible::Property::Label(label)]);
}

/// Writes the widget's own visibility flag, never `is_visible`: a widget
/// inside a hidden parent reports invisible while its own flag still says
/// otherwise, and skipping the write there would leak the stale state the
/// moment the parent comes back.
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

pub(super) fn set_label(label: &gtk::Label, text: &str) {
    if label.label() != text {
        label.set_label(text);
    }
}

pub(super) fn set_tooltip(widget: &impl IsA<gtk::Widget>, tooltip: Option<&str>) {
    if widget.tooltip_text().as_deref() != tooltip {
        widget.set_tooltip_text(tooltip);
    }
}

pub(super) const COMPACT_WIDTH: i32 = 560;

pub(super) fn watch_compact(row: &impl IsA<gtk::Widget>, apply: impl Fn(bool) + 'static) {
    let widget = row.as_ref().clone();
    widget.connect_notify_local(Some("width"), move |widget, _| {
        let width = widget.width();
        if width <= 0 {
            return;
        }
        apply(width < COMPACT_WIDTH);
    });
}

pub(super) fn connect_clicked(
    button: &gtk::Button,
    sender: &ComponentSender<ConfiguratorApp>,
    message: Message,
) {
    let sender = sender.clone();
    button.connect_clicked(move |_| {
        sender.input(message.clone());
    });
}

/// GtkPopover widgets created with `set_parent` are not ordinary children.
/// If they are still attached when the parent is destroyed, GTK warns on
/// finalize. Unparent in the destroy handler so shutdown stays quiet.
pub(super) fn unparent_on_destroy(parent: &impl IsA<gtk::Widget>, popover: &gtk::Popover) {
    let popover = popover.clone();
    parent.connect_destroy(move |_| {
        if popover.parent().is_some() {
            popover.unparent();
        }
    });
}

pub(super) fn field_canceled(
    popover: &gtk::Popover,
    field: KeybindingField,
    sender: &ComponentSender<ConfiguratorApp>,
    to_message: fn(KeybindingField) -> Message,
) {
    let sender = sender.clone();
    popover.connect_closed(move |_| {
        sender.input(to_message(field));
    });
}
