use iced::{Event, Subscription, event, mouse};

use crate::messages::Message;

use super::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(crate) fn subscription(&self) -> Subscription<Message> {
        event::listen_with(|event, status, _window| match event {
            Event::Keyboard(keyboard_event) => Some(Message::KeyboardEvent(keyboard_event, status)),
            Event::Mouse(mouse::Event::ButtonPressed(_)) => Some(Message::PointerPressed),
            _ => None,
        })
    }
}
