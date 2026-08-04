//! Session page: persistence settings and the saved-session catalog.
//!
//! The settings half is ordinary `PageBuilder` rows. The catalog half is a
//! list whose length the model owns, so each built card owns the typed layout
//! that produced it and its refresh closure. A card and its refresh therefore
//! cannot drift apart, and nothing has to rediscover a control by name or
//! position. Everything that changes without changing what a rebuilt
//! card would render — entry contents, button sensitivity, the two-step
//! clear, search filtering — is written in place by those closures, with the
//! entry handlers blocked so a refresh is never mistaken for typing.

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;
use gtk::glib::SignalHandlerId;

use crate::messages::Message;
use crate::models::{
    SessionCatalogItem, SessionCatalogOperation, SessionCompressionOption,
    SessionStorageModeOption, TabId, TextField, ToggleField,
};

use super::super::search::{AppSearchSummary, SearchArea};
use super::super::state::ConfiguratorApp;
use super::{BuiltPage, PageBuilder, set_text_blocked};

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::Session);

    page.group_in_area("Session Persistence", SearchArea::SessionPersistence)
        .switch_row(
            "Persist transparent mode drawings",
            "",
            |app| app.draft.session_persist_transparent,
            |value| Message::ToggleChanged(ToggleField::SessionPersistTransparent, value),
        )
        .switch_row(
            "Persist whiteboard mode drawings",
            "",
            |app| app.draft.session_persist_whiteboard,
            |value| Message::ToggleChanged(ToggleField::SessionPersistWhiteboard, value),
        )
        .switch_row(
            "Persist blackboard mode drawings",
            "",
            |app| app.draft.session_persist_blackboard,
            |value| Message::ToggleChanged(ToggleField::SessionPersistBlackboard, value),
        )
        .switch_row(
            "Persist undo/redo history",
            "",
            |app| app.draft.session_persist_history,
            |value| Message::ToggleChanged(ToggleField::SessionPersistHistory, value),
        )
        .switch_row(
            "Restore tool state on startup",
            "",
            |app| app.draft.session_restore_tool_state,
            |value| Message::ToggleChanged(ToggleField::SessionRestoreToolState, value),
        )
        .switch_row(
            "Per-output persistence",
            "",
            |app| app.draft.session_per_output,
            |value| Message::ToggleChanged(ToggleField::SessionPerOutput, value),
        );

    page.group_in_area("Storage", SearchArea::SessionPersistence)
        .combo_row(
            "Storage mode",
            "",
            SessionStorageModeOption::list(),
            option_labels(SessionStorageModeOption::list(), |option| option.label()),
            |app| app.draft.session_storage_mode,
            Message::SessionStorageModeChanged,
        );
    custom_directory_row(&mut page);
    page.combo_row(
        "Compression",
        "",
        SessionCompressionOption::list(),
        option_labels(SessionCompressionOption::list(), |option| option.label()),
        |app| app.draft.session_compression,
        Message::SessionCompressionChanged,
    )
    .entry_row_validated(
        "Max shapes per frame",
        |app| app.draft.session_max_shapes_per_frame.clone(),
        |value| Message::TextChanged(TextField::SessionMaxShapesPerFrame, value),
        |app| validate_whole_number(&app.draft.session_max_shapes_per_frame, 1, u64::MAX),
    )
    .entry_row(
        "Max persisted undo depth (blank = runtime limit)",
        |app| app.draft.session_max_persisted_undo_depth.clone(),
        |value| Message::TextChanged(TextField::SessionMaxPersistedUndoDepth, value),
    )
    .entry_row_validated(
        "Max file size (MB)",
        |app| app.draft.session_max_file_size_mb.clone(),
        |value| Message::TextChanged(TextField::SessionMaxFileSizeMb, value),
        |app| validate_whole_number(&app.draft.session_max_file_size_mb, 1, 1024),
    )
    .entry_row_validated(
        "Auto-compress threshold (KB)",
        |app| app.draft.session_auto_compress_threshold_kb.clone(),
        |value| Message::TextChanged(TextField::SessionAutoCompressThresholdKb, value),
        |app| validate_whole_number(&app.draft.session_auto_compress_threshold_kb, 1, u64::MAX),
    )
    .entry_row(
        "Backup retention count",
        |app| app.draft.session_backup_retention.clone(),
        |value| Message::TextChanged(TextField::SessionBackupRetention, value),
    );

    page.group_in_area("Autosave", SearchArea::SessionPersistence)
        .switch_row(
            "Enable autosave",
            "",
            |app| app.draft.session_autosave_enabled,
            |value| Message::ToggleChanged(ToggleField::SessionAutosaveEnabled, value),
        )
        .entry_row_validated(
            "Autosave idle (ms)",
            |app| app.draft.session_autosave_idle_ms.clone(),
            |value| Message::TextChanged(TextField::SessionAutosaveIdleMs, value),
            |app| validate_whole_number(&app.draft.session_autosave_idle_ms, 1000, u64::MAX),
        )
        .entry_row_validated(
            "Autosave interval (ms)",
            |app| app.draft.session_autosave_interval_ms.clone(),
            |value| Message::TextChanged(TextField::SessionAutosaveIntervalMs, value),
            |app| validate_whole_number(&app.draft.session_autosave_interval_ms, 1000, u64::MAX),
        )
        .entry_row_validated(
            "Autosave failure backoff (ms)",
            |app| app.draft.session_autosave_failure_backoff_ms.clone(),
            |value| Message::TextChanged(TextField::SessionAutosaveFailureBackoffMs, value),
            |app| {
                validate_whole_number(
                    &app.draft.session_autosave_failure_backoff_ms,
                    1000,
                    u64::MAX,
                )
            },
        );

    page.group_in_area("Saved Sessions", SearchArea::SessionCatalog);
    catalog_section(&mut page);

    page.finish()
}

