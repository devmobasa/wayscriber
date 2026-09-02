use log::warn;
use std::collections::HashMap;

use crate::config::{Action, Config, KeybindingsConfig, QuickColorPalette, Shortcut};
use crate::draw::{FontDescriptor, clamp_regular_sides};
use crate::input::state::InputStateSeed;
use crate::input::{ClickHighlightSettings, DragToolBindings, InputHudSettings, InputState};

impl InputState {
    /// Build runtime input state from validated application configuration.
    pub fn from_config(config: &Config) -> Self {
        let font_descriptor = FontDescriptor::new(
            config.drawing.font_family.clone(),
            config.drawing.font_weight.clone(),
            config.drawing.font_style.clone(),
        );

        let action_map = build_action_map(config);
        let action_bindings = build_action_bindings(config);

        let mut input_state = InputState::from_seed(InputStateSeed {
            color: config.drawing.default_color.to_color(),
            thickness: config.drawing.default_thickness,
            eraser_size: config.drawing.default_eraser_size,
            eraser_mode: config.drawing.default_eraser_mode,
            marker_opacity: config.drawing.marker_opacity,
            fill_enabled: config.drawing.default_fill_enabled,
            font_size: config.drawing.default_font_size,
            font_descriptor,
            text_background_enabled: config.drawing.text_background_enabled,
            arrow_length: config.arrow.length,
            arrow_angle: config.arrow.angle_degrees,
            arrow_head_at_end: config.arrow.head_at_end,
            show_status_bar: config.ui.show_status_bar,
            boards_config: config.resolved_boards(),
            action_map,
            max_shapes_per_frame: config.session.max_shapes_per_frame,
            click_highlight_settings: ClickHighlightSettings::from(&config.ui.click_highlight),
            undo_all_delay_ms: config.history.undo_all_delay_ms,
            redo_all_delay_ms: config.history.redo_all_delay_ms,
            custom_section_enabled: config.history.custom_section_enabled,
            custom_undo_delay_ms: config.history.custom_undo_delay_ms,
            custom_redo_delay_ms: config.history.custom_redo_delay_ms,
            custom_undo_steps: config.history.custom_undo_steps,
            custom_redo_steps: config.history.custom_redo_steps,
            presenter_mode_config: config.presenter_mode.clone(),
        });
        input_state.set_action_bindings(action_bindings);
        input_state.init_input_hud_from_config(InputHudSettings::from(&config.ui.input_hud));
        input_state.set_quick_colors(QuickColorPalette::from_config(&config.drawing.quick_colors));
        input_state.set_drag_tool_bindings(build_drag_tool_bindings(config));
        input_state.set_render_profiles(crate::render_profiles::RenderProfileSet::from_config(
            &config.render_profiles,
        ));

        input_state.set_hit_test_tolerance(config.drawing.hit_test_tolerance);
        input_state.set_hit_test_threshold(config.drawing.hit_test_linear_threshold);
        input_state.set_undo_stack_limit(config.drawing.undo_stack_limit);
        input_state.polygon_sides = clamp_regular_sides(config.drawing.polygon_sides);
        input_state.blur_style = config.drawing.default_blur_style;
        input_state.arrow_style = config.arrow.style;
        input_state.set_pen_smoothing(config.drawing.pen_smoothing);
        input_state.set_font_cycle(config.drawing.font_cycle.clone());
        input_state.spotlight_dim_opacity = config.spotlight.dim_opacity;
        input_state.spotlight_feather = config.spotlight.feather;
        input_state.spotlight_magnification = config.spotlight.magnification;
        input_state.set_context_menu_enabled(config.ui.context_menu.enabled);
        input_state.status_bar_interactive = config.ui.status_bar_interactive;
        input_state.show_status_selection_info = config.ui.show_status_selection_info;
        input_state.show_status_board_badge = config.ui.show_status_board_badge;
        input_state.show_status_page_badge = config.ui.show_status_page_badge;
        input_state.show_status_color = config.ui.show_status_color;
        input_state.show_status_tool = config.ui.show_status_tool;
        input_state.show_status_size = config.ui.show_status_size;
        input_state.show_status_context_indicators = config.ui.show_status_context_indicators;
        input_state.show_toolbar_hint = config.ui.show_toolbar_hint;
        input_state.show_status_help = config.ui.show_status_help;
        input_state.show_status_about = config.ui.show_status_about;
        input_state.show_floating_badge_always = config.ui.show_floating_badge_always;
        input_state.show_floating_badge = config.ui.show_floating_badge;
        input_state.show_active_output_badge = config.ui.active_output_badge;
        input_state.command_palette_toast_duration_ms = config.ui.command_palette_toast_duration_ms;
        input_state.radial_menu_mouse_binding = config.ui.radial_menu_mouse_binding;
        #[cfg(feature = "tablet-input")]
        {
            input_state.pressure_variation_threshold = config.tablet.pressure_variation_threshold;
            input_state.pressure_thickness_edit_mode = config.tablet.pressure_thickness_edit_mode;
            input_state.pressure_thickness_entry_mode = config.tablet.pressure_thickness_entry_mode;
            input_state.pressure_thickness_scale_step = config.tablet.pressure_thickness_scale_step;
        }

        input_state.init_toolbar_from_config(
            config.ui.toolbar.layout_mode,
            config.ui.toolbar.mode_overrides.clone(),
            config.ui.toolbar.items.clone(),
            config.ui.toolbar.top_pinned,
            config.ui.toolbar.use_icons,
            config.ui.toolbar.scale,
            config.ui.toolbar.show_more_colors,
            config.ui.toolbar.show_actions_section,
            config.ui.toolbar.show_actions_advanced,
            config.ui.toolbar.show_zoom_actions,
            config.ui.toolbar.show_pages_section,
            config.ui.toolbar.show_boards_section,
            config.ui.toolbar.show_presets,
            config.ui.toolbar.show_step_section,
            config.ui.toolbar.show_text_controls,
            config.ui.toolbar.context_aware_ui,
            config.ui.toolbar.show_delay_sliders,
            config.ui.toolbar.show_marker_opacity_section,
            config.ui.toolbar.show_preset_toasts,
            config.ui.toolbar.idle_fade,
            config.ui.toolbar.show_tool_preview,
        );
        input_state.init_toolbar_minimized_from_config(config.ui.toolbar.top_minimized);
        input_state.init_toolbar_display_mode_from_config(config.ui.toolbar.top_display_mode);
        input_state.zoom_chip_display = config.ui.toolbar.zoom_chip_display;
        input_state.show_zoom_chip = config.ui.toolbar.show_zoom_chip;
        input_state.init_toolbar_rebind_modifier_from_config(config.ui.toolbar.rebind_modifier);
        input_state.init_presets_from_config(&config.presets);

        input_state
    }
}

