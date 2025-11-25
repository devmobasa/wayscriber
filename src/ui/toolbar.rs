use crate::draw::{Color, FontDescriptor};
use crate::input::{InputState, Tool};

/// Events emitted by the floating toolbar UI.
#[derive(Debug, Clone)]
pub enum ToolbarEvent {
    SelectTool(Tool),
    SetColor(Color),
    SetThickness(f64),
    NudgeThickness(f64),
    SetMarkerOpacity(f64),
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
    /// Toggle both highlight tool and click highlight together
    ToggleAllHighlight(bool),
    ToggleFreeze,
    OpenConfigurator,
    OpenConfigFile,
    ToggleCustomSection(bool),
    ToggleDelaySliders(bool),
    SetCustomUndoDelay(f64),
    SetCustomRedoDelay(f64),
    SetCustomUndoSteps(usize),
    SetCustomRedoSteps(usize),
    CustomUndo,
    CustomRedo,
    /// Close the top toolbar panel
    CloseTopToolbar,
    /// Close the side toolbar panel
    CloseSideToolbar,
    /// Pin/unpin the top toolbar (saves to config)
    PinTopToolbar(bool),
    /// Pin/unpin the side toolbar (saves to config)
    PinSideToolbar(bool),
    /// Toggle between icon mode and text mode
    ToggleIconMode(bool),
    /// Toggle extended color palette
    ToggleMoreColors(bool),
    /// Toggle Actions section visibility (undo all, redo all, etc.)
    ToggleActionsSection(bool),
    /// Toggle marker opacity UI visibility
    ToggleMarkerOpacitySection(bool),
}

/// Snapshot of state mirrored to the toolbar UI.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarSnapshot {
    pub active_tool: Tool,
    pub tool_override: Option<Tool>,
    pub color: Color,
    pub thickness: f64,
    pub marker_opacity: f64,
    pub font: FontDescriptor,
    pub font_size: f64,
    pub text_active: bool,
    pub frozen_active: bool,
    pub fill_enabled: bool,
    pub undo_available: bool,
    pub redo_available: bool,
    pub click_highlight_enabled: bool,
    pub highlight_tool_active: bool,
    /// Whether any highlight feature is active (tool or click)
    pub any_highlight_active: bool,
    pub undo_all_delay_ms: u64,
    pub redo_all_delay_ms: u64,
    pub custom_section_enabled: bool,
    pub show_delay_sliders: bool,
    pub custom_undo_delay_ms: u64,
    pub custom_redo_delay_ms: u64,
    pub custom_undo_steps: usize,
    pub custom_redo_steps: usize,
    /// Whether the top toolbar is pinned (opens at startup)
    pub top_pinned: bool,
    /// Whether the side toolbar is pinned (opens at startup)
    pub side_pinned: bool,
    /// Whether to use icons instead of text labels
    pub use_icons: bool,
    /// Whether to show extended color palette
    pub show_more_colors: bool,
    /// Whether to show the Actions section
    pub show_actions_section: bool,
    /// Whether to show the marker opacity slider in the side toolbar
    pub show_marker_opacity_section: bool,
}