fn option_labels<O: Copy>(options: Vec<O>, label: impl Fn(&O) -> &'static str) -> Vec<String> {
    options
        .iter()
        .map(|option| label(option).to_string())
        .collect()
}

/// The custom directory only applies to one storage mode, so it is a row the
/// mode shows rather than a row that is always there and usually inert.
fn custom_directory_row(page: &mut PageBuilder) {
    let row = adw::EntryRow::builder().title("Custom directory").build();
    let handler = {
        let sender = page.sender();
        row.connect_changed(move |row| {
            sender.input(Message::TextChanged(
                TextField::SessionCustomDirectory,
                row.text().to_string(),
            ));
        })
    };
    page.custom(&row);
    page.bind(move |app, _summary| {
        set_visible(
            &row,
            app.draft.session_storage_mode == SessionStorageModeOption::Custom,
        );
        // Blocked: the model owns this text, and a load reporting its own
        // value back as a user edit clears the diagnostics that load produced.
        set_text_blocked(&row, &handler, &app.draft.session_custom_directory);
    });
}

// ---- Catalog -----------------------------------------------------------

fn catalog_section(page: &mut PageBuilder) {
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
            .map(|card| &card.layout)
            .eq(layout.items.iter())
        {
            cards = rebuild_items(&list, &layout, &sender);
        }
        for (item, card) in catalog.items.iter().zip(cards.iter()) {
            let values = catalog_row_values(app, summary, &gates, item);
            (card.refresh)(&values);
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

/// One card's values: the model-owned entry text and every action's state.
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
        clear_armed: clear_armed(catalog.pending_clear_id.as_deref(), id),
        rename,
        duplicate,
        move_target,
    }
}

/// One card's refresh: built beside its card, so it owns that card's typed
/// widget handles and the signal handler ids guarding each write.
type CatalogRowRefresh = Box<dyn Fn(&CatalogRowValues)>;

struct BoundCatalogCard {
    layout: CatalogItemLayout,
    refresh: CatalogRowRefresh,
}

