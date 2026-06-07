use iced::{Subscription, keyboard};

use crate::messages::Message;

use super::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(crate) fn subscription(&self) -> Subscription<Message> {
        keyboard::listen().map(Message::KeyboardEvent)
    }
}
