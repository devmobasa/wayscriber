//! Keyboard modifier state tracking.

use crate::config::{DragButtonConfig, MouseDragToolsConfig, enums::ColorSpec};
use crate::draw::Color;

use super::tool::DragTool;
use super::{events::MouseButton, tool::Tool};

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

/// Tool mapping for a single drag gesture, optionally with a default color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragBinding {
    pub tool: DragTool,
    pub color: Option<Color>,
}

impl DragBinding {
    pub fn new(tool: DragTool, color: Option<Color>) -> Self {
        Self { tool, color }
    }

    pub fn from_tool(tool: Tool) -> Self {
        Self {
            tool: DragTool::from_tool(tool),
            color: None,
        }
    }

    pub fn button_default() -> Self {
        Self {
            tool: DragTool::Default,
            color: None,
        }
    }
}

/// Tool mapping for drag gestures with optional modifiers on one mouse button.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragButtonBindings {
    pub drag: DragBinding,
    pub shift_drag: DragBinding,
    pub ctrl_drag: DragBinding,
    pub ctrl_shift_drag: DragBinding,
    pub tab_drag: DragBinding,
}

impl DragButtonBindings {
    pub fn legacy_left() -> Self {
        Self {
            drag: DragBinding::from_tool(Tool::Pen),
            shift_drag: DragBinding::from_tool(Tool::Line),
            ctrl_drag: DragBinding::from_tool(Tool::Rect),
            ctrl_shift_drag: DragBinding::from_tool(Tool::Arrow),
            tab_drag: DragBinding::from_tool(Tool::Ellipse),
        }
    }

    pub fn button_default() -> Self {
        Self {
            drag: DragBinding::button_default(),
            shift_drag: DragBinding::button_default(),
            ctrl_drag: DragBinding::button_default(),
            ctrl_shift_drag: DragBinding::button_default(),
            tab_drag: DragBinding::button_default(),
        }
    }

    pub fn binding_for_modifier(self, modifier: DragModifier) -> DragBinding {
        match modifier {
            DragModifier::None => self.drag,
            DragModifier::Shift => self.shift_drag,
            DragModifier::Ctrl => self.ctrl_drag,
            DragModifier::CtrlShift => self.ctrl_shift_drag,
            DragModifier::Tab => self.tab_drag,
        }
    }

    pub fn from_config(config: &DragButtonConfig) -> Self {
        Self {
            drag: DragBinding::new(
                config.drag_tool,
                config.drag_color.as_ref().map(|c| c.to_color()),
            ),
            shift_drag: DragBinding::new(
                config.shift_drag_tool,
                config.shift_drag_color.as_ref().map(|c| c.to_color()),
            ),
            ctrl_drag: DragBinding::new(
                config.ctrl_drag_tool,
                config.ctrl_drag_color.as_ref().map(|c| c.to_color()),
            ),
            ctrl_shift_drag: DragBinding::new(
                config.ctrl_shift_drag_tool,
                config.ctrl_shift_drag_color.as_ref().map(|c| c.to_color()),
            ),
            tab_drag: DragBinding::new(
                config.tab_drag_tool,
                config.tab_drag_color.as_ref().map(|c| c.to_color()),
            ),
        }
    }

    pub fn to_config(self) -> DragButtonConfig {
        DragButtonConfig {
            drag_tool: self.drag.tool,
            drag_color: self.drag.color.map(ColorSpec::from),
            shift_drag_tool: self.shift_drag.tool,
            shift_drag_color: self.shift_drag.color.map(ColorSpec::from),
            ctrl_drag_tool: self.ctrl_drag.tool,
            ctrl_drag_color: self.ctrl_drag.color.map(ColorSpec::from),
            ctrl_shift_drag_tool: self.ctrl_shift_drag.tool,
            ctrl_shift_drag_color: self.ctrl_shift_drag.color.map(ColorSpec::from),
            tab_drag_tool: self.tab_drag.tool,
            tab_drag_color: self.tab_drag.color.map(ColorSpec::from),
        }
    }
}

/// Tool mapping for drag gestures across mouse buttons.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragToolBindings {
    pub left: DragButtonBindings,
    pub right: DragButtonBindings,
    pub middle: DragButtonBindings,
}

impl Default for DragToolBindings {
    fn default() -> Self {
        Self {
            left: DragButtonBindings::legacy_left(),
            right: DragButtonBindings::button_default(),
            middle: DragButtonBindings::button_default(),
        }
    }
}

impl DragToolBindings {
    pub fn tool_for_modifier(self, modifier: DragModifier) -> Tool {
        self.binding_for_button_modifier(MouseButton::Left, modifier)
            .tool
            .as_tool()
            .unwrap_or(Tool::Select)
    }

    pub fn binding_for_button_modifier(
        self,
        button: MouseButton,
        modifier: DragModifier,
    ) -> DragBinding {
        match button {
            MouseButton::Left => self.left,
            MouseButton::Right => self.right,
            MouseButton::Middle => self.middle,
        }
        .binding_for_modifier(modifier)
    }

    pub fn from_config(config: &MouseDragToolsConfig) -> Self {
        Self {
            left: DragButtonBindings::from_config(&config.left),
            right: DragButtonBindings::from_config(&config.right),
            middle: DragButtonBindings::from_config(&config.middle),
        }
    }

    pub fn to_config(self) -> MouseDragToolsConfig {
        MouseDragToolsConfig::from_buttons(
            self.left.to_config(),
            self.right.to_config(),
            self.middle.to_config(),
        )
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
