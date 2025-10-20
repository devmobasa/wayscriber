//! Keyboard modifier state tracking.

use super::tool::Tool;

/// Keyboard modifier state.
///
/// Tracks which modifier keys (Shift, Ctrl, Alt, Tab) are currently pressed.
/// Used to determine the active drawing tool and handle keyboard shortcuts.
#[derive(Debug, Clone, Copy)]
pub struct Modifiers {
    /// Shift key pressed
    pub shift: bool,
    /// Ctrl key pressed
    pub ctrl: bool,
    /// Alt key pressed
    pub alt: bool,
    /// Tab key pressed
    pub tab: bool,
}

impl Default for Modifiers {
    fn default() -> Self {
        Self::new()
    }
}

impl Modifiers {
    /// Creates a new Modifiers instance with all keys released.
    pub fn new() -> Self {
        Self {
            shift: false,
            ctrl: false,
            alt: false,
            tab: false,
        }
    }

    /// Determines which drawing tool is active based on current modifier state.
    ///
    /// # Tool Selection Priority
    /// 1. Ctrl+Shift → Arrow
    /// 2. Ctrl → Rectangle
    /// 3. Shift → Line
    /// 4. Tab → Ellipse
    /// 5. None → Pen (default)
    pub fn current_tool(&self) -> Tool {
        if self.ctrl && self.shift {
            Tool::Arrow
        } else if self.ctrl {
            Tool::Rect
        } else if self.shift {
            Tool::Line
        } else if self.tab {
            Tool::Ellipse
        } else {
            Tool::Pen
        }
    }
}
