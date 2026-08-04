use relm4::{ComponentSender, gtk};

use gtk::prelude::*;

use crate::messages::Message;

use super::super::super::state::ConfiguratorApp;
use super::rows::{connect_button, plain_row, row_content_box, set_label};

/// The header's two labels, which restate the id and name rows below them.
pub(super) struct HeaderRow {
    pub(super) row: gtk::ListBoxRow,
    title: gtk::Label,
    id: gtk::Label,
}

impl HeaderRow {
    pub(super) fn set_labels(&self, index: usize, id: &str, name: &str) {
        let title = if name.trim().is_empty() {
            format!("Board {}", index + 1)
        } else {
            name.trim().to_string()
        };
        set_label(&self.title, &title);

        let id_label = if id.trim().is_empty() {
            "id: <unset>".to_string()
        } else {
            format!("id: {}", id.trim())
        };
        set_label(&self.id, &id_label);
    }
}

pub(super) fn build_header_row(
    index: usize,
    expanded: bool,
    sender: &ComponentSender<ConfiguratorApp>,
) -> HeaderRow {
    let labels = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .build();
    let title = gtk::Label::builder()
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let id = gtk::Label::builder()
        .xalign(0.0)
        .css_classes(["dim-label", "caption"])
        .build();
    labels.append(&title);
    labels.append(&id);

    let content = row_content_box(gtk::Orientation::Horizontal);
    content.append(&labels);
    for (label, message) in [
        (
            if expanded { "Collapse" } else { "Expand" },
            Message::BoardsCollapseToggled(index),
        ),
        ("Up", Message::BoardsMoveItemUp(index)),
        ("Down", Message::BoardsMoveItemDown(index)),
        ("Duplicate", Message::BoardsDuplicateItem(index)),
        ("Remove", Message::BoardsRemoveItem(index)),
    ] {
        let button = gtk::Button::builder()
            .label(label)
            .valign(gtk::Align::Center)
            .build();
        connect_button(&button, message, sender);
        content.append(&button);
    }

    HeaderRow {
        row: plain_row(&content),
        title,
        id,
    }
}
