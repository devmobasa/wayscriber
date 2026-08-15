use relm4::{adw, gtk};

use adw::prelude::*;

use super::super::pages;
use super::super::state::{ConfiguratorApp, StatusMessage};
use super::AppWidgets;

pub(super) fn refresh(app: &ConfiguratorApp, widgets: &mut AppWidgets) {
    // Header chrome.
    let subtitle = if app.is_dirty { "Unsaved changes" } else { "" };
    if widgets.window_title.subtitle() != subtitle {
        widgets.window_title.set_subtitle(subtitle);
    }
    // A color field the parser rejects is an edit that never reached the
    // draft, so Save is not offered while one is on screen: pressing it
    // would write the last value that parsed and lose the text being typed.
    let save_enabled = app.is_dirty
        && !app.is_saving
        && !app.is_loading
        && app.invalid_color_hex_count() == 0
        && app.pending_shortcut_conflict.is_none();
    if widgets.save_button.is_sensitive() != save_enabled {
        widgets.save_button.set_sensitive(save_enabled);
    }
    let busy = app.is_loading || app.is_saving;
    if widgets.reload_button.is_sensitive() == busy {
        widgets.reload_button.set_sensitive(!busy);
    }
    // The armed confirmation is the model's, so which of the two Defaults
    // affordances is on screen follows it: asking is offered until the
    // question stands, answering only while it does.
    let defaults_armed = app.defaults_reset_pending();
    let defaults_was_armed = widgets.defaults_confirm_button.get_visible();
    let defaults_arming = defaults_armed && !defaults_was_armed;
    let defaults_return_focus = !defaults_armed
        && defaults_was_armed
        && (widgets.defaults_confirm_button.has_focus()
            || widgets.defaults_cancel_button.has_focus());
    set_visible(&widgets.defaults_button, !defaults_armed);
    set_visible(&widgets.defaults_confirm_button, defaults_armed);
    set_visible(&widgets.defaults_cancel_button, defaults_armed);
    if defaults_arming {
        // The Defaults button just stepped aside. Keep keyboard users in
        // the revealed flow instead of leaving focus on a hidden widget.
        widgets.defaults_confirm_button.grab_focus();
    } else if defaults_return_focus {
        // Cancel and Confirm both remove the answer controls. Return the
        // keyboard user to the action that owns this header location.
        widgets.defaults_button.grab_focus();
    }

    // Status strip.
    let (status_text, status_class) = match &app.status {
        StatusMessage::Idle => ("", None),
        StatusMessage::Info(text) => (text.as_str(), None),
        StatusMessage::Success(text) => (text.as_str(), Some("success")),
        StatusMessage::Warning(text) => (text.as_str(), Some("warning")),
        StatusMessage::Error(text) => (text.as_str(), Some("error")),
        StatusMessage::Confirmation(prompt) => (prompt.message(), Some("warning")),
    };
    if widgets.status_label.text() != status_text {
        widgets.status_label.set_text(status_text);
        for class in ["success", "warning", "error"] {
            widgets.status_label.remove_css_class(class);
        }
        if let Some(class) = status_class {
            widgets.status_label.add_css_class(class);
        }
    }
    widgets
        .status_revealer
        .set_reveal_child(!status_text.is_empty());

    // Migration offer.
    let migration_text = app
        .pending_migration()
        .map(super::super::update::migration_offer_text)
        .unwrap_or_default();
    if widgets.migration_seen != migration_text {
        widgets.migration_label.set_text(&migration_text);
        widgets.migration_seen = migration_text.clone();
    }
    widgets
        .migration_revealer
        .set_reveal_child(!migration_text.is_empty());

    // Navigation: model decides, widgets follow.
    let stack_name = pages::stack_name(app.active_tab);
    if widgets.stack.visible_child_name().as_deref() != Some(stack_name) {
        widgets.stack.set_visible_child_name(stack_name);
    }
    let selected = widgets
        .sidebar_rows
        .iter()
        .find(|(tab, _)| *tab == app.active_tab)
        .map(|(_, row)| row.clone());
    if let Some(row) = selected
        && widgets.sidebar.selected_row().as_ref() != Some(&row)
    {
        widgets.sidebar.select_row(Some(&row));
    }

    // Search text + one-shot focus grabs.
    let query = app.search_query.raw();
    if widgets.search_entry.text() != query {
        widgets.search_entry.set_text(query);
    }
    if widgets.seen_focus_serial != app.search_focus_serial {
        widgets.seen_focus_serial = app.search_focus_serial;
        widgets.search_entry.grab_focus();
    }

    // Page rows.
    let summary = app.search_summary();
    // `&mut`: a binding may own the state its section needs between
    // refreshes, which is what the dynamic lists keep their built rows in.
    for binding in &mut widgets.bindings {
        binding(app, &summary);
    }
}

/// Writes the widget's own visibility flag, never `is_visible`: a widget
/// inside a hidden parent reports invisible while its own flag still says
/// otherwise, and skipping the write there would leak the stale state the
/// moment the parent comes back.
fn set_visible(widget: &impl IsA<gtk::Widget>, visible: bool) {
    if widget.get_visible() != visible {
        widget.set_visible(visible);
    }
}
