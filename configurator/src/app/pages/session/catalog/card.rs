//! Construction and in-place refresh of one saved-session catalog card.

use relm4::gtk;
use relm4::prelude::*;

use gtk::glib::SignalHandlerId;
use gtk::prelude::*;

use crate::messages::Message;

use super::super::super::super::state::ConfiguratorApp;
use super::super::super::set_text_blocked;
use super::{
    CatalogItemLayout, CatalogLayout, CatalogRowValues, body_label, caption_label, hint_label,
    message_button, row_box, set_sensitive, set_visible,
};

type CatalogRowRefresh = Box<dyn Fn(&CatalogRowValues)>;

pub(super) struct BoundCatalogCard {
    layout: CatalogItemLayout,
    refresh: CatalogRowRefresh,
}

impl BoundCatalogCard {
    pub(super) fn layout(&self) -> &CatalogItemLayout {
        &self.layout
    }

    pub(super) fn refresh(&self, values: &CatalogRowValues) {
        (self.refresh)(values);
    }
}

pub(super) fn rebuild_items(
    list: &gtk::Box,
    layout: &CatalogLayout,
    sender: &ComponentSender<ConfiguratorApp>,
    catalog_focus_target: &gtk::Box,
) -> Vec<BoundCatalogCard> {
    // Draining the list, not walking it for a control: the cards that replace
    // these carry their own refresh closures.
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let mut cards = Vec::with_capacity(layout.items.len());
    for item in &layout.items {
        let built = item_card(item, sender, catalog_focus_target);
        list.append(&built.card);
        cards.push(BoundCatalogCard {
            layout: item.clone(),
            refresh: built.refresh,
        });
    }
    cards
}

/// One catalog card: the widget, and the closure that writes the values the
/// layout deliberately left out.
struct CatalogRow {
    card: gtk::Box,
    refresh: CatalogRowRefresh,
}

fn item_card(
    item: &CatalogItemLayout,
    sender: &ComponentSender<ConfiguratorApp>,
    catalog_focus_target: &gtk::Box,
) -> CatalogRow {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["card"])
        .build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    card.append(&content);

    let header = row_box();
    let name = body_label(&item.display_name);
    name.add_css_class("heading");
    header.append(&name);
    header.append(&hint_label(&item.artifacts));
    content.append(&header);

    content.append(&caption_label(&item.path_label));
    if let Some(canonical) = item.canonical_path_label.as_deref() {
        content.append(&caption_label(&format!("Canonical: {canonical}")));
    }
    let times = row_box();
    times.append(&hint_label(&format!("Created: {}", item.created_label)));
    times.append(&hint_label(&format!("Opened: {}", item.last_opened_label)));
    times.append(&hint_label(&format!("Saved: {}", item.last_saved_label)));
    content.append(&times);

    let rename = input_row(
        InputRow {
            id: &item.id,
            placeholder: "Display name",
            button_label: "Save Name",
            on_input: Message::SessionCatalogRenameInputChanged,
            request: Message::SessionCatalogRenameRequested(item.id.clone()),
        },
        sender,
    );
    content.append(&rename.container);
    let duplicate = input_row(
        InputRow {
            id: &item.id,
            placeholder: "Duplicate target path",
            button_label: "Duplicate",
            on_input: Message::SessionCatalogDuplicateInputChanged,
            request: Message::SessionCatalogDuplicateRequested(item.id.clone()),
        },
        sender,
    );
    content.append(&duplicate.container);
    let move_row = input_row(
        InputRow {
            id: &item.id,
            placeholder: "Move target path",
            button_label: "Move",
            on_input: Message::SessionCatalogMoveInputChanged,
            request: Message::SessionCatalogMoveRequested(item.id.clone()),
        },
        sender,
    );
    content.append(&move_row.container);

    let actions = row_box();
    let reveal = message_button(
        "Reveal File",
        sender,
        Message::SessionCatalogRevealRequested(item.id.clone()),
    );
    actions.append(&reveal);
    let tool_state = message_button(
        "Clear Tool State",
        sender,
        Message::SessionCatalogClearToolStateRequested(item.id.clone()),
    );
    actions.append(&tool_state);
    content.append(&actions);

    let danger = row_box();
    let clear = message_button(
        "Clear Saved Data",
        sender,
        Message::SessionCatalogClearRequested(item.id.clone()),
    );
    clear.add_css_class("destructive-action");
    danger.append(&clear);

    // Both halves of the two-step clear exist from the start; arming the
    // pending id swaps which one is visible.
    let confirm = row_box();
    let confirm_button = message_button(
        "Confirm Clear",
        sender,
        Message::SessionCatalogClearConfirmed(item.id.clone()),
    );
    confirm_button.add_css_class("destructive-action");
    confirm.append(&confirm_button);
    let cancel_button = message_button(
        "Cancel",
        sender,
        Message::SessionCatalogClearCanceled(item.id.clone()),
    );
    cancel_button.add_css_class("flat");
    confirm.append(&cancel_button);
    danger.append(&confirm);

    let forget = message_button(
        "Forget",
        sender,
        Message::SessionCatalogForgetRequested(item.id.clone()),
    );
    forget.add_css_class("flat");
    danger.append(&forget);
    content.append(&danger);

    let handle = card.clone();
    let catalog_focus_target = catalog_focus_target.clone();
    let refresh: CatalogRowRefresh = Box::new(move |values| {
        set_visible(&handle, values.visible);

        // Blocked: these entries carry text the model owns, and a refresh
        // reporting it back as typing would pin an input the user never made.
        set_text_blocked(&rename.entry, &rename.handler, &values.rename);
        set_sensitive(&rename.button, values.rename_enabled);
        set_text_blocked(&duplicate.entry, &duplicate.handler, &values.duplicate);
        set_sensitive(&duplicate.button, values.duplicate_enabled);
        set_text_blocked(&move_row.entry, &move_row.handler, &values.move_target);
        set_sensitive(&move_row.button, values.move_enabled);

        set_sensitive(&reveal, values.actions_enabled);
        set_sensitive(&forget, values.actions_enabled);
        set_sensitive(&tool_state, values.tool_state_enabled);

        let clear_was_armed = confirm.get_visible();
        let answer_has_focus = confirm_button.has_focus() || cancel_button.has_focus();
        let focus_after_refresh = clear_focus_after_refresh(
            clear_was_armed,
            values.clear_armed,
            values.clear_enabled,
            answer_has_focus,
        );
        if focus_after_refresh == ClearFocusTarget::Catalog {
            // Confirm enters the busy state and disables the row's actions.
            // Temporarily make the stable page target focusable and move there
            // before hiding the focused answer controls. The page binding
            // removes it from the tab chain as soon as the catalog is idle.
            catalog_focus_target.set_focusable(true);
            catalog_focus_target.grab_focus();
        }
        set_visible(&clear, !values.clear_armed);
        set_sensitive(&clear, values.clear_enabled);
        set_visible(&confirm, values.clear_armed);
        match focus_after_refresh {
            ClearFocusTarget::Confirm => {
                // The destructive action just stepped aside. Move keyboard
                // focus to the revealed answer rather than leaving it hidden.
                confirm_button.grab_focus();
            }
            ClearFocusTarget::ClearAction => {
                // Cancel restores the still-enabled action in this row.
                clear.grab_focus();
            }
            ClearFocusTarget::Catalog | ClearFocusTarget::Unchanged => {}
        }
    });

    CatalogRow { card, refresh }
}

