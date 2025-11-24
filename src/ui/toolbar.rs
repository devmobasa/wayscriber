use crate::draw::{Color, FontDescriptor};
use crate::input::{InputState, Tool};

/// Events emitted by the floating toolbar UI.
#[derive(Debug, Clone)]
pub enum ToolbarEvent {
    SelectTool(Tool),
    SetColor(Color),
    SetThickness(f64),
    NudgeThickness(f64),
    SetFont(FontDescriptor),
    SetFontSize(f64),
    ToggleFill(bool),
    SetUndoDelay(f64),
    SetRedoDelay(f64),
    UndoAll,
    RedoAll,
    UndoAllDelayed,
    RedoAllDelayed,
    Undo,
    Redo,
    ClearCanvas,
    EnterTextMode,
    ToggleHighlightTool(bool),
    ToggleClickHighlight(bool),
    ToggleFreeze,
    OpenConfigurator,
    OpenConfigFile,
    /// Close the top toolbar panel
    CloseTopToolbar,
    /// Close the side toolbar panel
    CloseSideToolbar,
    /// Pin/unpin the top toolbar (saves to config)
    PinTopToolbar(bool),
    /// Pin/unpin the side toolbar (saves to config)
    PinSideToolbar(bool),
}

/// Snapshot of state mirrored to the toolbar UI.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarSnapshot {
    pub active_tool: Tool,
    pub tool_override: Option<Tool>,
    pub color: Color,
    pub thickness: f64,
    pub font: FontDescriptor,
    pub font_size: f64,
    pub text_active: bool,
    pub frozen_active: bool,
    pub fill_enabled: bool,
    pub undo_available: bool,
    pub redo_available: bool,
    pub click_highlight_enabled: bool,
    pub highlight_tool_active: bool,
    pub undo_all_delay_ms: u64,
    pub redo_all_delay_ms: u64,
    /// Whether the top toolbar is pinned (opens at startup)
    pub top_pinned: bool,
    /// Whether the side toolbar is pinned (opens at startup)
    pub side_pinned: bool,
}

impl ToolbarSnapshot {
    pub fn from_input(state: &InputState) -> Self {
        let frame = state.canvas_set.active_frame();
        Self {
            active_tool: state.active_tool(),
            tool_override: state.tool_override(),
            color: state.current_color,
            thickness: state.current_thickness,
            font: state.font_descriptor.clone(),
            font_size: state.current_font_size,
            text_active: matches!(state.state, crate::input::DrawingState::TextInput { .. }),
            frozen_active: state.frozen_active(),
            fill_enabled: state.fill_enabled,
            undo_available: frame.undo_stack_len() > 0,
            redo_available: frame.redo_stack_len() > 0,
            click_highlight_enabled: state.click_highlight_enabled(),
            highlight_tool_active: state.highlight_tool_active(),
            undo_all_delay_ms: state.undo_all_delay_ms,
            redo_all_delay_ms: state.redo_all_delay_ms,
            top_pinned: state.toolbar_top_pinned,
            side_pinned: state.toolbar_side_pinned,
        }
    }
}

impl InputState {
    /// Applies a toolbar-originated event to the input state.
    ///
    /// Returns true if the event resulted in a state change.
    pub fn apply_toolbar_event(&mut self, event: ToolbarEvent) -> bool {
        match event {
            ToolbarEvent::SelectTool(tool) => {
                if matches!(self.state, crate::input::DrawingState::TextInput { .. }) {
                    self.clear_text_preview_dirty();
                    self.last_text_preview_bounds = None;
                    self.state = crate::input::DrawingState::Idle;
                }
                self.set_tool_override(Some(tool))
            }
            ToolbarEvent::SetColor(color) => self.set_color(color),
            ToolbarEvent::SetThickness(value) => self.set_thickness(value),
            ToolbarEvent::SetFont(descriptor) => self.set_font_descriptor(descriptor),
            ToolbarEvent::SetFontSize(size) => self.set_font_size(size),
            ToolbarEvent::ToggleFill(enable) => self.set_fill_enabled(enable),
            ToolbarEvent::SetUndoDelay(delay_secs) => {
                let min_delay_s = 0.05;
                let clamped_ms = (delay_secs.clamp(min_delay_s, 5.0) * 1000.0).round();
                self.undo_all_delay_ms = clamped_ms as u64;
                true
            }
            ToolbarEvent::SetRedoDelay(delay_secs) => {
                let min_delay_s = 0.05;
                let clamped_ms = (delay_secs.clamp(min_delay_s, 5.0) * 1000.0).round();
                self.redo_all_delay_ms = clamped_ms as u64;
                true
            }
            ToolbarEvent::NudgeThickness(delta) => {
                self.set_thickness(self.current_thickness + delta)
            }
            ToolbarEvent::Undo => {
                self.toolbar_undo();
                true
            }
            ToolbarEvent::Redo => {
                self.toolbar_redo();
                true
            }
            ToolbarEvent::UndoAll => {
                self.undo_all_immediate();
                true
            }
            ToolbarEvent::RedoAll => {
                self.redo_all_immediate();
                true
            }
            ToolbarEvent::UndoAllDelayed => {
                self.start_undo_all_delayed(self.undo_all_delay_ms);
                true
            }
            ToolbarEvent::RedoAllDelayed => {
                self.start_redo_all_delayed(self.redo_all_delay_ms);
                true
            }
            ToolbarEvent::ClearCanvas => {
                self.toolbar_clear();
                true
            }
            ToolbarEvent::EnterTextMode => {
                let _ = self.set_tool_override(None);
                self.toolbar_enter_text_mode();
                true
            }
            ToolbarEvent::ToggleHighlightTool(enable) => {
                let active = self.highlight_tool_active();
                if active == enable {
                    false
                } else {
                    self.set_highlight_tool(enable);
                    true
                }
            }
            ToolbarEvent::ToggleClickHighlight(enable) => {
                let active = self.click_highlight_enabled();
                if active == enable {
                    false
                } else {
                    self.toggle_click_highlight();
                    true
                }
            }
            ToolbarEvent::ToggleFreeze => {
                self.request_frozen_toggle();
                self.needs_redraw = true;
                true
            }
            ToolbarEvent::OpenConfigurator => {
                self.launch_configurator();
                true
            }
            ToolbarEvent::OpenConfigFile => {
                self.open_config_file_default();
                true
            }
            ToolbarEvent::CloseTopToolbar => {
                self.toolbar_top_visible = false;
                self.toolbar_visible = self.toolbar_top_visible && self.toolbar_side_visible;
                true
            }
            ToolbarEvent::CloseSideToolbar => {
                self.toolbar_side_visible = false;
                self.toolbar_visible = self.toolbar_top_visible && self.toolbar_side_visible;
                true
            }
            ToolbarEvent::PinTopToolbar(pin) => {
                if self.toolbar_top_pinned != pin {
                    self.toolbar_top_pinned = pin;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::PinSideToolbar(pin) => {
                if self.toolbar_side_pinned != pin {
                    self.toolbar_side_pinned = pin;
                    true
                } else {
                    false
                }
            }
        }
    }
}
