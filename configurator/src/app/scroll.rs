use iced::Task;
use iced::keyboard::{self, Key, key};
use iced::widget::operation::{self, AbsoluteOffset};

use crate::messages::Message;

pub(crate) const CONTENT_SCROLL_ID: &str = "configurator-content-scroll";

const LINE_SCROLL_DELTA_Y: f32 = 64.0;
const PAGE_SCROLL_DELTA_Y: f32 = 360.0;
const EDGE_SCROLL_DELTA_Y: f32 = 1_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentScrollAction {
    Top,
    Bottom,
    LineUp,
    LineDown,
    PageUp,
    PageDown,
}

impl ContentScrollAction {
    pub(crate) fn task(self) -> Task<Message> {
        match self {
            ContentScrollAction::Top => scroll_by_y(-EDGE_SCROLL_DELTA_Y),
            ContentScrollAction::Bottom => scroll_by_y(EDGE_SCROLL_DELTA_Y),
            ContentScrollAction::LineUp => scroll_by_y(-LINE_SCROLL_DELTA_Y),
            ContentScrollAction::LineDown => scroll_by_y(LINE_SCROLL_DELTA_Y),
            ContentScrollAction::PageUp => scroll_by_y(-PAGE_SCROLL_DELTA_Y),
            ContentScrollAction::PageDown => scroll_by_y(PAGE_SCROLL_DELTA_Y),
        }
    }
}

pub(crate) fn content_scroll_action_for_event(
    event: &keyboard::Event,
) -> Option<ContentScrollAction> {
    let keyboard::Event::KeyPressed {
        key,
        physical_key,
        modifiers,
        ..
    } = event
    else {
        return None;
    };

    if !modifiers.is_empty() {
        return None;
    }

    content_scroll_action_for_key(key.as_ref())
        .or_else(|| content_scroll_action_for_physical_key(*physical_key))
}

fn content_scroll_action_for_key(key: Key<&str>) -> Option<ContentScrollAction> {
    match key {
        Key::Named(key::Named::Home) => Some(ContentScrollAction::Top),
        Key::Named(key::Named::End) => Some(ContentScrollAction::Bottom),
        Key::Named(key::Named::ArrowUp) => Some(ContentScrollAction::LineUp),
        Key::Named(key::Named::ArrowDown) => Some(ContentScrollAction::LineDown),
        Key::Named(key::Named::PageUp) => Some(ContentScrollAction::PageUp),
        Key::Named(key::Named::PageDown) => Some(ContentScrollAction::PageDown),
        _ => None,
    }
}

fn content_scroll_action_for_physical_key(
    physical_key: key::Physical,
) -> Option<ContentScrollAction> {
    match physical_key {
        key::Physical::Code(key::Code::Home) => Some(ContentScrollAction::Top),
        key::Physical::Code(key::Code::End) => Some(ContentScrollAction::Bottom),
        key::Physical::Code(key::Code::ArrowUp) => Some(ContentScrollAction::LineUp),
        key::Physical::Code(key::Code::ArrowDown) => Some(ContentScrollAction::LineDown),
        key::Physical::Code(key::Code::PageUp) => Some(ContentScrollAction::PageUp),
        key::Physical::Code(key::Code::PageDown) => Some(ContentScrollAction::PageDown),
        _ => None,
    }
}

fn scroll_by_y(y: f32) -> Task<Message> {
    operation::scroll_by(CONTENT_SCROLL_ID, AbsoluteOffset { x: 0.0, y })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::{Location, Modifiers, key};

    fn key_press(key: Key) -> keyboard::Event {
        keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
            location: Location::Standard,
            modifiers: Modifiers::empty(),
            text: None,
            repeat: false,
        }
    }

    fn physical_key_press(physical_key: key::Physical) -> keyboard::Event {
        keyboard::Event::KeyPressed {
            key: Key::Unidentified,
            modified_key: Key::Unidentified,
            physical_key,
            location: Location::Standard,
            modifiers: Modifiers::empty(),
            text: None,
            repeat: false,
        }
    }

    #[test]
    fn navigation_keys_map_to_content_scroll_actions() {
        let cases = [
            (Key::Named(key::Named::Home), ContentScrollAction::Top),
            (Key::Named(key::Named::End), ContentScrollAction::Bottom),
            (Key::Named(key::Named::ArrowUp), ContentScrollAction::LineUp),
            (
                Key::Named(key::Named::ArrowDown),
                ContentScrollAction::LineDown,
            ),
            (Key::Named(key::Named::PageUp), ContentScrollAction::PageUp),
            (
                Key::Named(key::Named::PageDown),
                ContentScrollAction::PageDown,
            ),
        ];

        for (key, expected) in cases {
            assert_eq!(
                content_scroll_action_for_event(&key_press(key)),
                Some(expected)
            );
        }
    }

    #[test]
    fn modified_navigation_keys_do_not_scroll_content() {
        let event = keyboard::Event::KeyPressed {
            key: Key::Named(key::Named::Home),
            modified_key: Key::Named(key::Named::Home),
            physical_key: key::Physical::Code(key::Code::Home),
            location: Location::Standard,
            modifiers: Modifiers::CTRL,
            text: None,
            repeat: false,
        };

        assert_eq!(content_scroll_action_for_event(&event), None);
    }

    #[test]
    fn physical_navigation_keys_map_to_content_scroll_actions() {
        let cases = [
            (
                key::Physical::Code(key::Code::Home),
                ContentScrollAction::Top,
            ),
            (
                key::Physical::Code(key::Code::End),
                ContentScrollAction::Bottom,
            ),
            (
                key::Physical::Code(key::Code::ArrowUp),
                ContentScrollAction::LineUp,
            ),
            (
                key::Physical::Code(key::Code::ArrowDown),
                ContentScrollAction::LineDown,
            ),
            (
                key::Physical::Code(key::Code::PageUp),
                ContentScrollAction::PageUp,
            ),
            (
                key::Physical::Code(key::Code::PageDown),
                ContentScrollAction::PageDown,
            ),
        ];

        for (physical_key, expected) in cases {
            assert_eq!(
                content_scroll_action_for_event(&physical_key_press(physical_key)),
                Some(expected)
            );
        }
    }
}
