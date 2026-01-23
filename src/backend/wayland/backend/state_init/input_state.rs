use log::warn;
use std::collections::HashMap;

use crate::config::{Action, Config, KeyBinding, KeybindingsConfig};
use crate::draw::FontDescriptor;
use crate::input::{ClickHighlightSettings, InputState};

pub(super) fn build_input_state(config: &Config) -> InputState {
    let font_descriptor = FontDescriptor::new(
        config.drawing.font_family.clone(),
        config.drawing.font_weight.clone(),
        config.drawing.font_style.clone(),
    );

    let action_map = build_action_map(config);
    let action_bindings = build_action_bindings(config);

    let mut input_state = InputState::with_defaults(
        config.drawing.default_color.to_color(),
        config.drawing.default_thickness,
        config.drawing.default_eraser_size,
        config.drawing.default_eraser_mode,
        config.drawing.marker_opacity,
        config.drawing.default_fill_enabled,
        config.drawing.default_font_size,
        font_descriptor,
        config.drawing.text_background_enabled,
        config.arrow.length,
        config.arrow.angle_degrees,
        config.arrow.head_at_end,
        config.ui.show_status_bar,
        config.resolved_boards(),
        action_map,
        config.session.max_shapes_per_frame,
        ClickHighlightSettings::from(&config.ui.click_highlight),
        config.history.undo_all_delay_ms,
        config.history.redo_all_delay_ms,
        config.history.custom_section_enabled,
        config.history.custom_undo_delay_ms,
        config.history.custom_redo_delay_ms,
        config.history.custom_undo_steps,
        config.history.custom_redo_steps,
        config.presenter_mode.clone(),
    );
    input_state.set_action_bindings(action_bindings);

    input_state.set_hit_test_tolerance(config.drawing.hit_test_tolerance);
    input_state.set_hit_test_threshold(config.drawing.hit_test_linear_threshold);
    input_state.set_undo_stack_limit(config.drawing.undo_stack_limit);
    input_state.set_context_menu_enabled(config.ui.context_menu.enabled);
    input_state.show_status_board_badge = config.ui.show_status_board_badge;
    input_state.show_status_page_badge = config.ui.show_status_page_badge;
    input_state.show_floating_badge_always = config.ui.show_floating_badge_always;
    #[cfg(tablet)]
    {
        input_state.pressure_variation_threshold = config.tablet.pressure_variation_threshold;
        input_state.pressure_thickness_edit_mode = config.tablet.pressure_thickness_edit_mode;
        input_state.pressure_thickness_entry_mode = config.tablet.pressure_thickness_entry_mode;
        input_state.pressure_thickness_scale_step = config.tablet.pressure_thickness_scale_step;
    }

    input_state.init_toolbar_from_config(
        config.ui.toolbar.layout_mode,
        config.ui.toolbar.mode_overrides.clone(),
        config.ui.toolbar.top_pinned,
        config.ui.toolbar.side_pinned,
        config.ui.toolbar.use_icons,
        config.ui.toolbar.show_more_colors,
        config.ui.toolbar.show_actions_section,
        config.ui.toolbar.show_actions_advanced,
        config.ui.toolbar.show_zoom_actions,
        config.ui.toolbar.show_pages_section,
        config.ui.toolbar.show_boards_section,
        config.ui.toolbar.show_presets,
        config.ui.toolbar.show_step_section,
        config.ui.toolbar.show_text_controls,
        config.ui.toolbar.show_settings_section,
        config.ui.toolbar.show_delay_sliders,
        config.ui.toolbar.show_marker_opacity_section,
        config.ui.toolbar.show_preset_toasts,
        config.ui.toolbar.show_tool_preview,
    );
    input_state.init_presets_from_config(&config.presets);

    input_state
}

fn build_action_map(config: &Config) -> HashMap<KeyBinding, Action> {
    match config.keybindings.build_action_map() {
        Ok(map) => map,
        Err(err) => {
            warn!(
                "Invalid keybindings config: {}. Falling back to defaults.",
                err
            );
            KeybindingsConfig::default()
                .build_action_map()
                .unwrap_or_else(|err| {
                    warn!(
                        "Failed to build default keybindings: {}. Continuing with no bindings.",
                        err
                    );
                    HashMap::new()
                })
        }
    }
}

fn build_action_bindings(config: &Config) -> HashMap<Action, Vec<KeyBinding>> {
    match config.keybindings.build_action_bindings() {
        Ok(map) => map,
        Err(err) => {
            warn!(
                "Invalid keybindings config: {}. Falling back to defaults.",
                err
            );
            KeybindingsConfig::default()
                .build_action_bindings()
                .unwrap_or_else(|err| {
                    warn!(
                        "Failed to build default keybindings: {}. Continuing with no bindings.",
                        err
                    );
                    HashMap::new()
                })
        }
    }
}