struct InputRow<'a> {
    id: &'a str,
    placeholder: &'a str,
    button_label: &'a str,
    on_input: fn(String, String) -> Message,
    request: Message,
}

/// An entry the model owns beside the button that acts on it, kept with the
/// handler a refresh has to block before writing the entry.
struct InputRowWidgets {
    container: gtk::Box,
    entry: gtk::Entry,
    handler: SignalHandlerId,
    button: gtk::Button,
}

fn input_row(row: InputRow<'_>, sender: &ComponentSender<ConfiguratorApp>) -> InputRowWidgets {
    let container = row_box();
    let entry = gtk::Entry::builder()
        .hexpand(true)
        .placeholder_text(row.placeholder)
        .build();
    let handler = {
        let sender = sender.clone();
        let id = row.id.to_string();
        let on_input = row.on_input;
        entry.connect_changed(move |entry| {
            sender.input(on_input(id.clone(), entry.text().to_string()));
        })
    };
    container.append(&entry);

    let button = message_button(row.button_label, sender, row.request);
    container.append(&button);
    InputRowWidgets {
        container,
        entry,
        handler,
        button,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClearFocusTarget {
    Unchanged,
    Confirm,
    ClearAction,
    Catalog,
}

fn clear_focus_after_refresh(
    was_armed: bool,
    is_armed: bool,
    clear_enabled: bool,
    answer_has_focus: bool,
) -> ClearFocusTarget {
    if is_armed && !was_armed {
        return ClearFocusTarget::Confirm;
    }
    if was_armed && !is_armed && answer_has_focus {
        return if clear_enabled {
            ClearFocusTarget::ClearAction
        } else {
            ClearFocusTarget::Catalog
        };
    }
    ClearFocusTarget::Unchanged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_confirmation_focus_has_enabled_targets_for_every_transition() {
        assert_eq!(
            clear_focus_after_refresh(false, true, true, false),
            ClearFocusTarget::Confirm,
            "arming focuses the newly revealed confirmation"
        );
        assert_eq!(
            clear_focus_after_refresh(true, false, true, true),
            ClearFocusTarget::ClearAction,
            "cancel returns to the restored clear action"
        );
        assert_eq!(
            clear_focus_after_refresh(true, false, false, true),
            ClearFocusTarget::Catalog,
            "confirm moves away from the row actions disabled by busy state"
        );
        assert_eq!(
            clear_focus_after_refresh(true, false, false, false),
            ClearFocusTarget::Unchanged,
            "a pointer-triggered transition does not steal keyboard focus"
        );
    }
}
