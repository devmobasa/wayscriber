use std::time::Instant;

use crate::config::ToolbarLayoutMode;
use crate::draw::{Color, EraserKind, FontDescriptor};
use crate::input::state::{PRESET_FEEDBACK_DURATION_MS, PresetFeedbackKind};
use crate::input::{EraserMode, InputState, Tool, ToolbarDrawerTab};

use super::bindings::ToolbarBindingHints;

/// Snapshot of a single preset slot for toolbar display.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetSlotSnapshot {
    pub name: Option<String>,
    pub tool: Tool,
    pub color: Color,
    pub size: f64,
    pub eraser_kind: Option<EraserKind>,
    pub eraser_mode: Option<EraserMode>,
    pub marker_opacity: Option<f64>,
    pub fill_enabled: Option<bool>,
    pub font_size: Option<f64>,
    pub text_background_enabled: Option<bool>,
    pub arrow_length: Option<f64>,
    pub arrow_angle: Option<f64>,
    pub arrow_head_at_end: Option<bool>,
    pub show_status_bar: Option<bool>,
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
    pub note_active: bool,
    pub frozen_active: bool,
    pub zoom_active: bool,
    pub zoom_locked: bool,
    pub fill_enabled: bool,
    pub arrow_label_enabled: bool,
    pub undo_available: bool,
    pub redo_available: bool,
    pub page_index: usize,
    pub page_count: usize,
    pub click_highlight_enabled: bool,
    pub highlight_tool_active: bool,
    /// Whether any highlight feature is active (tool or click)
    pub any_highlight_active: bool,
    pub undo_all_delay_ms: u64,
    pub redo_all_delay_ms: u64,
    pub custom_section_enabled: bool,
    pub show_delay_sliders: bool,
    /// Whether to show delayed undo/redo actions in the toolbar
    pub delay_actions_enabled: bool,
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
    /// Current toolbar layout mode
    pub layout_mode: ToolbarLayoutMode,
    /// Whether to show extended color palette
    pub show_more_colors: bool,
    /// Whether to show the Actions section
    pub show_actions_section: bool,
    /// Whether to show advanced action buttons
    pub show_actions_advanced: bool,
    /// Whether to show zoom actions
    pub show_zoom_actions: bool,
    /// Whether to show the Pages section
    pub show_pages_section: bool,
    /// Whether to show the marker opacity slider section
    pub show_marker_opacity_section: bool,
    /// Whether to show preset action toasts
    pub show_preset_toasts: bool,
    /// Whether to show presets in the side toolbar
    pub show_presets: bool,
    /// Whether to show the Step Undo/Redo section
    pub show_step_section: bool,
    /// Whether to keep text controls visible when text is inactive
    pub show_text_controls: bool,
    /// Whether to show the Settings section
    pub show_settings_section: bool,
    pub show_tool_preview: bool,
    pub show_status_bar: bool,
    /// Whether the simple-mode shape picker is expanded
    pub shape_picker_open: bool,
    /// Whether the drawer is open
    pub drawer_open: bool,
    /// Active drawer tab
    pub drawer_tab: ToolbarDrawerTab,
    /// Number of preset slots to display
    pub preset_slot_count: usize,
    /// Preset slot previews
    pub presets: Vec<Option<PresetSlotSnapshot>>,
    /// Currently active preset slot
    pub active_preset_slot: Option<usize>,
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
        let active_mode = state.board_mode();
        let page_count = state.canvas_set.page_count(active_mode);
        let page_index = state.canvas_set.active_page_index(active_mode);
        let text_active = matches!(state.state, crate::input::DrawingState::TextInput { .. })
            && state.text_input_mode == crate::input::TextInputMode::Plain;
        let note_active = matches!(state.state, crate::input::DrawingState::TextInput { .. })
            && state.text_input_mode == crate::input::TextInputMode::StickyNote;
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
                    eraser_kind: preset.eraser_kind,
                    eraser_mode: preset.eraser_mode,
                    marker_opacity: preset.marker_opacity,
                    fill_enabled: preset.fill_enabled,
                    font_size: preset.font_size,
                    text_background_enabled: preset.text_background_enabled,
                    arrow_length: preset.arrow_length,
                    arrow_angle: preset.arrow_angle,
                    arrow_head_at_end: preset.arrow_head_at_end,
                    show_status_bar: preset.show_status_bar,
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
        let drawer_open = state.toolbar_drawer_open;
        let drawer_tab = state.toolbar_drawer_tab;
        let show_actions_advanced = state.show_actions_advanced;
        let show_zoom_actions = state.show_zoom_actions;
        let show_pages_section = state.show_pages_section;
        let show_step_section = state.show_step_section;
        let show_settings_section = state.show_settings_section;
        let delay_actions_enabled = state.show_step_section && state.show_delay_sliders;

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
            text_active,
            note_active,
            frozen_active: state.frozen_active(),
            zoom_active: state.zoom_active(),
            zoom_locked: state.zoom_locked(),
            fill_enabled: state.fill_enabled,
            arrow_label_enabled: state.arrow_label_enabled,
            undo_available: frame.undo_stack_len() > 0,
            redo_available: frame.redo_stack_len() > 0,
            page_index,
            page_count,
            click_highlight_enabled: state.click_highlight_enabled(),
            highlight_tool_active: state.highlight_tool_active(),
            any_highlight_active: state.click_highlight_enabled() || state.highlight_tool_active(),
            undo_all_delay_ms: state.undo_all_delay_ms,
            redo_all_delay_ms: state.redo_all_delay_ms,
            custom_section_enabled: state.custom_section_enabled,
            show_delay_sliders: state.show_delay_sliders,
            delay_actions_enabled,
            custom_undo_delay_ms: state.custom_undo_delay_ms,
            custom_redo_delay_ms: state.custom_redo_delay_ms,
            custom_undo_steps: state.custom_undo_steps,
            custom_redo_steps: state.custom_redo_steps,
            top_pinned: state.toolbar_top_pinned,
            side_pinned: state.toolbar_side_pinned,
            use_icons: state.toolbar_use_icons,
            layout_mode: state.toolbar_layout_mode,
            show_more_colors: state.show_more_colors,
            show_actions_section: state.show_actions_section,
            show_actions_advanced,
            show_zoom_actions,
            show_pages_section,
            show_marker_opacity_section: state.show_marker_opacity_section,
            show_preset_toasts: state.show_preset_toasts,
            show_presets: state.show_presets,
            show_step_section,
            show_text_controls: state.show_text_controls,
            show_settings_section,
            show_tool_preview: state.show_tool_preview,
            show_status_bar: state.show_status_bar,
            preset_slot_count: state.preset_slot_count,
            presets,
            active_preset_slot: state.active_preset_slot,
            preset_feedback,
            shape_picker_open: state.toolbar_shapes_expanded,
            drawer_open,
            drawer_tab,
            binding_hints,
        }
    }
}
