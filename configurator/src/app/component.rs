//! GTK4/libadwaita shell: the Relm4 root component.
//!
//! [`ConfiguratorApp`] stays the single-owner model it was under Iced; this
//! module owns everything GTK. `update` feeds messages through the
//! framework-free dispatch in [`super::update`] and runs the returned
//! [`Effect`]s as Relm4 commands, whose results come back as ordinary
//! messages through `update_cmd`. `update_view` is the one place widget
//! state is written: shell chrome directly, page rows through the bindings
//! the page builders registered ([`super::pages`]).

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;

use crate::messages::{CommandMessage, Message};
use crate::models::{StartupRequest, TabId};

use super::effects::Effect;
use super::pages::{self, Binding};
use super::search::AppSearchSummary;
use super::state::{ConfiguratorApp, StatusMessage};
use super::{daemon_setup, io, session_catalog};

/// GApplication id. A valid dotted id is required by GLib; the window still
/// advertises this as its Wayland app-id, so compositor rules and the
/// `.desktop` `StartupWMClass` must match it.
const APP_ID: &str = "org.wayscriber.Configurator";

pub(crate) fn run(startup: StartupRequest) {
    // Every launch is its own window, as it was under Iced: the overlay and
    // the tray spawn `wayscriber-configurator --open <destination>` and
    // expect that destination to land. GApplication's default uniqueness
    // would instead forward a bare activation to an already-running
    // instance and drop the request on the floor. Concurrent windows are
    // safe: the guarded `ConfigDocument` save revision arbitrates writes.
    let application = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let app = RelmApp::from_app(application);
    // GTK must not parse our argv: `--open <destination>` is ours and was
    // consumed by `StartupRequest::from_args` already.
    app.with_args(Vec::new()).run::<ConfiguratorApp>(startup);
}

pub(crate) struct AppWidgets {
    window_title: adw::WindowTitle,
    status_label: gtk::Label,
    status_revealer: gtk::Revealer,
    migration_revealer: gtk::Revealer,
    migration_label: gtk::Label,
    /// Last migration text rendered, so the label is only rewritten when the
    /// offer actually changes.
    migration_seen: String,
    save_button: gtk::Button,
    defaults_button: gtk::Button,
    defaults_confirm_button: gtk::Button,
    defaults_cancel_button: gtk::Button,
    reload_button: gtk::Button,
    sidebar_rows: Vec<(TabId, gtk::ListBoxRow)>,
    sidebar: gtk::ListBox,
    stack: gtk::Stack,
    search_entry: gtk::SearchEntry,
    /// Focus-request serial already honored, so one request grabs focus once
    /// instead of on every refresh.
    seen_focus_serial: u64,
    bindings: Vec<Binding>,
}

impl Component for ConfiguratorApp {
    type Init = StartupRequest;
    type Input = Message;
    type Output = ();
    type CommandOutput = CommandMessage;
    type Root = adw::ApplicationWindow;
    type Widgets = AppWidgets;

    fn init_root() -> Self::Root {
        adw::ApplicationWindow::builder()
            .default_width(1000)
            .default_height(680)
            .build()
    }

