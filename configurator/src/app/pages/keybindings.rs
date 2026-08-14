//! Keybindings page: nine sections of shortcut lists.
//!
//! The section in view is model state, not widget state: the switcher sends
//! `KeybindingsTabSelected` and a binding drives the stack from
//! `active_keybindings_tab`, so a deep link or the search realignment that
//! moves the active section shows up here without anyone touching the stack.

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;

use crate::messages::Message;
use crate::models::keybindings::parse_keybinding_list;
use crate::models::{KeybindingField, KeybindingsTabId, TabId};

use super::super::search::AppSearchSummary;
use super::super::state::ConfiguratorApp;
use super::{Binding, BuiltPage, set_text_blocked};

/// The format hint the Iced page carried in its heading.
const SECTION_DESCRIPTION: &str = "Shortcut lists, separated by commas.";

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vexpand(true)
        .build();
    let mut bindings: Vec<Binding> = Vec::new();
    let mut sections: Vec<(KeybindingsTabId, adw::PreferencesPage)> = Vec::new();
    let fields = KeybindingField::all();

    for tab in KeybindingsTabId::ALL {
        let section = adw::PreferencesPage::new();
        let group = adw::PreferencesGroup::builder()
            .description(SECTION_DESCRIPTION)
            .build();
        section.add(&group);
        for field in fields.iter().copied().filter(|field| field.tab() == tab) {
            binding_row(&group, field, sender, &mut bindings);
        }
        stack.add_titled(&section, Some(tab.title()), tab.title());
        sections.push((tab, section));
    }

    {
        let sender = sender.clone();
        stack.connect_visible_child_name_notify(move |stack| {
            let Some(name) = stack.visible_child_name() else {
                return;
            };
            let Some(tab) = tab_from_name(&name) else {
                return;
            };
            sender.input(Message::KeybindingsTabSelected(tab));
        });
    }

    {
        let stack = stack.clone();
        bindings.push(Box::new(move |app, summary| {
            // Reveal before switching and hide after: the stack drops a
            // visible child that goes invisible, and picking the section the
            // model asks for first keeps that fallback out of the way.
            for (tab, section) in &sections {
                if section_visible(summary, *tab) && !section.is_visible() {
                    section.set_visible(true);
                }
            }
            let name = app.active_keybindings_tab.title();
            if stack.visible_child_name().as_deref() != Some(name) {
                stack.set_visible_child_name(name);
            }
            for (tab, section) in &sections {
                if !section_visible(summary, *tab) && section.is_visible() {
                    section.set_visible(false);
                }
            }
        }));
    }

    let switcher = gtk::StackSwitcher::builder()
        .stack(&stack)
        .halign(gtk::Align::Center)
        .margin_top(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    // Nine section buttons are wider than a narrow window; let them scroll
    // sideways rather than set a floor on the window's width.
    let switcher_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .child(&switcher)
        .build();

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&switcher_scroll);
    content.append(&stack);

    BuiltPage {
        widget: content.upcast(),
        bindings,
    }
}

/// One shortcut field: the draft's comma-joined list, with the default
/// binding alongside it the way the Iced row showed it.
fn binding_row(
    group: &adw::PreferencesGroup,
    field: KeybindingField,
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) {
    let row = adw::EntryRow::builder().title(field.label()).build();
    let handler = {
        let sender = sender.clone();
        row.connect_changed(move |row| {
            sender.input(Message::KeybindingChanged(field, row.text().to_string()));
        })
    };
    // Bounded so a two-shortcut default cannot crowd out the entry; the
    // whole list stays readable in the tooltip.
    let default_label = gtk::Label::builder()
        .css_classes(["dim-label", "caption"])
        .valign(gtk::Align::Center)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(28)
        .build();
    row.add_suffix(&default_label);
    group.add(&row);

    bindings.push(Box::new(move |app, summary| {
        let value = app.draft.keybindings.value_for(field).unwrap_or_default();
        // Blocked: the draft owns this list, and a load reporting its own
        // value back as a user edit clears that load's diagnostics.
        set_text_blocked(&row, &handler, value);

        let parse_error = parse_keybinding_list(value).err();
        let has_error_class = row.has_css_class("error");
        match parse_error {
            Some(message) => {
                if !has_error_class {
                    row.add_css_class("error");
                }
                if row.tooltip_text().as_deref() != Some(message.as_str()) {
                    row.set_tooltip_text(Some(&message));
                }
            }
            None => {
                if has_error_class {
                    row.remove_css_class("error");
                }
                if row.tooltip_text().is_some() {
                    row.set_tooltip_text(None);
                }
            }
        }

        let default = app
            .defaults
            .keybindings
            .value_for(field)
            .unwrap_or_default();
        let default_text = if default.trim().is_empty() {
            String::new()
        } else {
            format!("Default: {default}")
        };
        if default_label.label() != default_text {
            default_label.set_label(&default_text);
        }
        let tooltip = (!default_text.is_empty()).then_some(default_text.as_str());
        if default_label.tooltip_text().as_deref() != tooltip {
            default_label.set_tooltip_text(tooltip);
        }

        let visible = row_visible(summary, field);
        if row.is_visible() != visible {
            row.set_visible(visible);
        }
    }));
}

fn tab_from_name(name: &str) -> Option<KeybindingsTabId> {
    KeybindingsTabId::ALL
        .into_iter()
        .find(|tab| tab.title() == name)
}

/// Whether a section still has anything the search asked for, which is also
/// what decides if the switcher offers it.
fn section_visible(summary: &AppSearchSummary, tab: KeybindingsTabId) -> bool {
    summary
        .tab(TabId::Keybindings)
        .is_none_or(|keybindings| keybindings.keybindings_tab_visible(tab))
}

/// A field row shows when it matched, or when its section's title did.
fn row_visible(summary: &AppSearchSummary, field: KeybindingField) -> bool {
    summary.tab(TabId::Keybindings).is_none_or(|keybindings| {
        keybindings.keybinding_field_visible(field)
            || keybindings.keybinding_tab_title_visible(field.tab())
    })
}
