use std::cell::RefCell;
use std::rc::Rc;

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;
use wayscriber::config::Shortcut;

use crate::messages::Message;
use crate::models::KeybindingField;
use crate::models::keybindings::{
    ShortcutManagerSummary, field_has_internal_duplicate, field_matches_defaults,
    parse_keybindings, reset_tooltip, serialize_bindings,
};

use super::super::super::state::ConfiguratorApp;
use super::super::Binding;
use super::recorder::RecorderPopover;
use super::text_editor::TextEditorPopover;
use super::widgets::{
    connect_clicked, icon_button, set_accessible_label, set_label, set_sensitive, set_tooltip,
    set_visible, unparent_on_destroy, watch_compact,
};

pub(super) type ManagerRefresh =
    Rc<RefCell<Option<(ShortcutManagerSummary, Vec<KeybindingField>)>>>;

pub(super) fn binding_row(
    group: &adw::PreferencesGroup,
    field: KeybindingField,
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
    refresh: ManagerRefresh,
) -> adw::PreferencesRow {
    let row = adw::PreferencesRow::builder().title(field.label()).build();
    let title = gtk::Label::builder()
        .label(field.label())
        .xalign(0.0)
        .hexpand(true)
        .wrap(true)
        .css_classes(["title"])
        .build();

    let chips = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(1)
        .max_children_per_line(8)
        .column_spacing(6)
        .row_spacing(6)
        .hexpand(true)
        .build();
    let add = icon_button("input-keyboard-symbolic", "Add shortcut");
    connect_clicked(&add, sender, Message::ShortcutRecordingStarted(field));
    let record_sequence = gtk::Button::builder()
        .label("Record Sequence")
        .valign(gtk::Align::Center)
        .build();
    set_accessible_label(&record_sequence, "Record sequence");
    set_tooltip(
        &record_sequence,
        Some("Record a two- or three-chord keyboard sequence"),
    );
    connect_clicked(
        &record_sequence,
        sender,
        Message::ShortcutSequenceRecordingStarted(field),
    );
    let reset = icon_button("view-refresh-symbolic", "Reset to default");
    connect_clicked(&reset, sender, Message::ShortcutResetRequested(field));
    let edit = icon_button("document-edit-symbolic", "Edit as text");
    connect_clicked(&edit, sender, Message::ShortcutTextEditStarted(field));

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    controls.append(&add);
    controls.append(&record_sequence);
    controls.append(&reset);
    controls.append(&edit);

    let editor = gtk::Box::new(gtk::Orientation::Vertical, 8);
    editor.append(&chips);
    editor.append(&controls);

    let compact_summary = gtk::Label::builder()
        .css_classes(["dim-label"])
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(24)
        .xalign(1.0)
        .build();
    let edit_shortcuts = gtk::Button::builder()
        .label("Edit Shortcuts")
        .valign(gtk::Align::Center)
        .build();
    set_accessible_label(&edit_shortcuts, "Edit shortcuts");
    let compact = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    compact.set_halign(gtk::Align::End);
    compact.append(&compact_summary);
    compact.append(&edit_shortcuts);
    compact.set_visible(false);

    let badges = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    badges.set_halign(gtk::Align::End);
    badges.set_valign(gtk::Align::Center);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header.append(&title);
    header.append(&badges);
    header.append(&compact);

    let caption = gtk::Label::builder()
        .css_classes(["dim-label", "caption"])
        .wrap(true)
        .xalign(0.0)
        .build();

    let inline_host = gtk::Box::new(gtk::Orientation::Vertical, 8);
    inline_host.append(&editor);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.set_margin_top(10);
    body.set_margin_bottom(10);
    body.set_margin_start(12);
    body.set_margin_end(12);
    body.append(&header);
    body.append(&inline_host);
    body.append(&caption);
    row.set_child(Some(&body));
    row.set_can_focus(true);
    group.add(&row);
    {
        let sender = sender.clone();
        let focus = gtk::EventControllerFocus::new();
        focus.connect_enter(move |_| {
            sender.input(Message::ShortcutManagerRowSelected(field));
        });
        row.add_controller(focus);
    }

    let editor_popover = gtk::Popover::builder().autohide(true).build();
    editor_popover.set_parent(&edit_shortcuts);
    unparent_on_destroy(&edit_shortcuts, &editor_popover);
    {
        let editor_popover = editor_popover.clone();
        let editor = editor.clone();
        edit_shortcuts.connect_clicked(move |_| {
            if editor.parent().as_ref() != Some(editor_popover.upcast_ref()) {
                editor.unparent();
                editor_popover.set_child(Some(&editor));
            }
            editor_popover.popup();
        });
    }
    {
        let editor = editor.clone();
        let inline_host = inline_host.clone();
        editor_popover.connect_closed(move |_| {
            if editor.parent().as_ref() != Some(inline_host.upcast_ref()) {
                editor.unparent();
                inline_host.append(&editor);
            }
        });
    }

    let recorder = RecorderPopover::build(&row, field, sender);
    let text_editor = TextEditorPopover::build(&row, field, sender);
    let seen_chips = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let seen_badges = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let compact_mode = std::rc::Rc::new(std::cell::Cell::new(false));
    let last_focus_serial = std::rc::Rc::new(std::cell::Cell::new(0));
    {
        let compact = compact.clone();
        let inline_host = inline_host.clone();
        let compact_mode = compact_mode.clone();
        let editor_popover = editor_popover.clone();
        let editor = editor.clone();
        watch_compact(&row, move |is_compact| {
            compact_mode.set(is_compact);
            set_visible(&compact, is_compact);
            set_visible(&inline_host, !is_compact);
            if !is_compact && editor_popover.is_visible() {
                editor_popover.popdown();
                if editor.parent().as_ref() != Some(inline_host.upcast_ref()) {
                    editor.unparent();
                    inline_host.append(&editor);
                }
            }
        });
    }

    let sender = sender.clone();
    let row_for_bind = row.clone();
    bindings.push(Box::new(move |app, _search_summary| {
        let refresh = refresh.borrow();
        let Some((summary, visible)) = refresh.as_ref() else {
            return;
        };
        let row_is_visible = visible.contains(&field);
        set_visible(&row_for_bind, row_is_visible);

        let value = app.draft.keybindings.value_for(field).unwrap_or_default();
        let parsed = parse_keybindings(value);
        let parse_error = parsed.as_ref().err().cloned();
        match &parsed {
            Ok(bindings) => {
                let labels: Vec<String> = bindings.iter().map(Shortcut::display_label).collect();
                if seen_chips.borrow().as_slice() != labels.as_slice() {
                    sync_chips(&chips, field, bindings, &sender);
                    *seen_chips.borrow_mut() = labels;
                }
                let summary_text = if bindings.is_empty() {
                    "Unbound".to_string()
                } else {
                    serialize_bindings(bindings)
                };
                set_label(&compact_summary, &summary_text);
                set_tooltip(&compact_summary, Some(&summary_text));
            }
            Err(message) => {
                if !seen_chips.borrow().is_empty() {
                    clear_flow(&chips);
                    seen_chips.borrow_mut().clear();
                }
                set_label(&compact_summary, value);
                set_tooltip(&compact_summary, Some(message));
            }
        }

        let default_tooltip = reset_tooltip(&app.defaults.keybindings, field);
        set_tooltip(&reset, Some(&default_tooltip));
        set_accessible_label(&reset, &default_tooltip);
        let at_defaults =
            field_matches_defaults(&app.draft.keybindings, &app.defaults.keybindings, field);
        set_sensitive(&add, parse_error.is_none());
        set_sensitive(&record_sequence, parse_error.is_none());
        set_sensitive(&reset, !at_defaults);

        let caption_text = match &parse_error {
            Some(message) => message.clone(),
            None if field_has_internal_duplicate(&app.draft.keybindings, field) => {
                "This action lists the same shortcut twice.".to_string()
            }
            None => {
                let default = app
                    .defaults
                    .keybindings
                    .value_for(field)
                    .unwrap_or_default();
                if default.trim().is_empty() {
                    "Default: Unbound".to_string()
                } else {
                    format!("Default: {default}")
                }
            }
        };
        set_label(&caption, &caption_text);
        if parse_error.is_some() || field_has_internal_duplicate(&app.draft.keybindings, field) {
            if !caption.has_css_class("error") {
                caption.add_css_class("error");
            }
        } else if caption.has_css_class("error") {
            caption.remove_css_class("error");
        }

        let recording = app
            .shortcuts
            .recorder()
            .filter(|recorder| recorder.field == field);
        recorder.refresh(recording);

        let editing = app
            .shortcuts
            .editor()
            .filter(|editor| editor.field == field);
        let editor_text = editing.map(|editor| editor.text.as_str()).unwrap_or(value);
        let editor_error = editing.and_then(|editor| editor.parse_error());
        text_editor.refresh(editing.is_some(), editor_text, editor_error.as_deref());

        if let Some(manager_row) = summary.row(field) {
            let titles: Vec<String> = manager_row
                .badge_titles()
                .into_iter()
                .map(str::to_string)
                .collect();
            if seen_badges.borrow().as_slice() != titles.as_slice() {
                sync_badges(&badges, &titles);
                *seen_badges.borrow_mut() = titles;
            }
        }

        let selected = app.selected_keybinding == Some(field);
        if selected {
            if !title.has_css_class("accent") {
                title.add_css_class("accent");
            }
        } else if title.has_css_class("accent") {
            title.remove_css_class("accent");
        }
        if selected && row_is_visible && app.keybinding_focus_serial > last_focus_serial.get() {
            last_focus_serial.set(app.keybinding_focus_serial);
            row_for_bind.grab_focus();
        }
    }));
    row
}