impl ToolbarSnapshot {
    pub fn from_input(state: &InputState) -> Self {
        let frame = state.canvas_set.active_frame();
        Self {
            active_tool: state.active_tool(),
            tool_override: state.tool_override(),
            color: state.current_color,
            thickness: state.current_thickness,
            marker_opacity: state.marker_opacity,
            font: state.font_descriptor.clone(),
            font_size: state.current_font_size,
            text_active: matches!(state.state, crate::input::DrawingState::TextInput { .. }),
            frozen_active: state.frozen_active(),
            fill_enabled: state.fill_enabled,
            undo_available: frame.undo_stack_len() > 0,
            redo_available: frame.redo_stack_len() > 0,
            click_highlight_enabled: state.click_highlight_enabled(),
            highlight_tool_active: state.highlight_tool_active(),
            any_highlight_active: state.click_highlight_enabled() || state.highlight_tool_active(),
            undo_all_delay_ms: state.undo_all_delay_ms,
            redo_all_delay_ms: state.redo_all_delay_ms,
            custom_section_enabled: state.custom_section_enabled,
            show_delay_sliders: state.show_delay_sliders,
            custom_undo_delay_ms: state.custom_undo_delay_ms,
            custom_redo_delay_ms: state.custom_redo_delay_ms,
            custom_undo_steps: state.custom_undo_steps,
            custom_redo_steps: state.custom_redo_steps,
            top_pinned: state.toolbar_top_pinned,
            side_pinned: state.toolbar_side_pinned,
            use_icons: state.toolbar_use_icons,
            show_more_colors: state.show_more_colors,
            show_actions_section: state.show_actions_section,
            show_marker_opacity_section: state.show_marker_opacity_section,
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
            ToolbarEvent::SetMarkerOpacity(value) => self.set_marker_opacity(value),
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
            ToolbarEvent::SetCustomUndoDelay(delay_secs) => {
                let min_delay_s = 0.05;
                let clamped_ms = (delay_secs.clamp(min_delay_s, 5.0) * 1000.0).round();
                self.custom_undo_delay_ms = clamped_ms as u64;
                true
            }
            ToolbarEvent::SetCustomRedoDelay(delay_secs) => {
                let min_delay_s = 0.05;
                let clamped_ms = (delay_secs.clamp(min_delay_s, 5.0) * 1000.0).round();
                self.custom_redo_delay_ms = clamped_ms as u64;
                true
            }
            ToolbarEvent::SetCustomUndoSteps(steps) => {
                let clamped = steps.clamp(1, 500);
                if self.custom_undo_steps != clamped {
                    self.custom_undo_steps = clamped;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::SetCustomRedoSteps(steps) => {
                let clamped = steps.clamp(1, 500);
                if self.custom_redo_steps != clamped {
                    self.custom_redo_steps = clamped;
                    true
                } else {
                    false
                }
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
            ToolbarEvent::CustomUndo => {
                self.start_custom_undo(self.custom_undo_delay_ms, self.custom_undo_steps);
                true
            }
            ToolbarEvent::CustomRedo => {
                self.start_custom_redo(self.custom_redo_delay_ms, self.custom_redo_steps);
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
            ToolbarEvent::ToggleAllHighlight(enable) => {
                // set_highlight_tool already handles both highlight tool and click highlight
                let currently_active = self.highlight_tool_active() || self.click_highlight_enabled();
                if currently_active != enable {
                    self.set_highlight_tool(enable);
                    self.needs_redraw = true;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleFreeze => {
                self.request_frozen_toggle();
                self.needs_redraw = true;
                true
            }
            ToolbarEvent::ToggleCustomSection(enable) => {
                if self.custom_section_enabled != enable {
                    self.custom_section_enabled = enable;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleDelaySliders(show) => {
                if self.show_delay_sliders != show {
                    self.show_delay_sliders = show;
                    true
                } else {
                    false
                }
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
                self.toolbar_visible = self.toolbar_top_visible || self.toolbar_side_visible;
                true
            }
            ToolbarEvent::CloseSideToolbar => {
                self.toolbar_side_visible = false;
                self.toolbar_visible = self.toolbar_top_visible || self.toolbar_side_visible;
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
            ToolbarEvent::ToggleIconMode(use_icons) => {
                if self.toolbar_use_icons != use_icons {
                    self.toolbar_use_icons = use_icons;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleMoreColors(show) => {
                if self.show_more_colors != show {
                    self.show_more_colors = show;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleActionsSection(show) => {
                if self.show_actions_section != show {
                    self.show_actions_section = show;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleMarkerOpacitySection(show) => {
                if self.show_marker_opacity_section != show {
                    self.show_marker_opacity_section = show;
                    true
                } else {
                    false
                }
            }
        }
    }
}
