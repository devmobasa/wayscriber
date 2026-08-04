use relm4::{ComponentSender, adw, gtk};

use adw::prelude::*;

use crate::messages::Message;
use crate::models::TabId;

use super::super::pages::{self, Binding};
use super::super::search::AppSearchSummary;
use super::super::state::ConfiguratorApp;
use super::AppWidgets;

pub(super) fn build(
    root: &adw::ApplicationWindow,
    sender: &ComponentSender<ConfiguratorApp>,
) -> AppWidgets {
    // ---- Sidebar ----------------------------------------------------
    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text("Search settings")
        .build();
    {
        let sender = sender.clone();
        search_entry.connect_search_changed(move |entry| {
            sender.input(Message::SearchChanged(entry.text().to_string()));
        });
    }
    {
        let sender = sender.clone();
        search_entry.connect_stop_search(move |_| {
            sender.input(Message::SearchCleared);
        });
    }

    let sidebar = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .css_classes(["navigation-sidebar"])
        .build();
    let mut sidebar_rows = Vec::new();
    for tab in TabId::ALL {
        let row = gtk::ListBoxRow::builder()
            .child(
                &gtk::Label::builder()
                    .label(tab.title())
                    .halign(gtk::Align::Start)
                    .margin_top(8)
                    .margin_bottom(8)
                    .margin_start(6)
                    .margin_end(6)
                    .build(),
            )
            .build();
        sidebar.append(&row);
        sidebar_rows.push((tab, row));
    }
    {
        let sender = sender.clone();
        let rows = sidebar_rows.clone();
        sidebar.connect_row_selected(move |_, selected| {
            let Some(selected) = selected else {
                return;
            };
            if let Some((tab, _)) = rows.iter().find(|(_, row)| row == selected) {
                sender.input(Message::TabSelected(*tab));
            }
        });
    }

    let sidebar_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&sidebar)
        .build();
    let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    sidebar_box.set_margin_top(6);
    sidebar_box.set_margin_start(6);
    sidebar_box.set_margin_end(6);
    sidebar_box.append(&search_entry);
    sidebar_box.append(&sidebar_scroll);
    let build_identity = gtk::Label::builder()
        .label(format!(
            "Version {} · commit {}",
            wayscriber::build_info::version(),
            wayscriber::build_info::commit_hash()
        ))
        .css_classes(["dim-label"])
        .halign(gtk::Align::Start)
        .selectable(true)
        .margin_start(6)
        .margin_end(6)
        .margin_bottom(6)
        .build();
    sidebar_box.append(&build_identity);
    let sidebar_page = adw::NavigationPage::builder()
        .title("Wayscriber")
        .child(&sidebar_box)
        .build();

    // ---- Header actions ---------------------------------------------
    let window_title = adw::WindowTitle::new("Wayscriber Configurator", "");
    let reload_button = gtk::Button::with_label("Reload");
    {
        let sender = sender.clone();
        reload_button.connect_clicked(move |_| sender.input(Message::ReloadRequested));
    }
    // Asking for the reset and answering for it are different messages,
    // so they are different controls: while the confirmation stands, the
    // button that asks steps aside for the pair that answers. All three
    // exist from the start and visibility picks between them, which is
    // what keeps a repeat of the same press from ever applying defaults.
    let defaults_button = gtk::Button::with_label("Defaults");
    {
        let sender = sender.clone();
        defaults_button.connect_clicked(move |_| {
            sender.input(Message::ResetToDefaultsRequested);
        });
    }
    let defaults_confirm_button = gtk::Button::builder()
        .label("Confirm Defaults")
        .visible(false)
        .css_classes(["destructive-action"])
        .build();
    {
        let sender = sender.clone();
        defaults_confirm_button.connect_clicked(move |_| {
            sender.input(Message::ResetToDefaultsConfirmed);
        });
    }
    let defaults_cancel_button = gtk::Button::builder()
        .label("Cancel")
        .visible(false)
        .css_classes(["flat"])
        .build();
    {
        let sender = sender.clone();
        defaults_cancel_button.connect_clicked(move |_| {
            sender.input(Message::ResetToDefaultsCanceled);
        });
    }
    let defaults_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    defaults_box.append(&defaults_button);
    defaults_box.append(&defaults_confirm_button);
    defaults_box.append(&defaults_cancel_button);

    let save_button = gtk::Button::with_label("Save");
    save_button.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        save_button.connect_clicked(move |_| sender.input(Message::SaveRequested));
    }

    let header = adw::HeaderBar::builder()
        .title_widget(&window_title)
        .build();
    header.pack_start(&reload_button);
    header.pack_start(&defaults_box);
    header.pack_end(&save_button);

    // ---- Status + migration strip -----------------------------------
    let status_label = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .selectable(true)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(12)
        .margin_end(12)
        .build();
    let status_revealer = gtk::Revealer::builder().child(&status_label).build();

    let migration_label = gtk::Label::builder().wrap(true).xalign(0.0).build();
    let migration_apply = gtk::Button::with_label("Apply Update");
    migration_apply.add_css_class("suggested-action");
    {
        let sender = sender.clone();
        migration_apply.connect_clicked(move |_| {
            sender.input(Message::MigrationApplyRequested);
        });
    }
    let migration_dismiss = gtk::Button::with_label("Dismiss");
    {
        let sender = sender.clone();
        migration_dismiss.connect_clicked(move |_| {
            sender.input(Message::MigrationDismissed);
        });
    }
    let migration_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    migration_buttons.append(&migration_apply);
    migration_buttons.append(&migration_dismiss);
    let migration_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    migration_box.add_css_class("card");
    migration_box.set_margin_start(12);
    migration_box.set_margin_end(12);
    migration_box.set_margin_top(6);
    migration_label.set_margin_top(8);
    migration_label.set_margin_start(8);
    migration_label.set_margin_end(8);
    migration_box.append(&migration_label);
    migration_buttons.set_margin_start(8);
    migration_buttons.set_margin_bottom(8);
    migration_box.append(&migration_buttons);
    let migration_revealer = gtk::Revealer::builder().child(&migration_box).build();

    // ---- Pages -------------------------------------------------------
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vexpand(true)
        .build();
    let mut bindings: Vec<Binding> = Vec::new();
    for (tab, built) in pages::build_all(sender) {
        stack.add_named(&built.widget, Some(pages::stack_name(tab)));
        bindings.extend(built.bindings);
    }

    // Sidebar visibility follows the search summary.
    {
        let rows = sidebar_rows.clone();
        bindings.push(Box::new(
            move |_app: &ConfiguratorApp, summary: &AppSearchSummary| {
                for (tab, row) in &rows {
                    let visible = !summary.is_active() || summary.tab(*tab).is_some();
                    if row.is_visible() != visible {
                        row.set_visible(visible);
                    }
                }
            },
        ));
    }

    let content_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content_box.append(&status_revealer);
    content_box.append(&migration_revealer);
    content_box.append(&stack);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&content_box));
    let content_page = adw::NavigationPage::builder()
        .title("Settings")
        .child(&toolbar_view)
        .build();

    let split = adw::NavigationSplitView::builder()
        .sidebar(&sidebar_page)
        .content(&content_page)
        .build();
    root.set_content(Some(&split));

    // Ctrl+F focuses search from anywhere in the window.
    {
        let sender = sender.clone();
        let controller = gtk::EventControllerKey::new();
        // Run before focused-widget keybindings so Escape always disarms the
        // model-owned confirmation. The handler still returns Proceed, which
        // lets widgets such as SearchEntry perform their own Escape behavior.
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        controller.connect_key_pressed(move |_, key, _, modifiers| {
            if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && matches!(key, gtk::gdk::Key::f | gtk::gdk::Key::F)
            {
                sender.input(Message::SearchFocusRequested);
                return gtk::glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Escape {
                // The model owns which destructive question is current.
                // Propagate as well so a widget with its own Escape
                // behavior does not lose it when no confirmation exists.
                sender.input(Message::ActiveConfirmationCanceled);
            }
            // Tab is the user moving focus deliberately; a still-pending
            // startup search focus must not steal it back later.
            if matches!(key, gtk::gdk::Key::Tab | gtk::gdk::Key::ISO_Left_Tab) {
                sender.input(Message::StartupInteractionObserved);
            }
            gtk::glib::Propagation::Proceed
        });
        root.add_controller(controller);
    }

    // Any click or tap is the same signal: the user is interacting, so
    // the deferred startup search focus (which fires when the initial
    // config load lands) must stand down instead of yanking focus.
    {
        let sender = sender.clone();
        let click = gtk::GestureClick::new();
        click.set_button(0);
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        click.connect_pressed(move |_, _, _, _| {
            sender.input(Message::StartupInteractionObserved);
        });
        root.add_controller(click);
    }

    AppWidgets {
        window_title,
        status_label,
        status_revealer,
        migration_revealer,
        migration_label,
        migration_seen: String::new(),
        save_button,
        defaults_button,
        defaults_confirm_button,
        defaults_cancel_button,
        reload_button,
        sidebar_rows,
        sidebar,
        stack,
        search_entry,
        seen_focus_serial: 0,
        bindings,
    }
}
