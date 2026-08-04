//! GTK4/libadwaita shell: the Relm4 root component.
//!
//! [`ConfiguratorApp`] stays the single-owner model it was under Iced; this
//! module owns everything GTK. `update` feeds messages through the
//! framework-free dispatch in [`super::update`] and runs the returned
//! [`Effect`]s as Relm4 commands, whose results come back as ordinary
//! messages through `update_cmd`. `update_view` is the one place widget
//! state is written: shell chrome directly, page rows through the bindings
//! the page builders registered ([`super::pages`]).

mod effects;
mod shell;
mod view;

use relm4::prelude::*;
use relm4::{adw, gtk};

use crate::messages::{CommandMessage, Message};
use crate::models::{StartupRequest, TabId};

use super::pages::Binding;
use super::state::ConfiguratorApp;

/// GApplication id. A valid dotted id is required by GLib; the window still
/// advertises this as its Wayland app-id, so compositor rules and the
/// `.desktop` `StartupWMClass` must match it.
const APP_ID: &str = "org.wayscriber.Configurator";

use effects::spawn_effect;

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

        let widgets = shell::build(&root, &sender);
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
        view::refresh(self, widgets);
    }
}
