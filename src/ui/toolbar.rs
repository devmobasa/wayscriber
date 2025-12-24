use crate::config::KeybindingsConfig;
use crate::draw::{Color, EraserKind, FontDescriptor};
use crate::input::{EraserMode, InputState, Tool};
use crate::input::state::{PresetFeedbackKind, PRESET_FEEDBACK_DURATION_MS};
use std::time::Instant;

/// Events emitted by the floating toolbar UI.
#[derive(Debug, Clone)]
pub enum ToolbarEvent {
    SelectTool(Tool),
    SetColor(Color),
    SetThickness(f64),
    NudgeThickness(f64),
    SetMarkerOpacity(f64),
    NudgeMarkerOpacity(f64),
    SetEraserMode(EraserMode),
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
    ZoomIn,
    ZoomOut,
    ResetZoom,
    ToggleZoomLock,
    #[allow(dead_code)]
    RefreshZoomCapture,
    ApplyPreset(usize),
    SavePreset(usize),
    ClearPreset(usize),
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
    /// Drag handle for top toolbar (carries pointer position in toolbar coords)
    MoveTopToolbar {
        x: f64,
        y: f64,
    },
    /// Drag handle for side toolbar (carries pointer position in toolbar coords)
    MoveSideToolbar {
        x: f64,
        y: f64,
    },
}

/// Snapshot of a single preset slot for toolbar display.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetSlotSnapshot {
    pub name: Option<String>,
    pub tool: Tool,
    pub color: Color,
    pub size: f64,
}

/// Snapshot of an in-progress preset feedback animation.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetFeedbackSnapshot {
    pub kind: PresetFeedbackKind,
    pub progress: f32,
}

/// Snapshot of state mirrored to the toolbar UI.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarSnapshot {
    pub active_tool: Tool,
    pub tool_override: Option<Tool>,
    pub color: Color,
    pub thickness: f64,
    pub eraser_size: f64,
    pub thickness_targets_eraser: bool,
    pub thickness_targets_marker: bool,
    pub eraser_kind: EraserKind,
    pub eraser_mode: EraserMode,
    pub marker_opacity: f64,
    pub font: FontDescriptor,
    pub font_size: f64,
    pub text_active: bool,
    pub frozen_active: bool,
    pub zoom_active: bool,
    pub zoom_locked: bool,
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
    /// Whether to show the marker opacity slider section
    pub show_marker_opacity_section: bool,
    /// Number of preset slots to display
    pub preset_slot_count: usize,
    /// Preset slot previews
    pub presets: Vec<Option<PresetSlotSnapshot>>,
    /// Transient preset feedback animations
    pub preset_feedback: Vec<Option<PresetFeedbackSnapshot>>,
    /// Binding hints for tooltips
    pub binding_hints: ToolbarBindingHints,
}

impl ToolbarSnapshot {
    #[allow(dead_code)]
    pub fn from_input(state: &InputState) -> Self {
        Self::from_input_with_bindings(state, ToolbarBindingHints::default())
    }

