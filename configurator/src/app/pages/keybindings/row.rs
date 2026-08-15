use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;
use wayscriber::config::KeyBinding;

use crate::messages::Message;
use crate::models::KeybindingField;
use crate::models::keybindings::{
    field_has_internal_duplicate, field_matches_defaults, parse_keybindings, reset_tooltip,
    serialize_bindings,
};

use super::super::super::search::AppSearchSummary;
use super::super::super::state::ConfiguratorApp;
use super::super::Binding;
use super::recorder::RecorderPopover;
use super::text_editor::TextEditorPopover;
use super::widgets::{
    connect_clicked, icon_button, set_accessible_label, set_label, set_sensitive, set_tooltip,
    set_visible, watch_compact,
};
use crate::models::TabId;

pub(super) fn binding_row(
    group: &adw::PreferencesGroup,
    field: KeybindingField,
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) {
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
    let reset = icon_button("view-refresh-symbolic", "Reset to default");
    connect_clicked(&reset, sender, Message::ShortcutResetRequested(field));
    let edit = icon_button("document-edit-symbolic", "Edit as text");
    connect_clicked(&edit, sender, Message::ShortcutTextEditStarted(field));

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    controls.append(&add);
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

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header.append(&title);
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
    group.add(&row);

    let editor_popover = gtk::Popover::builder().autohide(true).build();
    editor_popover.set_parent(&edit_shortcuts);
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
    let compact_mode = std::rc::Rc::new(std::cell::Cell::new(false));
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
    bindings.push(Box::new(move |app, search_summary| {
        let visible = row_visible(search_summary, field);
        set_visible(&row, visible);

        let value = app.draft.keybindings.value_for(field).unwrap_or_default();
        let parsed = parse_keybindings(value);
        let parse_error = parsed.as_ref().err().cloned();
        match &parsed {
            Ok(bindings) => {
                let labels: Vec<String> = bindings.iter().map(ToString::to_string).collect();
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
            .active_shortcut_recorder
            .as_ref()
            .filter(|recorder| recorder.field == field);
        recorder.refresh(
            recording.is_some(),
            recording
                .map(|recorder| recorder.prompt.as_str())
                .unwrap_or_default(),
        );

        let editing = app
            .shortcut_text_editor
            .as_ref()
            .filter(|editor| editor.field == field);
        let editor_text = editing.map(|editor| editor.text.as_str()).unwrap_or(value);
        let editor_error = editing.and_then(|editor| editor.parse_error());
        text_editor.refresh(editing.is_some(), editor_text, editor_error.as_deref());
    }));
}

fn row_visible(summary: &AppSearchSummary, field: KeybindingField) -> bool {
    summary.tab(TabId::Keybindings).is_none_or(|keybindings| {
        keybindings.keybinding_field_visible(field)
            || keybindings.keybinding_tab_title_visible(field.tab())
    })
}

fn sync_chips(
    flow: &gtk::FlowBox,
    field: KeybindingField,
    bindings: &[KeyBinding],
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
    binding: &KeyBinding,
    sender: &ComponentSender<ConfiguratorApp>,
) -> gtk::Box {
    let label_text = binding.to_string();
    let chip = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    chip.add_css_class("osd");
    chip.set_valign(gtk::Align::Center);
    let label = gtk::Label::builder().label(&label_text).build();
    let remove = icon_button("window-close-symbolic", &format!("Remove {label_text}"));
    let binding = binding.clone();
    let sender = sender.clone();
    remove.connect_clicked(move |_| {
        sender.input(Message::ShortcutRemoved(field, binding.clone()));
    });
    chip.append(&label);
    chip.append(&remove);
    chip
}
