use std::path::PathBuf;
use std::time::Instant;

use crate::input::state::PRESET_FEEDBACK_DURATION_MS;
use crate::input::{BoardBackground, InputState};

use super::super::bindings::ToolbarBindingHints;
use super::types::{PresetFeedbackSnapshot, PresetSlotSnapshot, ToolbarSnapshot};

impl ToolbarSnapshot {
    #[allow(dead_code)]
    pub fn from_input(state: &InputState) -> Self {
        Self::from_input_with_bindings(state, ToolbarBindingHints::default())
    }

    pub fn from_input_with_bindings(
        state: &InputState,
        binding_hints: ToolbarBindingHints,
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
            && state.text_editing.mode() == crate::input::TextInputMode::Plain;
        let note_active = matches!(state.state, crate::input::DrawingState::TextInput { .. })
            && state.text_editing.mode() == crate::input::TextInputMode::StickyNote;
        let override_tool = state.tool_override();
        let thickness_targets_eraser = active_tool.uses_eraser_size()
            || override_tool
                .map(|tool| tool.uses_eraser_size())
                .unwrap_or(false);
        let thickness_targets_marker = active_tool.uses_marker_opacity()
            || override_tool
                .map(|tool| tool.uses_marker_opacity())
                .unwrap_or(false);
        let eraser_kind = state.style.eraser_kind;
        let eraser_mode = state.style.eraser_mode;
        let thickness_value = if thickness_targets_eraser {
            state.style.eraser_size
        } else {
            state.thickness_for_tool(active_tool)
        };
        let presets = state
            .preset_slots
            .presets()
            .iter()
            .map(|preset| {
                preset.as_ref().map(|preset| PresetSlotSnapshot {
                    name: preset.name.clone(),
                    tool: preset.tool,
                    color: preset.preview_color(),
                    size: preset.preview_size(),
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
            .preset_slots
            .feedback()
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
        let customize_items_open = state.toolbar_customize_items_open();
        let customize_items_group = state.toolbar_customize_items_group();
        let status_bar_contents_open = state.toolbar_status_bar_contents_open();
        let show_actions_advanced = state.ui_visibility.show_actions_advanced;
        let show_zoom_actions = state.ui_visibility.show_zoom_actions;
        let show_pages_section = state.ui_visibility.show_pages_section;
        let show_boards_section = state.ui_visibility.show_boards_section;
        let show_step_section = state.ui_visibility.show_step_section;
        let delay_actions_enabled =
            state.ui_visibility.show_step_section && state.ui_visibility.show_delay_sliders;

        Self {
            active_tool,
            tool_override: state.tool_override(),
            color: state.color_for_tool(active_tool),
            quick_colors: state.style.quick_colors.clone(),
            thickness: thickness_value,
            eraser_size: state.style.eraser_size,
            thickness_targets_eraser,
            thickness_targets_marker,
            eraser_kind,
            eraser_mode,
            marker_opacity: state.style.marker_opacity,
            pen_smoothing: state.style.pen_smoothing,
            spotlight_magnification: state.style.spotlight_magnification,
            // Filled in by the backend that renders the canvas; see the field.
            spotlight_magnifier_source: None,
            selection_spotlight_magnification: state.selection_spotlight_magnification(),
            font: state.style.font_descriptor.clone(),
            selection_has_text: state.selection_has_text(),
            selected_text_bold: state.first_editable_selected_text_is_bold(),
            font_size: state.style.current_font_size,
            text_active,
            note_active,
            frozen_active: state.frozen_active(),
            zoom_active: state.zoom_active(),
            zoom_locked: state.zoom_locked(),
            fill_enabled: state.style.fill_enabled,
            polygon_sides: state.style.polygon_sides,
            arrow_label_enabled: state.style.arrow_label_enabled,
            arrow_style: state.style.arrow_style,
            arrow_label_next: state.style.arrow_label_counter.max(1),
            step_marker_next: state.style.step_marker_counter.max(1),
            undo_available: frame.undo_stack_len() > 0,
            redo_available: frame.redo_stack_len() > 0,
            board_index,
            board_count,
            board_name,
            board_color,
            page_index,
            page_count,
            click_highlight_enabled: state.click_highlight_enabled(),
            input_hud_enabled: state.input_hud_enabled(),
            highlight_tool_active: state.highlight_tool_active(),
            highlight_tool_ring_enabled: state.highlight_tool_ring_enabled(),
            any_highlight_active: state.click_highlight_enabled() || state.highlight_tool_active(),
            undo_all_delay_ms: state.history_limits.undo_all_delay_ms(),
            redo_all_delay_ms: state.history_limits.redo_all_delay_ms(),
            custom_section_enabled: state.history_limits.custom_section_enabled(),
            show_delay_sliders: state.ui_visibility.show_delay_sliders,
            delay_actions_enabled,
            custom_undo_delay_ms: state.history_limits.custom_undo_delay_ms(),
            custom_redo_delay_ms: state.history_limits.custom_redo_delay_ms(),
            custom_undo_steps: state.history_limits.custom_undo_steps(),
            custom_redo_steps: state.history_limits.custom_redo_steps(),
            top_pinned: state.toolbar_top_pinned(),
            use_icons: state.toolbar_use_icons(),
            toolbar_scale: state.toolbar_scale(),
            layout_mode: state.toolbar_layout_mode(),
            resolved_toolbar_items: state.resolved_toolbar_items().clone(),
            show_more_colors: state.ui_visibility.show_more_colors,
            show_actions_section: state.ui_visibility.show_actions_section,
            show_actions_advanced,
            show_zoom_actions,
            show_pages_section,
            show_boards_section,
            show_marker_opacity_section: state.ui_visibility.show_marker_opacity_section,
            show_preset_toasts: state.ui_visibility.show_preset_toasts,
            idle_fade: state.ui_visibility.idle_fade,
            show_presets: state.ui_visibility.show_presets,
            show_step_section,
            show_text_controls: state.ui_visibility.show_text_controls,
            context_aware_ui: state.ui_visibility.context_aware_ui,
            show_tool_preview: state.ui_visibility.show_tool_preview,
            show_status_bar: state.ui_visibility.show_status_bar,
            status_bar_interactive: state.ui_visibility.status_bar_interactive,
            show_active_output_badge: state.ui_visibility.show_active_output_badge,
            show_status_selection_info: state.ui_visibility.show_status_selection_info,
            show_status_board_badge: state.ui_visibility.show_status_board_badge,
            show_status_page_badge: state.ui_visibility.show_status_page_badge,
            show_status_color: state.ui_visibility.show_status_color,
            show_status_tool: state.ui_visibility.show_status_tool,
            show_status_size: state.ui_visibility.show_status_size,
            show_status_context_indicators: state.ui_visibility.show_status_context_indicators,
            show_toolbar_hint: state.ui_visibility.show_toolbar_hint,
            show_status_help: state.ui_visibility.show_status_help,
            show_status_about: state.ui_visibility.show_status_about,
            show_floating_badge_always: state.ui_visibility.show_floating_badge_always,
            preset_slot_count: state.preset_slots.slot_count(),
            presets,
            active_preset_slot: state.preset_slots.active(),
            preset_feedback,
            shape_picker_open: state.toolbar_top_menu()
                == crate::input::state::TopMenuState::ShapePicker,
            top_overflow_open: state.toolbar_top_menu()
                == crate::input::state::TopMenuState::TopOverflow,
            session_popover_open: state.toolbar_top_menu()
                == crate::input::state::TopMenuState::SessionPopover,
            settings_popover_open: state.toolbar_top_menu()
                == crate::input::state::TopMenuState::SettingsPopover,
            canvas_popover_open: state.toolbar_top_menu()
                == crate::input::state::TopMenuState::CanvasPopover,
            top_popover_scroll: state.toolbar_top_popover_scroll(),
            top_minimized: state.toolbar_top_minimized(),
            top_display_mode: state.toolbar_top_display_mode(),
            // Fade is owned by the backend engine; renderers see 1.0 until
            // the backend publishes the animated value.
            top_fade: 1.0,
            selection_properties: if active_tool == crate::input::Tool::Select {
                state.selection_pill_entries()
            } else {
                Vec::new()
            },
            top_viewport_max: None,
            top_available_height: None,
            customize_items_open,
            customize_items_group,
            status_bar_contents_open,
            binding_hints,
            is_transparent: state.board_is_transparent(),
            render_profile_generation: state.render_profile_generation(),
            active_session_name: None,
            active_session_path: None,
            recent_sessions: Vec::new(),
            pending_save_as_overwrite_path: state.pending_save_as_overwrite().map(PathBuf::from),
            runtime_ui_persistence: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{QuickColorPalette, QuickColorPaletteEntry};
    use crate::draw::Color;
    use crate::input::state::{TopMenuState, test_support::make_test_input_state};

    #[test]
    fn snapshot_carries_input_state_quick_colors() {
        let mut state = make_test_input_state();
        let palette = QuickColorPalette::from_entries(vec![QuickColorPaletteEntry {
            label: "Custom".to_string(),
            color: Color {
                r: 0.20,
                g: 0.40,
                b: 0.60,
                a: 1.0,
            },
        }]);
        state.set_quick_colors(palette.clone());

        let snapshot = ToolbarSnapshot::from_input(&state);

        assert_eq!(snapshot.quick_colors, palette);
    }

    #[test]
    fn snapshot_projects_exactly_one_active_top_menu() {
        let cases = [
            (TopMenuState::Closed, [false; 5]),
            (
                TopMenuState::ShapePicker,
                [true, false, false, false, false],
            ),
            (
                TopMenuState::TopOverflow,
                [false, true, false, false, false],
            ),
            (
                TopMenuState::CanvasPopover,
                [false, false, true, false, false],
            ),
            (
                TopMenuState::SessionPopover,
                [false, false, false, true, false],
            ),
            (
                TopMenuState::SettingsPopover,
                [false, false, false, false, true],
            ),
        ];

        for (top_menu, expected) in cases {
            let mut state = make_test_input_state();
            state.test_set_toolbar_menu_state(top_menu, state.toolbar_top_popover_scroll());

            let snapshot = ToolbarSnapshot::from_input(&state);
            let actual = [
                snapshot.shape_picker_open,
                snapshot.top_overflow_open,
                snapshot.canvas_popover_open,
                snapshot.session_popover_open,
                snapshot.settings_popover_open,
            ];

            assert_eq!(actual, expected, "projecting {top_menu:?}");
        }
    }
}
