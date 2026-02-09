use std::time::Instant;

use crate::input::state::PRESET_FEEDBACK_DURATION_MS;
use crate::input::{BoardBackground, InputState, Tool};

use super::super::bindings::ToolbarBindingHints;
use super::types::{PresetFeedbackSnapshot, PresetSlotSnapshot, ToolbarSnapshot};

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
