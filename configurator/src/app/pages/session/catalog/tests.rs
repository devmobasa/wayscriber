use std::path::PathBuf;

use super::*;
use crate::app::state::PendingConfirmation;
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
    app.pending_confirmation = Some(PendingConfirmation::SessionClear("one".to_string()));
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
/// card leaves its armed state in the same refresh that starts the work: the
/// Confirm/Cancel pair goes away and the button that asks comes back
/// unpressable, rather than offering a confirm the model would refuse.
#[test]
fn a_confirmed_clear_collapses_the_armed_row_into_the_busy_one() {
    let mut app = app_with_items(vec![test_item("one", "First")]);
    app.pending_confirmation = Some(PendingConfirmation::SessionClear("one".to_string()));
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
    app.pending_confirmation = None;
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
fn catalog_focus_fallback_leaves_the_tab_chain_when_busy_work_finishes() {
    assert!(
        !should_release_catalog_focus_fallback(true, true),
        "the temporary target remains available while row actions are disabled"
    );
    assert!(
        should_release_catalog_focus_fallback(false, true),
        "an idle refresh returns focus to an action and removes the fallback"
    );
    assert!(
        !should_release_catalog_focus_fallback(false, false),
        "ordinary idle browsing does not add or manage a structural tab stop"
    );
}
