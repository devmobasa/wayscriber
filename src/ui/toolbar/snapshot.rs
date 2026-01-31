use std::time::Instant;

use crate::config::ToolbarLayoutMode;
use crate::draw::{Color, EraserKind, FontDescriptor};
use crate::input::state::{PRESET_FEEDBACK_DURATION_MS, PresetFeedbackKind};
use crate::input::{BoardBackground, EraserMode, InputState, Tool, ToolbarDrawerTab};

use super::bindings::ToolbarBindingHints;

/// The kind of tool-specific options to display in the side panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOptionsKind {
    /// No tool-specific options (e.g., Select tool)
    None,
    /// Pen/Line tools: thickness only
    Stroke,
    /// Marker tool: thickness + opacity
    Marker,
    /// Eraser tool: eraser size + mode toggle
    Eraser,
    /// Shape tools (Rect/Ellipse): thickness + fill toggle
    Shape,
    /// Arrow tool: thickness + labels toggle + counter
    Arrow,
    /// StepMarker tool: size + counter
    StepMarker,
    /// Text mode: font size + font family
    Text,
}

/// Context that determines which UI sections to show based on the active tool.
///
/// This enables contextual/adaptive UI - showing only relevant controls for the
/// current tool rather than all options at once.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolContext {
    /// Whether to show the color section (compact swatch + popover)
    pub needs_color: bool,
    /// Whether to show a thickness/size slider
    pub needs_thickness: bool,
    /// The kind of tool-specific options to display
    pub tool_options_kind: ToolOptionsKind,
    /// Label for the thickness/size slider
    pub thickness_label: &'static str,
    /// Whether the fill toggle should be shown (for shapes)
    pub show_fill_toggle: bool,
    /// Whether the arrow labels section should be shown
    pub show_arrow_labels: bool,
    /// Whether the step marker counter should be shown
    pub show_step_counter: bool,
    /// Whether the eraser mode toggle should be shown
    pub show_eraser_mode: bool,
    /// Whether the marker opacity slider should be shown
    pub show_marker_opacity: bool,
    /// Whether font controls should be shown
    pub show_font_controls: bool,
}

impl ToolContext {
    /// Compute the tool context from the current toolbar state.
    pub fn from_snapshot(snapshot: &ToolbarSnapshot) -> Self {
        // If context-aware UI is disabled, show all sections (classic behavior)
        if !snapshot.context_aware_ui {
            return Self::all_visible(snapshot);
        }

        let effective_tool = snapshot.tool_override.unwrap_or(snapshot.active_tool);
        let text_or_note_active = snapshot.text_active || snapshot.note_active;

        // If text/note mode is active, show text controls
        if text_or_note_active {
            return Self {
                needs_color: true,
                needs_thickness: false,
                tool_options_kind: ToolOptionsKind::Text,
                thickness_label: "",
                show_fill_toggle: false,
                show_arrow_labels: false,
                show_step_counter: false,
                show_eraser_mode: false,
                // Honor show_marker_opacity_section setting
                show_marker_opacity: snapshot.show_marker_opacity_section,
                show_font_controls: true,
            };
        }

        // Compute base context from tool, then apply snapshot settings
        let mut ctx = match effective_tool {
            Tool::Select | Tool::Highlight => Self {
                needs_color: false,
                needs_thickness: false,
                tool_options_kind: ToolOptionsKind::None,
                thickness_label: "",
                show_fill_toggle: false,
                show_arrow_labels: false,
                show_step_counter: false,
                show_eraser_mode: false,
                show_marker_opacity: false,
                show_font_controls: false,
            },
            Tool::Pen | Tool::Line => Self {
                needs_color: true,
                needs_thickness: true,
                tool_options_kind: ToolOptionsKind::Stroke,
                thickness_label: "Thickness",
                show_fill_toggle: false,
                show_arrow_labels: false,
                show_step_counter: false,
                show_eraser_mode: false,
                show_marker_opacity: false,
                show_font_controls: false,
            },
            Tool::Marker => Self {
                needs_color: true,
                needs_thickness: true,
                tool_options_kind: ToolOptionsKind::Marker,
                thickness_label: "Thickness",
                show_fill_toggle: false,
                show_arrow_labels: false,
                show_step_counter: false,
                show_eraser_mode: false,
                show_marker_opacity: true,
                show_font_controls: false,
            },
            Tool::Eraser => Self {
                needs_color: false,
                needs_thickness: true,
                tool_options_kind: ToolOptionsKind::Eraser,
                thickness_label: "Eraser Size",
                show_fill_toggle: false,
                show_arrow_labels: false,
                show_step_counter: false,
                show_eraser_mode: true,
                show_marker_opacity: false,
                show_font_controls: false,
            },
            Tool::Rect | Tool::Ellipse => Self {
                needs_color: true,
                needs_thickness: true,
                tool_options_kind: ToolOptionsKind::Shape,
                thickness_label: "Thickness",
                show_fill_toggle: true,
                show_arrow_labels: false,
                show_step_counter: false,
                show_eraser_mode: false,
                show_marker_opacity: false,
                show_font_controls: false,
            },
            Tool::Arrow => Self {
                needs_color: true,
                needs_thickness: true,
                tool_options_kind: ToolOptionsKind::Arrow,
                thickness_label: "Thickness",
                show_fill_toggle: false,
                show_arrow_labels: true,
                show_step_counter: false,
                show_eraser_mode: false,
                show_marker_opacity: false,
                show_font_controls: false,
            },
            Tool::StepMarker => Self {
                needs_color: true,
                needs_thickness: true,
                tool_options_kind: ToolOptionsKind::StepMarker,
                thickness_label: "Size",
                show_fill_toggle: false,
                show_arrow_labels: false,
                show_step_counter: true,
                show_eraser_mode: false,
                show_marker_opacity: false,
                show_font_controls: false,
            },
        };

        // Honor snapshot settings that override tool-based visibility
        // show_text_controls: keep font controls visible even when text mode is inactive
        if snapshot.show_text_controls {
            ctx.show_font_controls = true;
        }
        // show_marker_opacity_section: keep opacity slider visible for all tools
        if snapshot.show_marker_opacity_section {
            ctx.show_marker_opacity = true;
        }

        ctx
    }