    pub fn from_input_with_bindings(
        state: &InputState,
        binding_hints: ToolbarBindingHints,
    ) -> Self {
        let frame = state.canvas_set.active_frame();
        let active_tool = state.active_tool();
        let thickness_targets_eraser =
            active_tool == Tool::Eraser || matches!(state.tool_override(), Some(Tool::Eraser));
        let thickness_targets_marker =
            active_tool == Tool::Marker || matches!(state.tool_override(), Some(Tool::Marker));
        let eraser_kind = state.eraser_kind;
        let eraser_mode = state.eraser_mode;
        let thickness_value = if thickness_targets_eraser {
            state.eraser_size
        } else {
            state.current_thickness
        };
        let presets = state
            .presets
            .iter()
            .map(|preset| {
                preset.as_ref().map(|preset| PresetSlotSnapshot {
                    name: preset.name.clone(),
                    tool: preset.tool,
                    color: preset.color.to_color(),
                    size: preset.size,
                })
            })
            .collect();
        let now = Instant::now();
        let duration_secs = PRESET_FEEDBACK_DURATION_MS as f32 / 1000.0;
        let preset_feedback = state
            .preset_feedback
            .iter()
            .map(|entry| {
                entry.as_ref().and_then(|feedback| {
                    let elapsed = now.saturating_duration_since(feedback.started);
                    let progress = (elapsed.as_secs_f32() / duration_secs).clamp(0.0, 1.0);
                    if progress >= 1.0 {
                        None
                    } else {
                        Some(PresetFeedbackSnapshot {
                            kind: feedback.kind,
                            progress,
                        })
                    }
                })
            })
            .collect();
        Self {
            active_tool,
            tool_override: state.tool_override(),
            color: state.current_color,
            thickness: thickness_value,
            eraser_size: state.eraser_size,
            thickness_targets_eraser,
            thickness_targets_marker,
            eraser_kind,
            eraser_mode,
            marker_opacity: state.marker_opacity,
            font: state.font_descriptor.clone(),
            font_size: state.current_font_size,
            text_active: matches!(state.state, crate::input::DrawingState::TextInput { .. }),
            frozen_active: state.frozen_active(),
            zoom_active: state.zoom_active(),
            zoom_locked: state.zoom_locked(),
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
            preset_slot_count: state.preset_slot_count,
            presets,
            preset_feedback,
            binding_hints,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ToolbarBindingHints {
    pub pen: Option<String>,
    pub line: Option<String>,
    pub rect: Option<String>,
    pub ellipse: Option<String>,
    pub arrow: Option<String>,
    pub marker: Option<String>,
    pub highlight: Option<String>,
    pub eraser: Option<String>,
    pub toggle_eraser_mode: Option<String>,
    pub text: Option<String>,
    pub clear: Option<String>,
    pub fill: Option<String>,
    pub toggle_highlight: Option<String>,
}

impl ToolbarBindingHints {
    pub fn for_tool(&self, tool: Tool) -> Option<&str> {
        match tool {
            Tool::Pen => self.pen.as_deref(),
            Tool::Line => self.line.as_deref(),
            Tool::Rect => self.rect.as_deref(),
            Tool::Ellipse => self.ellipse.as_deref(),
            Tool::Arrow => self.arrow.as_deref(),
            Tool::Marker => self.marker.as_deref(),
            Tool::Highlight => self.highlight.as_deref(),
            Tool::Eraser => self.eraser.as_deref(),
            Tool::Select => None,
        }
    }

    pub fn from_keybindings(kb: &KeybindingsConfig) -> Self {
        let first = |v: &Vec<String>| v.first().cloned();
        Self {
            pen: first(&kb.select_pen_tool),
            line: first(&kb.select_line_tool),
            rect: first(&kb.select_rect_tool),
            ellipse: first(&kb.select_ellipse_tool),
            arrow: first(&kb.select_arrow_tool),
            marker: first(&kb.select_marker_tool),
            highlight: first(&kb.select_highlight_tool),
            eraser: first(&kb.select_eraser_tool),
            toggle_eraser_mode: first(&kb.toggle_eraser_mode),
            text: first(&kb.enter_text_mode),
            clear: first(&kb.clear_canvas),
            fill: first(&kb.toggle_fill),
            toggle_highlight: first(&kb.toggle_highlight_tool),
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
            ToolbarEvent::SetThickness(value) => self.set_thickness_for_active_tool(value),
            ToolbarEvent::SetMarkerOpacity(value) => self.set_marker_opacity(value),
            ToolbarEvent::SetEraserMode(mode) => self.set_eraser_mode(mode),
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
            ToolbarEvent::NudgeThickness(delta) => self.nudge_thickness_for_active_tool(delta),
            ToolbarEvent::NudgeMarkerOpacity(delta) => {
                self.set_marker_opacity(self.marker_opacity + delta)
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
                let currently_active =
                    self.highlight_tool_active() || self.click_highlight_enabled();
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
            ToolbarEvent::ZoomIn => {
                self.request_zoom_action(crate::input::ZoomAction::In);
                true
            }
            ToolbarEvent::ZoomOut => {
                self.request_zoom_action(crate::input::ZoomAction::Out);
                true
            }
            ToolbarEvent::ResetZoom => {
                self.request_zoom_action(crate::input::ZoomAction::Reset);
                true
            }
            ToolbarEvent::ToggleZoomLock => {
                self.request_zoom_action(crate::input::ZoomAction::ToggleLock);
                true
            }
            ToolbarEvent::RefreshZoomCapture => {
                self.request_zoom_action(crate::input::ZoomAction::RefreshCapture);
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
            ToolbarEvent::ApplyPreset(slot) => self.apply_preset(slot),
            ToolbarEvent::SavePreset(slot) => self.save_preset(slot),
            ToolbarEvent::ClearPreset(slot) => self.clear_preset(slot),
            ToolbarEvent::MoveTopToolbar { .. } | ToolbarEvent::MoveSideToolbar { .. } => false,
        }
    }
}
