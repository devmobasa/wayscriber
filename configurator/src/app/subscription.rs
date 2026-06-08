use iced::{Event, Subscription, event, mouse, touch};

use crate::messages::Message;

use super::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(crate) fn subscription(&self) -> Subscription<Message> {
        event::listen_with(|event, status, _window| message_for_runtime_event(event, status))
    }
}

fn message_for_runtime_event(event: Event, status: event::Status) -> Option<Message> {
    match event {
        Event::Keyboard(keyboard_event) => Some(Message::KeyboardEvent(keyboard_event, status)),
        Event::Mouse(mouse::Event::ButtonPressed(_))
        | Event::Touch(touch::Event::FingerPressed { .. }) => Some(Message::PointerPressed),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::{Point, touch, window};

    #[test]
    fn window_focus_does_not_request_startup_search_focus() {
        let message = message_for_runtime_event(
            Event::Window(window::Event::Focused),
            event::Status::Ignored,
        );

        assert!(message.is_none());
    }

    #[test]
    fn touch_press_clears_search_focus_hint_like_mouse_press() {
        let message = message_for_runtime_event(
            Event::Touch(touch::Event::FingerPressed {
                id: touch::Finger(1),
                position: Point::ORIGIN,
            }),
            event::Status::Ignored,
        );

        assert!(matches!(message, Some(Message::PointerPressed)));
    }

    #[test]
    fn touch_move_does_not_clear_search_focus_hint() {
        let message = message_for_runtime_event(
            Event::Touch(touch::Event::FingerMoved {
                id: touch::Finger(1),
                position: Point::ORIGIN,
            }),
            event::Status::Ignored,
        );

        assert!(message.is_none());
    }
}