fn rebuild_items(
    list: &gtk::Box,
    layout: &CatalogLayout,
    sender: &ComponentSender<ConfiguratorApp>,
) -> Vec<BoundCatalogCard> {
    // Draining the list, not walking it for a control: the cards that replace
    // these carry their own refresh closures.
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let mut cards = Vec::with_capacity(layout.items.len());
    for item in &layout.items {
        let built = item_card(item, sender);
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

fn item_card(item: &CatalogItemLayout, sender: &ComponentSender<ConfiguratorApp>) -> CatalogRow {
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

        let clear_arming = values.clear_armed && !confirm.get_visible();
        set_visible(&clear, !values.clear_armed);
        set_sensitive(&clear, values.clear_enabled);
        set_visible(&confirm, values.clear_armed);
        if clear_arming {
            // The destructive action just stepped aside. Move keyboard focus
            // to the revealed answer rather than leaving it hidden.
            confirm_button.grab_focus();
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

fn clear_armed(pending: Option<&str>, id: &str) -> bool {
    pending == Some(id)
}

fn item_visible(summary: &AppSearchSummary, id: &str) -> bool {
    !summary.is_active()
        || summary
            .tab(TabId::Session)
            .is_none_or(|tab| tab.session_item_visible(id))
}

/// Error text for a whole-number field, `None` while the input is
/// acceptable. `u64::MAX` as the upper bound means "no maximum" and reports
/// only the minimum, matching the Iced view's two validators.
fn validate_whole_number(value: &str, min: u64, max: u64) -> Option<String> {
    let Ok(parsed) = value.trim().parse::<u64>() else {
        return Some("Expected a whole number".to_string());
    };
    if (min..=max).contains(&parsed) {
        return None;
    }
    Some(if max == u64::MAX {
        format!("Minimum: {min}")
    } else {
        format!("Range: {min}-{max}")
    })
}

// ---- Widget helpers ----------------------------------------------------

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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::models::SessionCatalogItem;
    use crate::models::session::SessionArtifactSummary;

    fn test_item(id: &str, display_name: &str) -> SessionCatalogItem {
        SessionCatalogItem {
            id: id.to_string(),
            display_name: display_name.to_string(),
            path: PathBuf::from(format!("/tmp/{id}.wayscriber-session")),
            path_label: format!("/tmp/{id}.wayscriber-session"),
            canonical_path_label: None,
            created_label: "2026-01-01 10:00".to_string(),
            last_opened_label: "Never".to_string(),
            last_saved_label: "Never".to_string(),
            artifacts: SessionArtifactSummary {
                primary_exists: true,
                backup_exists: false,
                recovery_exists: false,
                clear_marker_exists: false,
                lock_exists: false,
                non_lock_size_bytes: 2048,
            },
        }
    }

    fn app_with_items(items: Vec<SessionCatalogItem>) -> ConfiguratorApp {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        app.session_catalog.replace_items(items);
        app
    }

    /// The caret guarantee: typing in a card's entry, arming the clear, or a
    /// running action must not rebuild the card being typed into.
    #[test]
    fn layout_ignores_input_edits_and_pending_clear() {
        let mut app = app_with_items(vec![test_item("one", "First")]);
        let baseline = catalog_layout(&app);

        app.session_catalog
            .rename_inputs
            .insert("one".to_string(), "First draft".to_string());
        app.session_catalog.pending_clear_id = Some("one".to_string());
        app.session_catalog.busy = true;

        assert_eq!(catalog_layout(&app), baseline);
        // The edit rides the card's values instead, where a blocked write can
        // put it in the entry without a rebuild.
        let summary = app.search_summary();
        let values = catalog_row_values(
            &app,
            &summary,
            &CatalogGates::of(&app),
            &app.session_catalog.items[0],
        );
        assert_eq!(values.rename, "First draft");
    }

    #[test]
    fn layout_tracks_rendered_item_text() {
        let renamed = catalog_layout(&app_with_items(vec![test_item("one", "Renamed")]));
        let original = catalog_layout(&app_with_items(vec![test_item("one", "First")]));
        assert_ne!(renamed, original);

        let two_items = catalog_layout(&app_with_items(vec![
            test_item("one", "First"),
            test_item("two", "Second"),
        ]));
        assert_ne!(two_items, original);
    }

    #[test]
    fn loading_and_an_empty_catalog_both_build_no_cards() {
        let empty = app_with_items(Vec::new());
        let loading = ConfiguratorApp::new_app().0;

        assert!(loading.session_catalog.is_loading);
        // Nothing is rendered while loading, so nothing is built for it and
        // there is no card left over to refresh.
        assert!(catalog_layout(&empty).items.is_empty());
        assert!(catalog_layout(&loading).items.is_empty());
    }

    /// A card's buttons follow the model, and the rename button waits for an
    /// edit that is actually a change.
    #[test]
    fn a_busy_catalog_leaves_every_action_unpressable() {
        let mut app = app_with_items(vec![test_item("one", "First")]);
        let summary = app.search_summary();
        let idle = catalog_row_values(
            &app,
            &summary,
            &CatalogGates::of(&app),
            &app.session_catalog.items[0],
        );
        assert!(idle.actions_enabled);
        assert!(!idle.rename_enabled, "an unchanged name is not a rename");

        app.session_catalog
            .rename_inputs
            .insert("one".to_string(), "Second".to_string());
        let changed = catalog_row_values(
            &app,
            &summary,
            &CatalogGates::of(&app),
            &app.session_catalog.items[0],
        );
        assert!(changed.rename_enabled);

        app.session_catalog.busy = true;
        let busy = catalog_row_values(
            &app,
            &summary,
            &CatalogGates::of(&app),
            &app.session_catalog.items[0],
        );
        assert!(!busy.actions_enabled);
        assert!(!busy.rename_enabled);
        assert!(!busy.duplicate_enabled);
        assert!(!busy.move_enabled);
    }

    #[test]
    fn clear_confirmation_is_armed_for_one_row_only() {
        assert!(clear_armed(Some("one"), "one"));
        assert!(!clear_armed(Some("one"), "two"));
        assert!(!clear_armed(None, "one"));
    }

    /// Confirming consumes the pending id as it sets the catalog busy, so the
    /// card leaves its armed state in the same refresh that starts the work:
    /// the Confirm/Cancel pair goes away and the button that asks comes back
    /// unpressable, rather than offering a confirm the model would refuse.
    #[test]
    fn a_confirmed_clear_collapses_the_armed_row_into_the_busy_one() {
        let mut app = app_with_items(vec![test_item("one", "First")]);
        app.session_catalog.pending_clear_id = Some("one".to_string());
        let summary = app.search_summary();
        let armed = catalog_row_values(
            &app,
            &summary,
            &CatalogGates::of(&app),
            &app.session_catalog.items[0],
        );
        assert!(armed.clear_armed);

        // What `handle_session_catalog_clear_confirmed` leaves behind: the
        // answered question consumed, the clear running.
        app.session_catalog.pending_clear_id = None;
        app.session_catalog.busy = true;
        let running = catalog_row_values(
            &app,
            &summary,
            &CatalogGates::of(&app),
            &app.session_catalog.items[0],
        );

        assert!(!running.clear_armed);
        assert!(!running.clear_enabled);
    }

    #[test]
    fn whole_number_validation_matches_the_old_hints() {
        assert_eq!(validate_whole_number("1000", 1000, u64::MAX), None);
        assert_eq!(
            validate_whole_number("999", 1000, u64::MAX).as_deref(),
            Some("Minimum: 1000")
        );
        assert_eq!(
            validate_whole_number("", 1, u64::MAX).as_deref(),
            Some("Expected a whole number")
        );
        assert_eq!(
            validate_whole_number("-4", 1, u64::MAX).as_deref(),
            Some("Expected a whole number")
        );
        assert_eq!(validate_whole_number(" 512 ", 1, 1024), None);
        assert_eq!(
            validate_whole_number("2048", 1, 1024).as_deref(),
            Some("Range: 1-1024")
        );
    }
}