fn build_drag_tool_bindings(config: &Config) -> DragToolBindings {
    let drag_tools = config.drawing.effective_drag_tools();
    DragToolBindings::from_config(&drag_tools)
}

fn build_action_map(config: &Config) -> HashMap<Shortcut, Action> {
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

fn build_action_bindings(config: &Config) -> HashMap<Action, Vec<Shortcut>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_action_map_falls_back_when_keybindings_invalid() {
        let mut config = Config::default();
        config.keybindings.core.undo = vec!["Ctrl+Z".to_string()];
        config.keybindings.core.redo = vec!["Ctrl+Z".to_string()];

        let map = build_action_map(&config);
        let default_map = KeybindingsConfig::default()
            .build_action_map()
            .expect("default keybindings should build");

        assert_eq!(map, default_map);
    }

    #[test]
    fn build_action_bindings_fall_back_when_keybindings_invalid() {
        let mut config = Config::default();
        config.keybindings.core.undo = vec!["Ctrl+Z".to_string()];
        config.keybindings.core.redo = vec!["Ctrl+Z".to_string()];

        let bindings = build_action_bindings(&config);
        let default_bindings = KeybindingsConfig::default()
            .build_action_bindings()
            .expect("default keybindings should build");

        assert_eq!(bindings, default_bindings);
    }

    #[test]
    fn build_input_state_applies_selected_ui_flags() {
        let mut config = Config::default();
        config.ui.context_menu.enabled = false;
        config.ui.status_bar_interactive = false;
        config.ui.show_status_selection_info = false;
        config.ui.show_status_board_badge = false;
        config.ui.show_status_page_badge = false;
        config.ui.show_status_color = false;
        config.ui.show_status_tool = false;
        config.ui.show_status_size = false;
        config.ui.show_status_context_indicators = false;
        config.ui.show_toolbar_hint = false;
        config.ui.show_status_help = false;
        config.ui.show_status_about = false;
        config.ui.show_floating_badge_always = true;
        config.ui.show_floating_badge = false;
        config.ui.toolbar.show_zoom_chip = false;
        config.ui.active_output_badge = true;
        config.ui.command_palette_toast_duration_ms = 1234;
        let boards = config.boards.as_mut().expect("boards config");
        boards.pan_enabled = false;
        boards.show_pan_badge = false;

        let input = InputState::from_config(&config);

        assert!(!input.context_menu_enabled());
        assert!(!input.status_bar_interactive);
        assert!(!input.show_status_selection_info);
        assert!(!input.show_status_board_badge);
        assert!(!input.show_status_page_badge);
        assert!(!input.show_status_color);
        assert!(!input.show_status_tool);
        assert!(!input.show_status_size);
        assert!(!input.show_status_context_indicators);
        assert!(!input.show_toolbar_hint);
        assert!(!input.show_status_help);
        assert!(!input.show_status_about);
        assert!(input.show_floating_badge_always);
        assert!(
            !input.show_floating_badge,
            "persisted badge visibility must be restored at startup"
        );
        assert!(
            !input.show_zoom_chip,
            "persisted zoom chip visibility must be restored at startup"
        );
        assert!(input.show_active_output_badge);
        assert_eq!(input.command_palette_toast_duration_ms, 1234);
        assert!(!input.boards.pan_enabled());
        assert!(!input.boards.show_pan_badge());
    }

    #[test]
    fn build_input_state_applies_drag_tool_bindings() {
        let mut config = Config::default();
        config.drawing.drag_tool = crate::input::DragBindableTool::Arrow;
        config.drawing.shift_drag_tool = crate::input::DragBindableTool::Eraser;
        config.drawing.ctrl_drag_tool = crate::input::DragBindableTool::Pen;
        config.drawing.ctrl_shift_drag_tool = crate::input::DragBindableTool::Rect;
        config.drawing.tab_drag_tool = crate::input::DragBindableTool::Ellipse;

        let input = InputState::from_config(&config);
        assert_eq!(
            input.drag_tool_bindings,
            crate::input::DragToolBindings {
                left: crate::input::DragButtonBindings {
                    drag: crate::input::DragBinding::from_tool(crate::input::Tool::Arrow),
                    shift_drag: crate::input::DragBinding::from_tool(crate::input::Tool::Eraser),
                    ctrl_drag: crate::input::DragBinding::from_tool(crate::input::Tool::Pen),
                    ctrl_shift_drag: crate::input::DragBinding::from_tool(crate::input::Tool::Rect,),
                    tab_drag: crate::input::DragBinding::from_tool(crate::input::Tool::Ellipse),
                },
                right: crate::input::DragButtonBindings::button_default(),
                middle: crate::input::DragButtonBindings::button_default(),
            },
        );
    }

    #[test]
    fn build_input_state_applies_mouse_button_drag_tool_bindings() {
        let mut config = Config::default();
        let mut drag_tools = config.drawing.effective_drag_tools();
        drag_tools.left.drag_tool = crate::input::DragTool::Line;
        drag_tools.right.drag_tool = crate::input::DragTool::Pen;
        drag_tools.right.drag_color = Some(crate::config::ColorSpec::Name("blue".to_string()));
        config.drawing.drag_tools = Some(drag_tools);

        let input = InputState::from_config(&config);

        assert_eq!(
            input.drag_tool_bindings.left.drag.tool,
            crate::input::DragTool::Line
        );
        assert_eq!(
            input.drag_tool_bindings.right.drag.tool,
            crate::input::DragTool::Pen
        );
        assert_eq!(
            input.drag_tool_bindings.right.drag.color,
            Some(crate::config::ColorSpec::Name("blue".to_string()).to_color())
        );
    }
}