    /// Returns a context where all sections are visible (classic/non-contextual behavior).
    fn all_visible(snapshot: &ToolbarSnapshot) -> Self {
        let effective_tool = snapshot.tool_override.unwrap_or(snapshot.active_tool);
        let text_or_note_active = snapshot.text_active || snapshot.note_active;
        let show_arrow_labels = effective_tool == Tool::Arrow || snapshot.arrow_label_enabled;
        let show_step_counter = effective_tool == Tool::StepMarker;
        let show_marker_opacity =
            snapshot.show_marker_opacity_section || snapshot.thickness_targets_marker;
        let show_font_controls = text_or_note_active || snapshot.show_text_controls;

        Self {
            needs_color: true,
            needs_thickness: true,
            tool_options_kind: ToolOptionsKind::Stroke, // Generic
            thickness_label: if snapshot.thickness_targets_eraser {
                "Eraser size"
            } else {
                "Thickness"
            },
            show_fill_toggle: false, // Only shown contextually for shapes
            show_arrow_labels,
            show_step_counter,
            show_eraser_mode: snapshot.thickness_targets_eraser,
            show_marker_opacity,
            show_font_controls,
        }
    }
}

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
    pub arrow_label_next: u32,
    pub step_marker_next: u32,
    pub undo_available: bool,
    pub redo_available: bool,
    pub board_index: usize,
    pub board_count: usize,
    pub board_name: String,
    pub board_color: Option<Color>,
    pub page_index: usize,
    pub page_count: usize,
    pub click_highlight_enabled: bool,
    pub highlight_tool_active: bool,
    pub highlight_tool_ring_enabled: bool,
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
    /// Scale factor for toolbar UI (icons + layout)
    pub toolbar_scale: f64,
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
    /// Whether to show the Boards section
    pub show_boards_section: bool,
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
    /// Whether to enable context-aware UI that shows/hides controls based on active tool
    pub context_aware_ui: bool,
    /// Whether to show the Settings section
    pub show_settings_section: bool,
    pub show_tool_preview: bool,
    pub show_status_bar: bool,
    pub show_status_board_badge: bool,
    pub show_status_page_badge: bool,
    pub show_floating_badge_always: bool,
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
    /// Whether to show the drawer onboarding hint (first-time users)
    pub show_drawer_hint: bool,
    /// Whether the current board is the transparent overlay
    pub is_transparent: bool,
}

impl ToolbarSnapshot {
    #[allow(dead_code)]
    pub fn from_input(state: &InputState) -> Self {
        Self::from_input_with_options(state, ToolbarBindingHints::default(), false)
    }

    #[allow(dead_code)]
    pub fn from_input_with_bindings(
        state: &InputState,
        binding_hints: ToolbarBindingHints,
    ) -> Self {
        Self::from_input_with_options(state, binding_hints, false)
    }

    pub fn from_input_with_options(
        state: &InputState,
        binding_hints: ToolbarBindingHints,
        show_drawer_hint: bool,
    ) -> Self {
        let frame = state.boards.active_frame();
        let active_tool = state.active_tool();
        let board_count = state.boards.board_count();
        let board_index = state.boards.active_index();
        let board_name = state.board_name().to_string();
        let board_color = match state.boards.active_background() {
            BoardBackground::Solid(color) => Some(*color),
            BoardBackground::Transparent => None,
        };
        let page_count = state.boards.page_count();
        let page_index = state.boards.active_page_index();
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
        let show_boards_section = state.show_boards_section;
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
            arrow_label_next: state.arrow_label_counter.max(1),
            step_marker_next: state.step_marker_counter.max(1),
            undo_available: frame.undo_stack_len() > 0,
            redo_available: frame.redo_stack_len() > 0,
            board_index,
            board_count,
            board_name,
            board_color,
            page_index,
            page_count,
            click_highlight_enabled: state.click_highlight_enabled(),
            highlight_tool_active: state.highlight_tool_active(),
            highlight_tool_ring_enabled: state.highlight_tool_ring_enabled(),
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
            toolbar_scale: state.toolbar_scale,
            layout_mode: state.toolbar_layout_mode,
            show_more_colors: state.show_more_colors,
            show_actions_section: state.show_actions_section,
            show_actions_advanced,
            show_zoom_actions,
            show_pages_section,
            show_boards_section,
            show_marker_opacity_section: state.show_marker_opacity_section,
            show_preset_toasts: state.show_preset_toasts,
            show_presets: state.show_presets,
            show_step_section,
            show_text_controls: state.show_text_controls,
            context_aware_ui: state.context_aware_ui,
            show_settings_section,
            show_tool_preview: state.show_tool_preview,
            show_status_bar: state.show_status_bar,
            show_status_board_badge: state.show_status_board_badge,
            show_status_page_badge: state.show_status_page_badge,
            show_floating_badge_always: state.show_floating_badge_always,
            preset_slot_count: state.preset_slot_count,
            presets,
            active_preset_slot: state.active_preset_slot,
            preset_feedback,
            shape_picker_open: state.toolbar_shapes_expanded,
            drawer_open,
            drawer_tab,
            binding_hints,
            show_drawer_hint,
            is_transparent: state.board_is_transparent(),
        }
    }
}
