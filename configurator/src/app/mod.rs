mod entry;
mod io;
mod state;
mod update;
mod view;

pub use entry::run;

use iced::executor;
use iced::theme::Theme;
use iced::{Application, Command, Element};

use crate::messages::Message;
use state::ConfiguratorApp;

impl Application for ConfiguratorApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        ConfiguratorApp::new_app()
    }

    fn title(&self) -> String {
        "Wayscriber Configurator (Iced)".to_string()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        self.update_message(message)
    }

    fn view(&self) -> Element<'_, Message> {
        self.view()
    }
}
