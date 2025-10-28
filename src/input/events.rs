//! Generic input event types for cross-backend compatibility.

/// Generic key representation for cross-backend compatibility.
///
/// Backend implementations map their native key codes to these generic
/// key values for unified input handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Some variants used only in specific contexts
pub enum Key {
    /// Regular character key (a-z, 0-9, symbols)
    Char(char),
    /// Escape key
    Escape,
    /// Return/Enter key
    Return,
    /// Backspace key
    Backspace,
    /// Tab key
    Tab,
    /// Space bar
    Space,
    /// Shift modifier
    Shift,
    /// Ctrl modifier
    Ctrl,
    /// Alt modifier
    Alt,
    /// F10 function key (toggle help)
    F10,
    /// F11 function key (open configurator)
    F11,
    /// F12 function key (toggle status bar)
    F12,
    /// Unmapped or unrecognized key
    Unknown,
}

/// Mouse button identification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Left mouse button (primary drawing button)
    Left,
    /// Right mouse button (cancel action)
    Right,
    /// Middle mouse button (currently unused)
    Middle,
}
