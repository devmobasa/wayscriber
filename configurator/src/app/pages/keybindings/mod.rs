//! Keybindings page: bulk shortcut manager over the shared row editor.

mod recorder;
mod row;
mod text_editor;
mod toolbar;
mod widgets;

use std::cell::RefCell;
use std::rc::Rc;

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;

use crate::messages::Message;
use crate::models::{KeybindingField, KeybindingsTabId, keybinding_fields, keybinding_tab};

use super::super::state::ConfiguratorApp;
use super::{Binding, BuiltPage};
use row::{ManagerRefresh, binding_row};
use toolbar::build_chrome;
use widgets::{set_accessible_label, set_label, set_sensitive, set_visible};

const SECTION_DESCRIPTION: &str = "Record a shortcut or sequence, reset one action, or edit the comma-separated list as text. Filter and sort to review many actions at once.";

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut bindings: Vec<Binding> = Vec::new();
    let refresh: ManagerRefresh = Rc::new(RefCell::new(None));
    {
        let refresh = refresh.clone();
        bindings.push(Box::new(move |app, _summary| {
            *refresh.borrow_mut() = Some((
                app.shortcut_manager_summary(),
                app.visible_keybinding_fields(),
            ));
        }));
    }

    let page = adw::PreferencesPage::new();
    let fields = keybinding_fields();
    let mut groups: Vec<(KeybindingsTabId, adw::PreferencesGroup)> = Vec::new();
    let mut slots: Vec<(KeybindingField, adw::PreferencesRow, adw::PreferencesGroup)> = Vec::new();

    for tab in KeybindingsTabId::ALL {
        let group = adw::PreferencesGroup::builder()
            .title(tab.title())
            .description(SECTION_DESCRIPTION)
            .build();
        for field in fields
            .iter()
            .copied()
            .filter(|field| keybinding_tab(*field) == tab)
        {
            let row = binding_row(&group, field, sender, &mut bindings, refresh.clone());
            slots.push((field, row, group.clone()));
        }
        page.add(&group);
        groups.push((tab, group));
    }

    {
        let groups = groups.clone();
        let refresh = refresh.clone();
        bindings.push(Box::new(move |app, _summary| {
            let refresh = refresh.borrow();
            let Some((_summary, visible)) = refresh.as_ref() else {
                return;
            };
            for (tab, group) in &groups {
                let show = (app.keybindings_show_all || app.active_keybindings_tab == *tab)
                    && visible.iter().any(|field| keybinding_tab(*field) == *tab);
                set_visible(group, show);
            }
        }));
    }

    {
        let refresh = refresh.clone();
        let mut last_order: Vec<(KeybindingsTabId, Vec<KeybindingField>)> = KeybindingsTabId::ALL
            .into_iter()
            .map(|tab| (tab, Vec::new()))
            .collect();
        bindings.push(Box::new(move |_app, _summary| {
            let refresh = refresh.borrow();
            let Some((_summary, visible)) = refresh.as_ref() else {
                return;
            };
            for (tab, group) in &groups {
                let ordered: Vec<KeybindingField> = visible
                    .iter()
                    .copied()
                    .filter(|field| keybinding_tab(*field) == *tab)
                    .collect();
                let Some((_, last)) = last_order.iter_mut().find(|(known, _)| *known == *tab)
                else {
                    continue;
                };
                if last.as_slice() == ordered.as_slice() {
                    continue;
                }
                for field in &ordered {
                    if let Some((_, row, home)) = slots.iter().find(|(known, _, _)| known == field)
                    {
                        home.remove(row);
                        group.add(row);
                    }
                }
                *last = ordered;
            }
        }));
    }

    let conflict = conflict_banner(sender, &mut bindings);
    let chrome = build_chrome(sender, &mut bindings);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&chrome);
    content.append(&conflict);
    content.append(&page);

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
    let jump = gtk::Button::with_label("Jump to Conflict");
    set_accessible_label(&jump, "Jump to conflicting action");
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
    buttons.append(&jump);
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

    let jump_field = std::rc::Rc::new(std::cell::Cell::new(None::<KeybindingField>));
    {
        let sender = sender.clone();
        let jump_field = jump_field.clone();
        jump.connect_clicked(move |_| {
            if let Some(field) = jump_field.get() {
                sender.input(Message::ShortcutManagerJumpTo(field));
            }
        });
    }

    let revealer_for_bind = revealer.clone();
    bindings.push(Box::new(move |app, _summary| {
        match &app.pending_shortcut_conflict {
            Some(conflict) => {
                set_label(&label, &conflict.prompt());
                replace.set_label(conflict.replace_label());
                set_accessible_label(&replace, conflict.replace_label());
                let target = conflict.jump_field();
                jump_field.set(target);
                set_sensitive(&jump, target.is_some());
                if !revealer_for_bind.reveals_child() {
                    revealer_for_bind.set_reveal_child(true);
                }
                set_visible(&revealer_for_bind, true);
            }
            None => {
                jump_field.set(None);
                if revealer_for_bind.reveals_child() {
                    revealer_for_bind.set_reveal_child(false);
                }
            }
        }
    }));
    revealer
}
