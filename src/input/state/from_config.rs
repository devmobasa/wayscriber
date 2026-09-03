use crate::config::Config;
use crate::input::state::{DrawingStyle, HistoryLimits};
use crate::input::{ClickHighlightSettings, InputHudSettings, InputState};

use super::core::{CanvasIndex, InputStateSeed, Keymap};

impl InputState {
    /// Build runtime input state from validated application configuration.
    pub fn from_config(config: &Config) -> Self {
        let style = DrawingStyle::from((&config.drawing, &config.arrow, &config.spotlight));
        let keymap =
            Keymap::from_config(&config.keybindings, &config.drawing.effective_drag_tools());
        let canvas_index = CanvasIndex::from_config(
            config.drawing.hit_test_tolerance,
            config.session.max_shapes_per_frame,
        );

        let mut input_state = InputState::from_seed(InputStateSeed {
            style,
            ui_visibility: crate::input::state::UiVisibility::from(&config.ui),
            boards_config: config.resolved_boards(),
            keymap,
            canvas_index,
            click_highlight_settings: ClickHighlightSettings::from(&config.ui.click_highlight),
            history_limits: HistoryLimits::from(&config.history),
            presenter_mode_config: config.presenter_mode.clone(),
        });
        input_state.init_input_hud_from_config(InputHudSettings::from(&config.ui.input_hud));
        input_state.set_render_profiles(crate::render_profiles::RenderProfileSet::from_config(
            &config.render_profiles,
        ));

        input_state.set_hit_test_threshold(config.drawing.hit_test_linear_threshold);
        input_state.set_undo_stack_limit(config.drawing.undo_stack_limit);
        input_state.set_context_menu_enabled(config.ui.context_menu.enabled);
        input_state
            .set_command_palette_toast_duration_ms(config.ui.command_palette_toast_duration_ms);
        input_state.radial_menu.mouse_binding = config.ui.radial_menu_mouse_binding;
        #[cfg(feature = "tablet-input")]
        {
            input_state.style.pressure_variation_threshold =
                config.tablet.pressure_variation_threshold;
            input_state.style.pressure_thickness_edit_mode =
                config.tablet.pressure_thickness_edit_mode;
            input_state.style.pressure_thickness_entry_mode =
                config.tablet.pressure_thickness_entry_mode;
            input_state.style.pressure_thickness_scale_step =
                config.tablet.pressure_thickness_scale_step;
        }

        input_state.init_toolbar_from_config(&config.ui.toolbar);
        input_state.zoom_chip.display = config.ui.toolbar.zoom_chip_display;
        input_state.init_presets_from_config(&config.presets);

        input_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeybindingsConfig;

    #[test]
    fn build_action_map_falls_back_when_keybindings_invalid() {
        let mut config = Config::default();
        config.keybindings.core.undo = vec!["Ctrl+Z".to_string()];
        config.keybindings.core.redo = vec!["Ctrl+Z".to_string()];

        let map = Keymap::from_config(&config.keybindings, &config.drawing.effective_drag_tools());
        let default_map = KeybindingsConfig::default()
            .build_action_map()
            .expect("default keybindings should build");

        assert_eq!(map.action_map_for_test(), &default_map);
    }

    #[test]
    fn build_action_bindings_fall_back_when_keybindings_invalid() {
        let mut config = Config::default();
        config.keybindings.core.undo = vec!["Ctrl+Z".to_string()];
        config.keybindings.core.redo = vec!["Ctrl+Z".to_string()];

        let keymap =
            Keymap::from_config(&config.keybindings, &config.drawing.effective_drag_tools());
        let default_bindings = KeybindingsConfig::default()
            .build_action_bindings()
            .expect("default keybindings should build");

        assert_eq!(keymap.action_bindings_for_test(), &default_bindings);
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
        assert!(!input.ui_visibility.status_bar_interactive);
        assert!(!input.ui_visibility.show_status_selection_info);
        assert!(!input.ui_visibility.show_status_board_badge);
        assert!(!input.ui_visibility.show_status_page_badge);
        assert!(!input.ui_visibility.show_status_color);
        assert!(!input.ui_visibility.show_status_tool);
        assert!(!input.ui_visibility.show_status_size);
        assert!(!input.ui_visibility.show_status_context_indicators);
        assert!(!input.ui_visibility.show_toolbar_hint);
        assert!(!input.ui_visibility.show_status_help);
        assert!(!input.ui_visibility.show_status_about);
        assert!(input.ui_visibility.show_floating_badge_always);
        assert!(
            !input.ui_visibility.show_floating_badge,
            "persisted badge visibility must be restored at startup"
        );
        assert!(
            !input.ui_visibility.show_zoom_chip,
            "persisted zoom chip visibility must be restored at startup"
        );
        assert!(input.ui_visibility.show_active_output_badge);
        assert_eq!(input.command_palette_toast_duration_ms(), 1234);
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
            input.drag_tool_bindings(),
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
            input.drag_tool_bindings().left.drag.tool,
            crate::input::DragTool::Line
        );
        assert_eq!(
            input.drag_tool_bindings().right.drag.tool,
            crate::input::DragTool::Pen
        );
        assert_eq!(
            input.drag_tool_bindings().right.drag.color,
            Some(crate::config::ColorSpec::Name("blue".to_string()).to_color())
        );
    }
}