fn sync_chips(
    flow: &gtk::FlowBox,
    field: KeybindingField,
    bindings: &[Shortcut],
    sender: &ComponentSender<ConfiguratorApp>,
) {
    clear_flow(flow);
    for binding in bindings {
        flow.insert(&shortcut_chip(field, binding, sender), -1);
    }
}

fn clear_flow(flow: &gtk::FlowBox) {
    while let Some(child) = flow.first_child() {
        flow.remove(&child);
    }
}

fn shortcut_chip(
    field: KeybindingField,
    binding: &Shortcut,
    sender: &ComponentSender<ConfiguratorApp>,
) -> gtk::Box {
    let label_text = binding.display_label();
    // Application CSS (`.shortcut-chip-key`) draws the raised key. GTK's
    // `.keycap` class only applies under ShortcutLabel, so `+ x` would
    // otherwise stay plain text beside the clear button.
    let chip = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    chip.set_valign(gtk::Align::Center);
    let label = gtk::Label::builder()
        .label(&label_text)
        .css_classes(["shortcut-chip-key"])
        .valign(gtk::Align::Center)
        .build();
    let remove_label = format!("Remove {label_text}");
    let remove = gtk::Button::builder()
        .icon_name("edit-clear-symbolic")
        .tooltip_text(&remove_label)
        .valign(gtk::Align::Center)
        .has_frame(false)
        .css_classes(["flat", "circular", "dim-label"])
        .build();
    set_accessible_label(&remove, &remove_label);
    let binding = binding.clone();
    let sender = sender.clone();
    remove.connect_clicked(move |_| {
        sender.input(Message::ShortcutRemoved(field, binding.clone()));
    });
    chip.append(&label);
    chip.append(&remove);
    chip
}

fn sync_badges(host: &gtk::Box, titles: &[String]) {
    while let Some(child) = host.first_child() {
        host.remove(&child);
    }
    for title in titles {
        let badge = gtk::Label::builder()
            .label(title)
            .css_classes(["caption", "dim-label"])
            .build();
        host.append(&badge);
    }
}
