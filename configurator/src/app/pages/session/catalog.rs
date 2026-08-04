//! Saved-session catalog orchestration and model-to-row projection.
//!
//! The catalog owns the dynamic list layout. Each card owns the typed layout
//! that built it and the refresh closure for its stable controls, so layout and
//! widget state cannot drift and entry refreshes never masquerade as typing.

mod card;
#[cfg(test)]
mod tests;

use relm4::gtk;
use relm4::prelude::*;

use gtk::prelude::*;

use crate::messages::Message;
use crate::models::{SessionCatalogItem, SessionCatalogOperation, TabId};

use super::super::super::search::{AppSearchSummary, SearchArea};
use super::super::super::state::ConfiguratorApp;
use super::super::PageBuilder;
use card::{BoundCatalogCard, rebuild_items};

pub(super) fn add(page: &mut PageBuilder) {
    page.group_in_area("Saved Sessions", SearchArea::SessionCatalog);

    let sender = page.sender();
    let body = column_box();
    let refresh = message_button("Refresh", &sender, Message::SessionCatalogRefreshRequested);
    let toolbar = row_box();
    toolbar.append(&refresh);
    body.append(&toolbar);
    body.append(&hint_label(
        "Clear Tool State applies config defaults without deleting boards.",
    ));
    body.append(&hint_label("Clear Saved Data removes saved session files."));

    let blocker_label = warning_label("");
    body.append(&blocker_label);
    let loading_label = body_label("Loading sessions...");
    body.append(&loading_label);
    let empty_label = body_label("No named sessions in the catalog yet.");
    body.append(&empty_label);

    let list = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    body.append(&list);

    page.custom(&body);
    let mut cards: Vec<BoundCatalogCard> = Vec::new();
    page.bind(move |app, summary| {
        let catalog = &app.session_catalog;
        let gates = CatalogGates::of(app);
        set_sensitive(&refresh, !gates.busy);

        match SessionCatalogOperation::Clear.cached_status_blocker(app.daemon_status.as_ref()) {
            Some(blocker) => {
                set_label(&blocker_label, blocker);
                set_visible(&blocker_label, true);
            }
            None => set_visible(&blocker_label, false),
        }
        set_visible(&loading_label, catalog.is_loading);
        set_visible(
            &empty_label,
            !catalog.is_loading && catalog.items.is_empty(),
        );

        let layout = catalog_layout(app);
        if !cards
            .iter()
            .map(BoundCatalogCard::layout)
            .eq(layout.items.iter())
        {
            cards = rebuild_items(&list, &layout, &sender, &body);
        }
        for (item, card) in catalog.items.iter().zip(cards.iter()) {
            let values = catalog_row_values(app, summary, &gates, item);
            card.refresh(&values);
        }

        // Confirm can temporarily park focus on the catalog while every row
        // action is disabled. Once the operation finishes, return to the
        // enabled Refresh action and remove the structural box from the normal
        // tab chain. If the user already moved focus elsewhere, only remove the
        // temporary focusability; do not steal focus back.
        if should_release_catalog_focus_fallback(gates.busy, body.is_focusable()) {
            if body.has_focus() {
                refresh.grab_focus();
            }
            body.set_focusable(false);
        }
    });
}

/// Everything a rebuilt card would render, and nothing else.
///
/// Entry contents, button states, and the two-step clear are deliberately
/// absent: those are written in place, so typing never destroys the entry
/// being typed into.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct CatalogLayout {
    /// Empty while loading: the list renders nothing then, so there is
    /// nothing to build and nothing to refresh.
    items: Vec<CatalogItemLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CatalogItemLayout {
    id: String,
    display_name: String,
    path_label: String,
    canonical_path_label: Option<String>,
    created_label: String,
    last_opened_label: String,
    last_saved_label: String,
    artifacts: String,
}

fn catalog_layout(app: &ConfiguratorApp) -> CatalogLayout {
    if app.session_catalog.is_loading {
        return CatalogLayout::default();
    }

    CatalogLayout {
        items: app
            .session_catalog
            .items
            .iter()
            .map(|item| CatalogItemLayout {
                id: item.id.clone(),
                display_name: item.display_name.clone(),
                path_label: item.path_label.clone(),
                canonical_path_label: item.canonical_path_label.clone(),
                created_label: item.created_label.clone(),
                last_opened_label: item.last_opened_label.clone(),
                last_saved_label: item.last_saved_label.clone(),
                artifacts: item.artifacts.status_label(),
            })
            .collect(),
    }
}

