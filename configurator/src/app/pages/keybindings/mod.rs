//! Keybindings page: shortcut chips, recorder, and per-row reset.

mod recorder;
mod row;
mod text_editor;
mod widgets;

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;

use crate::messages::Message;
use crate::models::{KeybindingField, KeybindingsTabId, TabId};

use super::super::search::AppSearchSummary;
use super::super::state::ConfiguratorApp;
use super::{Binding, BuiltPage};
use row::binding_row;
use widgets::{set_accessible_label, set_label, set_visible};

const SECTION_DESCRIPTION: &str =
    "Record a shortcut or sequence, reset one action, or edit the comma-separated list as text.";

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
    let switcher_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .child(&switcher)
        .build();

    let conflict = conflict_banner(sender, &mut bindings);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&conflict);
    content.append(&switcher_scroll);
    content.append(&stack);

    BuiltPage {
        widget: content.upcast(),
        bindings,
    }
}

fn conflict_banner(
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) -> gtk::Revealer {
    let label = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .hexpand(true)
        .build();
    let replace = gtk::Button::builder()
        .label("Replace")
        .css_classes(["suggested-action"])
        .build();
    set_accessible_label(&replace, "Replace conflicting shortcuts");
    {
        let sender = sender.clone();
        replace.connect_clicked(move |_| {
            sender.input(Message::ShortcutConflictReplaceConfirmed);
        });
    }
    let cancel = gtk::Button::builder().label("Cancel").build();
    set_accessible_label(&cancel, "Cancel shortcut conflict");
    {
        let sender = sender.clone();
        cancel.connect_clicked(move |_| {
            sender.input(Message::ShortcutConflictCanceled);
        });
    }
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.append(&replace);
    buttons.append(&cancel);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_margin_top(8);
    row.set_margin_bottom(8);
    row.set_margin_start(12);
    row.set_margin_end(12);
    row.append(&label);
    row.append(&buttons);

    let revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .child(&row)
        .build();

    let revealer_for_bind = revealer.clone();
    bindings.push(Box::new(move |app, _summary| {
        match &app.pending_shortcut_conflict {
            Some(conflict) => {
                set_label(&label, &conflict.prompt());
                replace.set_label(conflict.replace_label());
                set_accessible_label(&replace, conflict.replace_label());
                if !revealer_for_bind.reveals_child() {
                    revealer_for_bind.set_reveal_child(true);
                }
                set_visible(&revealer_for_bind, true);
            }
            None => {
                if revealer_for_bind.reveals_child() {
                    revealer_for_bind.set_reveal_child(false);
                }
            }
        }
    }));
    revealer
}

fn tab_from_name(name: &str) -> Option<KeybindingsTabId> {
    KeybindingsTabId::ALL
        .into_iter()
        .find(|tab| tab.title() == name)
}

fn section_visible(summary: &AppSearchSummary, tab: KeybindingsTabId) -> bool {
    summary
        .tab(TabId::Keybindings)
        .is_none_or(|keybindings| keybindings.keybindings_tab_visible(tab))
}