    fn init(
        startup: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (model, effects) = ConfiguratorApp::new_app_with_startup(startup);
        for effect in effects {
            spawn_effect(effect, &sender);
        }

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
        for (tab, built) in pages::build_all(&sender) {
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
            controller.connect_key_pressed(move |_, key, _, modifiers| {
                if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                    && matches!(key, gtk::gdk::Key::f | gtk::gdk::Key::F)
                {
                    sender.input(Message::SearchFocusRequested);
                    return gtk::glib::Propagation::Stop;
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

        let widgets = AppWidgets {
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
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Message, sender: ComponentSender<Self>, _root: &Self::Root) {
        for effect in self.update_message(message) {
            spawn_effect(effect, &sender);
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        for effect in self.update_command(message) {
            spawn_effect(effect, &sender);
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {
        // Header chrome.
        let subtitle = if self.is_dirty { "Unsaved changes" } else { "" };
        if widgets.window_title.subtitle() != subtitle {
            widgets.window_title.set_subtitle(subtitle);
        }
        // A color field the parser rejects is an edit that never reached the
        // draft, so Save is not offered while one is on screen: pressing it
        // would write the last value that parsed and lose the text being typed.
        let save_enabled = self.is_dirty
            && !self.is_saving
            && !self.is_loading
            && self.invalid_color_hex_count() == 0;
        if widgets.save_button.is_sensitive() != save_enabled {
            widgets.save_button.set_sensitive(save_enabled);
        }
        let busy = self.is_loading || self.is_saving;
        if widgets.reload_button.is_sensitive() == busy {
            widgets.reload_button.set_sensitive(!busy);
        }
        // The armed confirmation is the model's, so which of the two Defaults
        // affordances is on screen follows it: asking is offered until the
        // question stands, answering only while it does.
        let defaults_armed = self.defaults_reset_pending;
        let defaults_arming = defaults_armed && !widgets.defaults_confirm_button.get_visible();
        set_visible(&widgets.defaults_button, !defaults_armed);
        set_visible(&widgets.defaults_confirm_button, defaults_armed);
        set_visible(&widgets.defaults_cancel_button, defaults_armed);
        if defaults_arming {
            // The Defaults button just stepped aside. Keep keyboard users in
            // the revealed flow instead of leaving focus on a hidden widget.
            widgets.defaults_confirm_button.grab_focus();
        }

        // Status strip.
        let (status_text, status_class) = match &self.status {
            StatusMessage::Idle => ("", None),
            StatusMessage::Info(text) => (text.as_str(), None),
            StatusMessage::Success(text) => (text.as_str(), Some("success")),
            StatusMessage::Warning(text) => (text.as_str(), Some("warning")),
            StatusMessage::Error(text) => (text.as_str(), Some("error")),
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
        let migration_text = self
            .pending_migration()
            .map(super::update::migration_offer_text)
            .unwrap_or_default();
        if widgets.migration_seen != migration_text {
            widgets.migration_label.set_text(&migration_text);
            widgets.migration_seen = migration_text.clone();
        }
        widgets
            .migration_revealer
            .set_reveal_child(!migration_text.is_empty());

        // Navigation: model decides, widgets follow.
        let stack_name = pages::stack_name(self.active_tab);
        if widgets.stack.visible_child_name().as_deref() != Some(stack_name) {
            widgets.stack.set_visible_child_name(stack_name);
        }
        let selected = widgets
            .sidebar_rows
            .iter()
            .find(|(tab, _)| *tab == self.active_tab)
            .map(|(_, row)| row.clone());
        if let Some(row) = selected
            && widgets.sidebar.selected_row().as_ref() != Some(&row)
        {
            widgets.sidebar.select_row(Some(&row));
        }

        // Search text + one-shot focus grabs.
        let query = self.search_query.raw();
        if widgets.search_entry.text() != query {
            widgets.search_entry.set_text(query);
        }
        if widgets.seen_focus_serial != self.search_focus_serial {
            widgets.seen_focus_serial = self.search_focus_serial;
            widgets.search_entry.grab_focus();
        }

        // Page rows.
        let summary = self.search_summary();
        // `&mut`: a binding may own the state its section needs between
        // refreshes, which is what the dynamic lists keep their built rows in.
        for binding in &mut widgets.bindings {
            binding(self, &summary);
        }
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

/// Runs one effect as a Relm4 command; its result re-enters the component
/// as an ordinary message through `update_cmd`.
fn spawn_effect(effect: Effect, sender: &ComponentSender<ConfiguratorApp>) {
    match effect {
        Effect::LoadConfig => sender.oneshot_command(async {
            CommandMessage::ConfigLoaded(io::load_config_from_disk().await)
        }),
        Effect::SaveConfig { document, config } => sender.oneshot_command(async move {
            CommandMessage::ConfigSaved(io::save_config_to_disk(document, *config).await)
        }),
        Effect::LoadDaemonStatus { request_id } => sender.oneshot_command(async move {
            CommandMessage::DaemonStatusLoaded(
                request_id,
                daemon_setup::load_daemon_runtime_status().await,
            )
        }),
        Effect::PerformDaemonAction {
            action,
            shortcut_input,
        } => sender.oneshot_command(async move {
            CommandMessage::DaemonActionCompleted(
                daemon_setup::perform_daemon_action(action, shortcut_input).await,
            )
        }),
        Effect::LoadSessionCatalog => sender.oneshot_command(async {
            CommandMessage::SessionCatalogLoaded(session_catalog::load_session_catalog().await)
        }),
        Effect::ForgetSessionEntry { id } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::forget_session_catalog_entry(id).await,
            )
        }),
        Effect::RenameSessionEntry { id, display_name } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::rename_session_catalog_entry(id, display_name).await,
            )
        }),
        Effect::DuplicateSessionEntry { id, target } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::duplicate_session_catalog_entry(id, target).await,
            )
        }),
        Effect::MoveSessionEntry { id, target } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::move_session_catalog_entry(id, target).await,
            )
        }),
        Effect::RevealSessionEntry { id } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::reveal_session_catalog_entry(id).await,
            )
        }),
        Effect::ClearSessionToolState { id } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::clear_session_catalog_tool_state_entry(id).await,
            )
        }),
        Effect::ClearSessionEntry { id } => sender.oneshot_command(async move {
            CommandMessage::SessionCatalogActionCompleted(
                session_catalog::clear_session_catalog_entry(id).await,
            )
        }),
    }
}