/// What a card's controls are allowed to do, resolved once per refresh: the
/// blockers are the same answer for every row and cost a lookup each.
struct CatalogGates {
    busy: bool,
    duplicate_blocked: bool,
    move_blocked: bool,
    tool_state_blocked: bool,
    clear_blocked: bool,
}

impl CatalogGates {
    fn of(app: &ConfiguratorApp) -> Self {
        let status = app.daemon_status.as_ref();
        Self {
            busy: app.session_catalog.busy || app.session_catalog.is_loading,
            duplicate_blocked: SessionCatalogOperation::Duplicate
                .cached_status_blocker(status)
                .is_some(),
            move_blocked: SessionCatalogOperation::Move
                .cached_status_blocker(status)
                .is_some(),
            tool_state_blocked: SessionCatalogOperation::ClearToolState
                .cached_status_blocker(status)
                .is_some(),
            clear_blocked: SessionCatalogOperation::Clear
                .cached_status_blocker(status)
                .is_some(),
        }
    }
}

/// One card's model-owned entry text and every action's state.
struct CatalogRowValues {
    visible: bool,
    rename: String,
    rename_enabled: bool,
    duplicate: String,
    duplicate_enabled: bool,
    move_target: String,
    move_enabled: bool,
    /// Reveal and Forget, which only wait on the catalog being idle.
    actions_enabled: bool,
    tool_state_enabled: bool,
    clear_enabled: bool,
    clear_armed: bool,
}

fn catalog_row_values(
    app: &ConfiguratorApp,
    summary: &AppSearchSummary,
    gates: &CatalogGates,
    item: &SessionCatalogItem,
) -> CatalogRowValues {
    let catalog = &app.session_catalog;
    let id = item.id.as_str();
    let rename = catalog.rename_value(id, &item.display_name);
    let duplicate = catalog.duplicate_value(id, &item.path);
    let move_target = catalog.move_value(id, &item.path);
    CatalogRowValues {
        visible: item_visible(summary, id),
        rename_enabled: !gates.busy
            && rename.trim() != item.display_name.trim()
            && !rename.trim().is_empty(),
        duplicate_enabled: !gates.busy && !gates.duplicate_blocked && !duplicate.trim().is_empty(),
        move_enabled: !gates.busy && !gates.move_blocked && !move_target.trim().is_empty(),
        actions_enabled: !gates.busy,
        tool_state_enabled: !gates.busy && !gates.tool_state_blocked,
        clear_enabled: !gates.busy && !gates.clear_blocked,
        clear_armed: clear_armed(app.pending_session_clear_id(), id),
        rename,
        duplicate,
        move_target,
    }
}

fn clear_armed(pending: Option<&str>, id: &str) -> bool {
    pending == Some(id)
}

fn should_release_catalog_focus_fallback(catalog_busy: bool, fallback_focusable: bool) -> bool {
    !catalog_busy && fallback_focusable
}

fn item_visible(summary: &AppSearchSummary, id: &str) -> bool {
    !summary.is_active()
        || summary
            .tab(TabId::Session)
            .is_none_or(|tab| tab.session_item_visible(id))
}

fn column_box() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .build()
}

fn row_box() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build()
}

fn body_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .build()
}

fn caption_label(text: &str) -> gtk::Label {
    let label = body_label(text);
    label.add_css_class("caption");
    label
}

fn hint_label(text: &str) -> gtk::Label {
    let label = caption_label(text);
    label.add_css_class("dim-label");
    label
}

fn warning_label(text: &str) -> gtk::Label {
    let label = caption_label(text);
    label.add_css_class("warning");
    label
}

fn message_button(
    label: &str,
    sender: &ComponentSender<ConfiguratorApp>,
    message: Message,
) -> gtk::Button {
    let button = gtk::Button::builder()
        .label(label)
        .valign(gtk::Align::Center)
        .build();
    let sender = sender.clone();
    button.connect_clicked(move |_| sender.input(message.clone()));
    button
}

fn set_label(label: &gtk::Label, text: &str) {
    if label.label() != text {
        label.set_label(text);
    }
}

/// Writes the widget's own visibility flag, never `is_visible`: a row inside
/// a hidden group reports invisible while its own flag still says otherwise,
/// and skipping the write there would leak the stale state the moment the
/// group comes back.
fn set_visible(widget: &impl IsA<gtk::Widget>, visible: bool) {
    if widget.get_visible() != visible {
        widget.set_visible(visible);
    }
}

fn set_sensitive(widget: &impl IsA<gtk::Widget>, sensitive: bool) {
    if widget.is_sensitive() != sensitive {
        widget.set_sensitive(sensitive);
    }
}
