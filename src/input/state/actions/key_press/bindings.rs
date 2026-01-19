use crate::input::events::Key;

pub(super) fn key_to_action_label(key: Key) -> Option<String> {
    match key {
        Key::Char(c) => Some(c.to_string()),
        Key::Escape => Some("Escape".to_string()),
        Key::Return => Some("Return".to_string()),
        Key::Backspace => Some("Backspace".to_string()),
        Key::Space => Some("Space".to_string()),
        Key::F1 => Some("F1".to_string()),
        Key::F2 => Some("F2".to_string()),
        Key::F4 => Some("F4".to_string()),
        Key::F9 => Some("F9".to_string()),
        Key::F10 => Some("F10".to_string()),
        Key::F11 => Some("F11".to_string()),
        Key::F12 => Some("F12".to_string()),
        Key::Menu => Some("Menu".to_string()),
        Key::Up => Some("ArrowUp".to_string()),
        Key::Down => Some("ArrowDown".to_string()),
        Key::Left => Some("ArrowLeft".to_string()),
        Key::Right => Some("ArrowRight".to_string()),
        Key::Delete => Some("Delete".to_string()),
        Key::Home => Some("Home".to_string()),
        Key::End => Some("End".to_string()),
        Key::PageUp => Some("PageUp".to_string()),
        Key::PageDown => Some("PageDown".to_string()),
        _ => None,
    }
}

pub(super) fn fallback_unshifted_label(key: &str) -> Option<&'static str> {
    match key {
        "!" => Some("1"),
        "@" => Some("2"),
        "#" => Some("3"),
        "$" => Some("4"),
        "%" => Some("5"),
        "^" => Some("6"),
        "&" => Some("7"),
        "*" => Some("8"),
        "(" => Some("9"),
        ")" => Some("0"),
        "_" => Some("-"),
        "+" => Some("="),
        "{" => Some("["),
        "}" => Some("]"),
        "|" => Some("\\"),
        ":" => Some(";"),
        "\"" => Some("'"),
        "<" => Some(","),
        ">" => Some("."),
        "?" => Some("/"),
        "~" => Some("`"),
        _ => None,
    }
}
