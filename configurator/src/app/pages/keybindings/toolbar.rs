//! Filter, sort, category scope, and bulk reset chrome for the shortcut manager.

use relm4::{gtk, prelude::*};

use gtk::prelude::*;

use crate::messages::Message;
use crate::models::{KeybindingsTabId, ShortcutManagerFilter, ShortcutManagerSort, TabId};

use super::super::super::state::ConfiguratorApp;
use super::super::Binding;
use super::widgets::{connect_clicked, set_accessible_label, set_sensitive, set_visible};

pub(super) fn build_chrome(
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) -> gtk::Box {
    let chrome = gtk::Box::new(gtk::Orientation::Vertical, 8);
    chrome.set_margin_top(8);
    chrome.set_margin_start(12);
    chrome.set_margin_end(12);

    chrome.append(&category_bar(sender, bindings));
    chrome.append(&filter_bar(sender, bindings));
    chrome.append(&actions_bar(sender, bindings));
    chrome.append(&reset_banner(sender, bindings));
    chrome
}

fn category_bar(
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) -> gtk::ScrolledWindow {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let all = scope_button("All Categories", sender, Message::ShortcutManagerShowAll);
    row.append(&all);
    let mut category_buttons = Vec::new();
    for tab in KeybindingsTabId::ALL {
        let button = scope_button(tab.title(), sender, Message::KeybindingsTabSelected(tab));
        row.append(&button);
        category_buttons.push((tab, button));
    }

    bindings.push(Box::new(move |app, summary| {
        let all_active = app.keybindings_show_all;
        set_suggested(&all, all_active);
        for (tab, button) in &category_buttons {
            let visible = summary
                .tab(TabId::Keybindings)
                .is_none_or(|keybindings| keybindings.keybindings_tab_visible(*tab));
            set_visible(button, visible);
            set_suggested(button, !all_active && app.active_keybindings_tab == *tab);
        }
    }));

    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .child(&row)
        .build()
}

fn filter_bar(
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) -> gtk::FlowBox {
    let filters = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(2)
        .max_children_per_line(6)
        .column_spacing(6)
        .row_spacing(6)
        .build();
    let mut buttons = Vec::new();
    for filter in ShortcutManagerFilter::ALL {
        let button = scope_button(
            filter.title(),
            sender,
            Message::ShortcutManagerFilterChanged(filter),
        );
        filters.insert(&button, -1);
        buttons.push((filter, button));
    }
    bindings.push(Box::new(move |app, _summary| {
        for (filter, button) in &buttons {
            set_suggested(button, app.shortcut_filter == *filter);
        }
    }));
    filters
}

fn actions_bar(sender: &ComponentSender<ConfiguratorApp>, bindings: &mut Vec<Binding>) -> gtk::Box {
    let labels: Vec<&str> = ShortcutManagerSort::ALL
        .iter()
        .map(|sort| sort.title())
        .collect();
    let sort = gtk::DropDown::from_strings(&labels);
    set_accessible_label(&sort, "Sort shortcuts");
    {
        let sender = sender.clone();
        sort.connect_selected_notify(move |dropdown| {
            let index = dropdown.selected() as usize;
            if let Some(sort) = ShortcutManagerSort::ALL.get(index).copied() {
                sender.input(Message::ShortcutManagerSortChanged(sort));
            }
        });
    }

    let reset_visible = gtk::Button::with_label("Reset Visible");
    set_accessible_label(&reset_visible, "Reset visible keybindings");
    connect_clicked(
        &reset_visible,
        sender,
        Message::ShortcutResetVisibleRequested,
    );
    let reset_all = gtk::Button::with_label("Reset All");
    set_accessible_label(&reset_all, "Reset all keybindings");
    connect_clicked(&reset_all, sender, Message::ShortcutResetAllRequested);
    let review = gtk::Button::with_label("Review Conflicts");
    set_accessible_label(&review, "Review shortcut conflicts");
    connect_clicked(&review, sender, Message::ShortcutConflictReviewStarted);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&sort);
    row.append(&reset_visible);
    row.append(&reset_all);
    row.append(&review);

    let sort_for_bind = sort.clone();
    bindings.push(Box::new(move |app, _summary| {
        let selected = ShortcutManagerSort::ALL
            .iter()
            .position(|sort| *sort == app.shortcut_sort)
            .unwrap_or(0) as u32;
        if sort_for_bind.selected() != selected {
            sort_for_bind.set_selected(selected);
        }
        let visible_empty = app.visible_keybinding_fields().is_empty();
        let reset_armed = app.shortcut_reset_visible_pending() || app.shortcut_reset_all_pending();
        set_visible(&reset_visible, !reset_armed);
        set_visible(&reset_all, !reset_armed);
        set_sensitive(
            &reset_visible,
            !visible_empty && !app.is_loading && !app.is_saving,
        );
        set_sensitive(&reset_all, !app.is_loading && !app.is_saving);
        set_sensitive(
            &review,
            app.shortcut_manager_summary().has_conflicts()
                && app.pending_shortcut_conflict.is_none(),
        );
    }));
    row
}

fn reset_banner(
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) -> gtk::Revealer {
    let confirm_visible = gtk::Button::builder()
        .label("Confirm Reset Visible")
        .css_classes(["destructive-action"])
        .build();
    set_accessible_label(&confirm_visible, "Confirm reset visible");
    connect_clicked(
        &confirm_visible,
        sender,
        Message::ShortcutResetVisibleConfirmed,
    );
    let confirm_all = gtk::Button::builder()
        .label("Confirm Reset All")
        .css_classes(["destructive-action"])
        .build();
    set_accessible_label(&confirm_all, "Confirm reset all");
    connect_clicked(&confirm_all, sender, Message::ShortcutResetAllConfirmed);
    let cancel = gtk::Button::with_label("Cancel");
    set_accessible_label(&cancel, "Cancel keybinding reset");
    connect_clicked(&cancel, sender, Message::ShortcutResetCanceled);

    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.append(&confirm_visible);
    buttons.append(&confirm_all);
    buttons.append(&cancel);

    let revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .child(&buttons)
        .build();
    let revealer_for_bind = revealer.clone();
    bindings.push(Box::new(move |app, _summary| {
        let visible_armed = app.shortcut_reset_visible_pending();
        let all_armed = app.shortcut_reset_all_pending();
        set_visible(&confirm_visible, visible_armed);
        set_visible(&confirm_all, all_armed);
        set_visible(&cancel, visible_armed || all_armed);
        if let Some(fields) = app.pending_shortcut_reset_visible_fields() {
            let label = format!("Confirm Reset Visible ({})", fields.len());
            if confirm_visible.label().as_deref() != Some(label.as_str()) {
                confirm_visible.set_label(&label);
            }
        }
        let reveal = visible_armed || all_armed;
        if revealer_for_bind.reveals_child() != reveal {
            revealer_for_bind.set_reveal_child(reveal);
        }
    }));
    revealer
}

fn scope_button(
    label: &str,
    sender: &ComponentSender<ConfiguratorApp>,
    message: Message,
) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    set_accessible_label(&button, label);
    connect_clicked(&button, sender, message);
    button
}

fn set_suggested(button: &gtk::Button, suggested: bool) {
    if suggested {
        if !button.has_css_class("suggested-action") {
            button.add_css_class("suggested-action");
        }
    } else if button.has_css_class("suggested-action") {
        button.remove_css_class("suggested-action");
    }
}
