//! Keyboard modifier state tracking.

use super::tool::Tool;

/// Active drag modifier combination used for tool mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragModifier {
    None,
    Shift,
    Ctrl,
    CtrlShift,
    Tab,
}

impl DragModifier {
    pub fn is_active(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Tool mapping for drag gestures with optional modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DragToolBindings {
    pub drag: Tool,
    pub shift_drag: Tool,
    pub ctrl_drag: Tool,
    pub ctrl_shift_drag: Tool,
    pub tab_drag: Tool,
}

impl Default for DragToolBindings {
    fn default() -> Self {
        Self {
            drag: Tool::Pen,
            shift_drag: Tool::Line,
            ctrl_drag: Tool::Rect,
            ctrl_shift_drag: Tool::Arrow,
            tab_drag: Tool::Ellipse,
        }
    }
}

impl DragToolBindings {
    pub fn tool_for_modifier(self, modifier: DragModifier) -> Tool {
        match modifier {
            DragModifier::None => self.drag,
            DragModifier::Shift => self.shift_drag,
            DragModifier::Ctrl => self.ctrl_drag,
            DragModifier::CtrlShift => self.ctrl_shift_drag,
            DragModifier::Tab => self.tab_drag,
        }
    }
}

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

    /// Returns the active drag modifier combination using the default priority.
    ///
    /// # Priority
    /// 1. Ctrl+Shift → Arrow
    /// 2. Ctrl → Rectangle
    /// 3. Shift → Line
    /// 4. Tab → Ellipse
    /// 5. None
    pub fn active_drag_modifier(&self) -> DragModifier {
        if self.ctrl && self.shift {
            DragModifier::CtrlShift
        } else if self.ctrl {
            DragModifier::Ctrl
        } else if self.shift {
            DragModifier::Shift
        } else if self.tab {
            DragModifier::Tab
        } else {
            DragModifier::None
        }
    }

    /// Determines which drawing tool is active based on current modifier state and drag mapping.
    pub fn current_tool_with_bindings(&self, bindings: DragToolBindings) -> Tool {
        bindings.tool_for_modifier(self.active_drag_modifier())
    }
}
