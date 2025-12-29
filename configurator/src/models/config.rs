use wayscriber::config::{
    Config, PRESET_SLOTS_MAX, PRESET_SLOTS_MIN, PresetSlotsConfig, ToolPresetConfig,
    ToolbarModeOverride, ToolbarModeOverrides,
};
use wayscriber::input::Tool;

use super::color::{ColorInput, ColorQuadInput, ColorTripletInput};
use super::error::FormError;
use super::fields::{
    BoardModeOption, EraserModeOption, FontStyleOption, FontWeightOption, OverrideOption,
    PresetEraserKindOption, PresetEraserModeOption, QuadField, SessionCompressionOption,
    SessionStorageModeOption, StatusPositionOption, TextField, ToggleField, ToolOption,
    ToolbarLayoutModeOption, ToolbarOverrideField, TripletField,
};
use super::keybindings::KeybindingsDraft;
use super::util::{format_float, parse_f64};

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigDraft {
    pub drawing_color: ColorInput,
    pub drawing_default_thickness: String,
    pub drawing_default_eraser_size: String,
    pub drawing_default_eraser_mode: EraserModeOption,
    pub drawing_default_font_size: String,
    pub drawing_marker_opacity: String,
    pub drawing_hit_test_tolerance: String,
    pub drawing_hit_test_linear_threshold: String,
    pub drawing_undo_stack_limit: String,
    pub drawing_font_family: String,
    pub drawing_font_weight: String,
    pub drawing_font_style: String,
    pub drawing_text_background_enabled: bool,
    pub drawing_default_fill_enabled: bool,
    pub drawing_font_style_option: FontStyleOption,
    pub drawing_font_weight_option: FontWeightOption,

    pub arrow_length: String,
    pub arrow_angle: String,
    pub arrow_head_at_end: bool,

    pub history_undo_all_delay_ms: String,
    pub history_redo_all_delay_ms: String,
    pub history_custom_section_enabled: bool,
    pub history_custom_undo_delay_ms: String,
    pub history_custom_redo_delay_ms: String,
    pub history_custom_undo_steps: String,
    pub history_custom_redo_steps: String,

    pub performance_buffer_count: u32,
    pub performance_enable_vsync: bool,
    pub performance_ui_animation_fps: String,

    pub ui_show_status_bar: bool,
    pub ui_show_frozen_badge: bool,
    pub ui_context_menu_enabled: bool,
    pub ui_preferred_output: String,
    pub ui_xdg_fullscreen: bool,
    pub ui_toolbar_top_pinned: bool,
    pub ui_toolbar_side_pinned: bool,
    pub ui_toolbar_use_icons: bool,
    pub ui_toolbar_show_more_colors: bool,
    pub ui_toolbar_show_preset_toasts: bool,
    pub ui_toolbar_layout_mode: ToolbarLayoutModeOption,
    pub ui_toolbar_show_presets: bool,
    pub ui_toolbar_show_actions_section: bool,
    pub ui_toolbar_show_actions_advanced: bool,
    pub ui_toolbar_show_step_section: bool,
    pub ui_toolbar_show_text_controls: bool,
    pub ui_toolbar_show_settings_section: bool,
    pub ui_toolbar_show_delay_sliders: bool,
    pub ui_toolbar_show_marker_opacity_section: bool,
    pub ui_toolbar_show_tool_preview: bool,
    pub ui_toolbar_force_inline: bool,
    pub ui_toolbar_top_offset: String,
    pub ui_toolbar_top_offset_y: String,
    pub ui_toolbar_side_offset: String,
    pub ui_toolbar_side_offset_x: String,
    pub ui_toolbar_mode_overrides: ToolbarModeOverridesDraft,
    pub ui_status_position: StatusPositionOption,
    pub status_font_size: String,
    pub status_padding: String,
    pub status_bar_bg_color: ColorQuadInput,
    pub status_bar_text_color: ColorQuadInput,
    pub status_dot_radius: String,

    pub click_highlight_enabled: bool,
    pub click_highlight_use_pen_color: bool,
    pub click_highlight_radius: String,
    pub click_highlight_outline_thickness: String,
    pub click_highlight_duration_ms: String,
    pub click_highlight_fill_color: ColorQuadInput,
    pub click_highlight_outline_color: ColorQuadInput,

    pub help_font_size: String,
    pub help_line_height: String,
    pub help_padding: String,
    pub help_bg_color: ColorQuadInput,
    pub help_border_color: ColorQuadInput,
    pub help_border_width: String,
    pub help_text_color: ColorQuadInput,

    pub board_enabled: bool,
    pub board_default_mode: BoardModeOption,
    pub board_whiteboard_color: ColorTripletInput,
    pub board_blackboard_color: ColorTripletInput,
    pub board_whiteboard_pen: ColorTripletInput,
    pub board_blackboard_pen: ColorTripletInput,
    pub board_auto_adjust_pen: bool,

    pub capture_enabled: bool,
    pub capture_save_directory: String,
    pub capture_filename_template: String,
    pub capture_format: String,
    pub capture_copy_to_clipboard: bool,
    pub capture_exit_after: bool,

    pub session_persist_transparent: bool,
    pub session_persist_whiteboard: bool,
    pub session_persist_blackboard: bool,
    pub session_persist_history: bool,
    pub session_restore_tool_state: bool,
    pub session_per_output: bool,
    pub session_storage_mode: SessionStorageModeOption,
    pub session_custom_directory: String,
    pub session_max_shapes_per_frame: String,
    pub session_max_file_size_mb: String,
    pub session_compression: SessionCompressionOption,
    pub session_auto_compress_threshold_kb: String,
    pub session_max_persisted_undo_depth: String,
    pub session_backup_retention: String,

    #[cfg(feature = "tablet-input")]
    pub tablet_enabled: bool,
    #[cfg(feature = "tablet-input")]
    pub tablet_pressure_enabled: bool,
    #[cfg(feature = "tablet-input")]
    pub tablet_min_thickness: String,
    #[cfg(feature = "tablet-input")]
    pub tablet_max_thickness: String,

    pub presets: PresetsDraft,

    pub keybindings: KeybindingsDraft,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresetSlotDraft {
    pub enabled: bool,
    pub name: String,
    pub tool: ToolOption,
    pub color: ColorInput,
    pub size: String,
    pub eraser_kind: PresetEraserKindOption,
    pub eraser_mode: PresetEraserModeOption,
    pub marker_opacity: String,
    pub fill_enabled: OverrideOption,
    pub font_size: String,
    pub text_background_enabled: OverrideOption,
    pub arrow_length: String,
    pub arrow_angle: String,
    pub arrow_head_at_end: OverrideOption,
    pub show_status_bar: OverrideOption,
}

impl PresetSlotDraft {
    fn from_config(preset: Option<&ToolPresetConfig>, defaults: &Config) -> Self {
        match preset {
            Some(preset) => Self {
                enabled: true,
                name: preset.name.clone().unwrap_or_default(),
                tool: ToolOption::from_tool(preset.tool),
                color: ColorInput::from_color(&preset.color),
                size: format_float(preset.size),
                eraser_kind: PresetEraserKindOption::from_option(preset.eraser_kind),
                eraser_mode: PresetEraserModeOption::from_option(preset.eraser_mode),
                marker_opacity: preset.marker_opacity.map(format_float).unwrap_or_default(),
                fill_enabled: OverrideOption::from_option(preset.fill_enabled),
                font_size: preset.font_size.map(format_float).unwrap_or_default(),
                text_background_enabled: OverrideOption::from_option(
                    preset.text_background_enabled,
                ),
                arrow_length: preset.arrow_length.map(format_float).unwrap_or_default(),
                arrow_angle: preset.arrow_angle.map(format_float).unwrap_or_default(),
                arrow_head_at_end: OverrideOption::from_option(preset.arrow_head_at_end),
                show_status_bar: OverrideOption::from_option(preset.show_status_bar),
            },
            None => {
                let mut slot = Self::default_from_config(defaults);
                slot.enabled = false;
                slot
            }
        }
    }

    fn default_from_config(defaults: &Config) -> Self {
        Self {
            enabled: true,
            name: String::new(),
            tool: ToolOption::from_tool(Tool::Pen),
            color: ColorInput::from_color(&defaults.drawing.default_color),
            size: format_float(defaults.drawing.default_thickness),
            eraser_kind: PresetEraserKindOption::Default,
            eraser_mode: PresetEraserModeOption::Default,
            marker_opacity: String::new(),
            fill_enabled: OverrideOption::Default,
            font_size: String::new(),
            text_background_enabled: OverrideOption::Default,
            arrow_length: String::new(),
            arrow_angle: String::new(),
            arrow_head_at_end: OverrideOption::Default,
            show_status_bar: OverrideOption::Default,
        }
    }

    fn to_config(
        &self,
        slot_index: usize,
        errors: &mut Vec<FormError>,
    ) -> Option<ToolPresetConfig> {
        if !self.enabled {
            return None;
        }

        let name = self.name.trim();
        let name = if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        };

        let color = match self
            .color
            .to_color_spec_with_field(&format!("presets.slot_{slot_index}.color"))
        {
            Ok(color) => Some(color),
            Err(err) => {
                errors.push(err);
                None
            }
        };

        let size = parse_required_f64(
            &self.size,
            format!("presets.slot_{slot_index}.size"),
            errors,
        );

        let marker_opacity = parse_optional_f64(
            &self.marker_opacity,
            format!("presets.slot_{slot_index}.marker_opacity"),
            errors,
        );
        let font_size = parse_optional_f64(
            &self.font_size,
            format!("presets.slot_{slot_index}.font_size"),
            errors,
        );
        let arrow_length = parse_optional_f64(
            &self.arrow_length,
            format!("presets.slot_{slot_index}.arrow_length"),
            errors,
        );
        let arrow_angle = parse_optional_f64(
            &self.arrow_angle,
            format!("presets.slot_{slot_index}.arrow_angle"),
            errors,
        );

        let color = color?;
        let size = size?;

        Some(ToolPresetConfig {
            name,
            tool: self.tool.to_tool(),
            color,
            size,
            eraser_kind: self.eraser_kind.to_option(),
            eraser_mode: self.eraser_mode.to_option(),
            marker_opacity,
            fill_enabled: self.fill_enabled.to_option(),
            font_size,
            text_background_enabled: self.text_background_enabled.to_option(),
            arrow_length,
            arrow_angle,
            arrow_head_at_end: self.arrow_head_at_end.to_option(),
            show_status_bar: self.show_status_bar.to_option(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresetsDraft {
    pub slot_count: usize,
    slots: Vec<PresetSlotDraft>,
}

impl PresetsDraft {
    pub fn from_config(config: &Config) -> Self {
        let slots = (1..=PRESET_SLOTS_MAX)
            .map(|slot| PresetSlotDraft::from_config(config.presets.get_slot(slot), config))
            .collect();
        Self {
            slot_count: config.presets.slot_count,
            slots,
        }
    }

    pub fn to_config(&self, errors: &mut Vec<FormError>) -> PresetSlotsConfig {
        let mut config = PresetSlotsConfig::default();

        if !(PRESET_SLOTS_MIN..=PRESET_SLOTS_MAX).contains(&self.slot_count) {
            errors.push(FormError::new(
                "presets.slot_count",
                format!("Expected a value between {PRESET_SLOTS_MIN} and {PRESET_SLOTS_MAX}"),
            ));
        }
        config.slot_count = self.slot_count.clamp(PRESET_SLOTS_MIN, PRESET_SLOTS_MAX);

        for (index, slot) in self.slots.iter().enumerate() {
            let slot_index = index + 1;
            config.set_slot(slot_index, slot.to_config(slot_index, errors));
        }

        config
    }

    pub fn slot(&self, slot: usize) -> Option<&PresetSlotDraft> {
        if slot == 0 {
            return None;
        }
        self.slots.get(slot - 1)
    }

    pub fn slot_mut(&mut self, slot: usize) -> Option<&mut PresetSlotDraft> {
        if slot == 0 {
            return None;
        }
        self.slots.get_mut(slot - 1)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarModeOverrideDraft {
    pub show_presets: OverrideOption,
    pub show_actions_section: OverrideOption,
    pub show_actions_advanced: OverrideOption,
    pub show_step_section: OverrideOption,
    pub show_text_controls: OverrideOption,
    pub show_settings_section: OverrideOption,
}

impl ToolbarModeOverrideDraft {
    fn from_override(override_cfg: &ToolbarModeOverride) -> Self {
        Self {
            show_presets: OverrideOption::from_option(override_cfg.show_presets),
            show_actions_section: OverrideOption::from_option(override_cfg.show_actions_section),
            show_actions_advanced: OverrideOption::from_option(override_cfg.show_actions_advanced),
            show_step_section: OverrideOption::from_option(override_cfg.show_step_section),
            show_text_controls: OverrideOption::from_option(override_cfg.show_text_controls),
            show_settings_section: OverrideOption::from_option(override_cfg.show_settings_section),
        }
    }

    fn to_override(&self) -> ToolbarModeOverride {
        ToolbarModeOverride {
            show_actions_section: self.show_actions_section.to_option(),
            show_actions_advanced: self.show_actions_advanced.to_option(),
            show_presets: self.show_presets.to_option(),
            show_step_section: self.show_step_section.to_option(),
            show_text_controls: self.show_text_controls.to_option(),
            show_settings_section: self.show_settings_section.to_option(),
        }
    }

    fn set(&mut self, field: ToolbarOverrideField, value: OverrideOption) {
        match field {
            ToolbarOverrideField::ShowPresets => self.show_presets = value,
            ToolbarOverrideField::ShowActionsSection => self.show_actions_section = value,
            ToolbarOverrideField::ShowActionsAdvanced => self.show_actions_advanced = value,
            ToolbarOverrideField::ShowStepSection => self.show_step_section = value,
            ToolbarOverrideField::ShowTextControls => self.show_text_controls = value,
            ToolbarOverrideField::ShowSettingsSection => self.show_settings_section = value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarModeOverridesDraft {
    pub simple: ToolbarModeOverrideDraft,
    pub regular: ToolbarModeOverrideDraft,
    pub advanced: ToolbarModeOverrideDraft,
}

impl ToolbarModeOverridesDraft {
    fn from_config(config: &ToolbarModeOverrides) -> Self {
        Self {
            simple: ToolbarModeOverrideDraft::from_override(&config.simple),
            regular: ToolbarModeOverrideDraft::from_override(&config.regular),
            advanced: ToolbarModeOverrideDraft::from_override(&config.advanced),
        }
    }

    fn to_config(&self) -> ToolbarModeOverrides {
        ToolbarModeOverrides {
            simple: self.simple.to_override(),
            regular: self.regular.to_override(),
            advanced: self.advanced.to_override(),
        }
    }

    pub fn for_mode(&self, mode: ToolbarLayoutModeOption) -> &ToolbarModeOverrideDraft {
        match mode {
            ToolbarLayoutModeOption::Simple => &self.simple,
            ToolbarLayoutModeOption::Regular => &self.regular,
            ToolbarLayoutModeOption::Advanced => &self.advanced,
        }
    }

    fn for_mode_mut(&mut self, mode: ToolbarLayoutModeOption) -> &mut ToolbarModeOverrideDraft {
        match mode {
            ToolbarLayoutModeOption::Simple => &mut self.simple,
            ToolbarLayoutModeOption::Regular => &mut self.regular,
            ToolbarLayoutModeOption::Advanced => &mut self.advanced,
        }
    }
}

impl ConfigDraft {
    pub fn from_config(config: &Config) -> Self {
        let (style_option, style_value) = FontStyleOption::from_value(&config.drawing.font_style);
        let (weight_option, weight_value) =
            FontWeightOption::from_value(&config.drawing.font_weight);
        Self {
            drawing_color: ColorInput::from_color(&config.drawing.default_color),
            drawing_default_thickness: format_float(config.drawing.default_thickness),
            drawing_default_eraser_size: format_float(config.drawing.default_eraser_size),
            drawing_default_eraser_mode: EraserModeOption::from_mode(
                config.drawing.default_eraser_mode,
            ),
            drawing_default_font_size: format_float(config.drawing.default_font_size),
            drawing_marker_opacity: format_float(config.drawing.marker_opacity),
            drawing_hit_test_tolerance: format_float(config.drawing.hit_test_tolerance),
            drawing_hit_test_linear_threshold: config.drawing.hit_test_linear_threshold.to_string(),
            drawing_undo_stack_limit: config.drawing.undo_stack_limit.to_string(),
            drawing_font_family: config.drawing.font_family.clone(),
            drawing_font_weight: weight_value,
            drawing_font_style: style_value,
            drawing_text_background_enabled: config.drawing.text_background_enabled,
            drawing_default_fill_enabled: config.drawing.default_fill_enabled,
            drawing_font_style_option: style_option,
            drawing_font_weight_option: weight_option,

            arrow_length: format_float(config.arrow.length),
            arrow_angle: format_float(config.arrow.angle_degrees),
            arrow_head_at_end: config.arrow.head_at_end,

            history_undo_all_delay_ms: config.history.undo_all_delay_ms.to_string(),
            history_redo_all_delay_ms: config.history.redo_all_delay_ms.to_string(),
            history_custom_section_enabled: config.history.custom_section_enabled,
            history_custom_undo_delay_ms: config.history.custom_undo_delay_ms.to_string(),
            history_custom_redo_delay_ms: config.history.custom_redo_delay_ms.to_string(),
            history_custom_undo_steps: config.history.custom_undo_steps.to_string(),
            history_custom_redo_steps: config.history.custom_redo_steps.to_string(),

            performance_buffer_count: config.performance.buffer_count,
            performance_enable_vsync: config.performance.enable_vsync,
            performance_ui_animation_fps: config.performance.ui_animation_fps.to_string(),

            ui_show_status_bar: config.ui.show_status_bar,
            ui_show_frozen_badge: config.ui.show_frozen_badge,
            ui_context_menu_enabled: config.ui.context_menu.enabled,
            ui_preferred_output: config.ui.preferred_output.clone().unwrap_or_default(),
            ui_xdg_fullscreen: config.ui.xdg_fullscreen,
            ui_toolbar_top_pinned: config.ui.toolbar.top_pinned,
            ui_toolbar_side_pinned: config.ui.toolbar.side_pinned,
            ui_toolbar_use_icons: config.ui.toolbar.use_icons,
            ui_toolbar_show_more_colors: config.ui.toolbar.show_more_colors,
            ui_toolbar_show_preset_toasts: config.ui.toolbar.show_preset_toasts,
            ui_toolbar_layout_mode: ToolbarLayoutModeOption::from_mode(
                config.ui.toolbar.layout_mode,
            ),
            ui_toolbar_show_presets: config.ui.toolbar.show_presets,
            ui_toolbar_show_actions_section: config.ui.toolbar.show_actions_section,
            ui_toolbar_show_actions_advanced: config.ui.toolbar.show_actions_advanced,
            ui_toolbar_show_step_section: config.ui.toolbar.show_step_section,
            ui_toolbar_show_text_controls: config.ui.toolbar.show_text_controls,
            ui_toolbar_show_settings_section: config.ui.toolbar.show_settings_section,
            ui_toolbar_show_delay_sliders: config.ui.toolbar.show_delay_sliders,
            ui_toolbar_show_marker_opacity_section: config.ui.toolbar.show_marker_opacity_section,
            ui_toolbar_show_tool_preview: config.ui.toolbar.show_tool_preview,
            ui_toolbar_force_inline: config.ui.toolbar.force_inline,
            ui_toolbar_top_offset: format_float(config.ui.toolbar.top_offset),
            ui_toolbar_top_offset_y: format_float(config.ui.toolbar.top_offset_y),
            ui_toolbar_side_offset: format_float(config.ui.toolbar.side_offset),
            ui_toolbar_side_offset_x: format_float(config.ui.toolbar.side_offset_x),
            ui_toolbar_mode_overrides: ToolbarModeOverridesDraft::from_config(
                &config.ui.toolbar.mode_overrides,
            ),
            ui_status_position: StatusPositionOption::from_status_position(
                config.ui.status_bar_position,
            ),
            status_font_size: format_float(config.ui.status_bar_style.font_size),
            status_padding: format_float(config.ui.status_bar_style.padding),
            status_bar_bg_color: ColorQuadInput::from(config.ui.status_bar_style.bg_color),
            status_bar_text_color: ColorQuadInput::from(config.ui.status_bar_style.text_color),
            status_dot_radius: format_float(config.ui.status_bar_style.dot_radius),

            click_highlight_enabled: config.ui.click_highlight.enabled,
            click_highlight_use_pen_color: config.ui.click_highlight.use_pen_color,
            click_highlight_radius: format_float(config.ui.click_highlight.radius),
            click_highlight_outline_thickness: format_float(
                config.ui.click_highlight.outline_thickness,
            ),
            click_highlight_duration_ms: config.ui.click_highlight.duration_ms.to_string(),
            click_highlight_fill_color: ColorQuadInput::from(config.ui.click_highlight.fill_color),
            click_highlight_outline_color: ColorQuadInput::from(
                config.ui.click_highlight.outline_color,
            ),

            help_font_size: format_float(config.ui.help_overlay_style.font_size),
            help_line_height: format_float(config.ui.help_overlay_style.line_height),
            help_padding: format_float(config.ui.help_overlay_style.padding),
            help_bg_color: ColorQuadInput::from(config.ui.help_overlay_style.bg_color),
            help_border_color: ColorQuadInput::from(config.ui.help_overlay_style.border_color),
            help_border_width: format_float(config.ui.help_overlay_style.border_width),
            help_text_color: ColorQuadInput::from(config.ui.help_overlay_style.text_color),

            board_enabled: config.board.enabled,
            board_default_mode: BoardModeOption::from_str(&config.board.default_mode)
                .unwrap_or(BoardModeOption::Transparent),
            board_whiteboard_color: ColorTripletInput::from(config.board.whiteboard_color),
            board_blackboard_color: ColorTripletInput::from(config.board.blackboard_color),
            board_whiteboard_pen: ColorTripletInput::from(config.board.whiteboard_pen_color),
            board_blackboard_pen: ColorTripletInput::from(config.board.blackboard_pen_color),
            board_auto_adjust_pen: config.board.auto_adjust_pen,

            capture_enabled: config.capture.enabled,
            capture_save_directory: config.capture.save_directory.clone(),
            capture_filename_template: config.capture.filename_template.clone(),
            capture_format: config.capture.format.clone(),
            capture_copy_to_clipboard: config.capture.copy_to_clipboard,
            capture_exit_after: config.capture.exit_after_capture,

            session_persist_transparent: config.session.persist_transparent,
            session_persist_whiteboard: config.session.persist_whiteboard,
            session_persist_blackboard: config.session.persist_blackboard,
            session_persist_history: config.session.persist_history,
            session_restore_tool_state: config.session.restore_tool_state,
            session_per_output: config.session.per_output,
            session_storage_mode: SessionStorageModeOption::from_mode(
                config.session.storage.clone(),
            ),
            session_custom_directory: config.session.custom_directory.clone().unwrap_or_default(),
            session_max_shapes_per_frame: config.session.max_shapes_per_frame.to_string(),
            session_max_file_size_mb: config.session.max_file_size_mb.to_string(),
            session_compression: SessionCompressionOption::from_compression(
                config.session.compress.clone(),
            ),
            session_auto_compress_threshold_kb: config
                .session
                .auto_compress_threshold_kb
                .to_string(),
            session_max_persisted_undo_depth: config
                .session
                .max_persisted_undo_depth
                .map(|value| value.to_string())
                .unwrap_or_default(),
            session_backup_retention: config.session.backup_retention.to_string(),

            #[cfg(feature = "tablet-input")]
            tablet_enabled: config.tablet.enabled,
            #[cfg(feature = "tablet-input")]
            tablet_pressure_enabled: config.tablet.pressure_enabled,
            #[cfg(feature = "tablet-input")]
            tablet_min_thickness: format_float(config.tablet.min_thickness),
            #[cfg(feature = "tablet-input")]
            tablet_max_thickness: format_float(config.tablet.max_thickness),

            presets: PresetsDraft::from_config(config),

            keybindings: KeybindingsDraft::from_config(&config.keybindings),
        }
    }

    pub fn to_config(&self, base: &Config) -> Result<Config, Vec<FormError>> {
        let mut errors = Vec::new();
        let mut config = base.clone();

        match self.drawing_color.to_color_spec() {
            Ok(color) => config.drawing.default_color = color,
            Err(err) => errors.push(err),
        }
        parse_field(
            &self.drawing_default_thickness,
            "drawing.default_thickness",
            &mut errors,
            |value| config.drawing.default_thickness = value,
        );
        parse_field(
            &self.drawing_default_eraser_size,
            "drawing.default_eraser_size",
            &mut errors,
            |value| config.drawing.default_eraser_size = value,
        );
        config.drawing.default_eraser_mode = self.drawing_default_eraser_mode.to_mode();
        parse_field(
            &self.drawing_default_font_size,
            "drawing.default_font_size",
            &mut errors,
            |value| config.drawing.default_font_size = value,
        );
        parse_field(
            &self.drawing_marker_opacity,
            "drawing.marker_opacity",
            &mut errors,
            |value| config.drawing.marker_opacity = value,
        );
        config.drawing.font_family = self.drawing_font_family.clone();
        config.drawing.font_weight = self.drawing_font_weight.clone();
        config.drawing.font_style = self.drawing_font_style.clone();
        config.drawing.text_background_enabled = self.drawing_text_background_enabled;
        config.drawing.default_fill_enabled = self.drawing_default_fill_enabled;
        parse_field(
            &self.drawing_hit_test_tolerance,
            "drawing.hit_test_tolerance",
            &mut errors,
            |value| config.drawing.hit_test_tolerance = value,
        );
        parse_usize_field(
            &self.drawing_hit_test_linear_threshold,
            "drawing.hit_test_linear_threshold",
            &mut errors,
            |value| config.drawing.hit_test_linear_threshold = value,
        );
        parse_usize_field(
            &self.drawing_undo_stack_limit,
            "drawing.undo_stack_limit",
            &mut errors,
            |value| config.drawing.undo_stack_limit = value,
        );

        parse_field(&self.arrow_length, "arrow.length", &mut errors, |value| {
            config.arrow.length = value
        });
        parse_field(
            &self.arrow_angle,
            "arrow.angle_degrees",
            &mut errors,
            |value| config.arrow.angle_degrees = value,
        );
        config.arrow.head_at_end = self.arrow_head_at_end;

        parse_u64_field(
            &self.history_undo_all_delay_ms,
            "history.undo_all_delay_ms",
            &mut errors,
            |value| config.history.undo_all_delay_ms = value,
        );
        parse_u64_field(
            &self.history_redo_all_delay_ms,
            "history.redo_all_delay_ms",
            &mut errors,
            |value| config.history.redo_all_delay_ms = value,
        );
        config.history.custom_section_enabled = self.history_custom_section_enabled;
        parse_u64_field(
            &self.history_custom_undo_delay_ms,
            "history.custom_undo_delay_ms",
            &mut errors,
            |value| config.history.custom_undo_delay_ms = value,
        );
        parse_u64_field(
            &self.history_custom_redo_delay_ms,
            "history.custom_redo_delay_ms",
            &mut errors,
            |value| config.history.custom_redo_delay_ms = value,
        );
        parse_usize_field(
            &self.history_custom_undo_steps,
            "history.custom_undo_steps",
            &mut errors,
            |value| config.history.custom_undo_steps = value,
        );
        parse_usize_field(
            &self.history_custom_redo_steps,
            "history.custom_redo_steps",
            &mut errors,
            |value| config.history.custom_redo_steps = value,
        );

        config.performance.buffer_count = self.performance_buffer_count;
        config.performance.enable_vsync = self.performance_enable_vsync;
        parse_u32_field(
            &self.performance_ui_animation_fps,
            "performance.ui_animation_fps",
            &mut errors,
            |value| config.performance.ui_animation_fps = value,
        );

        config.ui.show_status_bar = self.ui_show_status_bar;
        config.ui.show_frozen_badge = self.ui_show_frozen_badge;
        config.ui.context_menu.enabled = self.ui_context_menu_enabled;
        let preferred_output = self.ui_preferred_output.trim();
        config.ui.preferred_output = if preferred_output.is_empty() {
            None
        } else {
            Some(preferred_output.to_string())
        };
        config.ui.xdg_fullscreen = self.ui_xdg_fullscreen;
        config.ui.toolbar.top_pinned = self.ui_toolbar_top_pinned;
        config.ui.toolbar.side_pinned = self.ui_toolbar_side_pinned;
        config.ui.toolbar.use_icons = self.ui_toolbar_use_icons;
        config.ui.toolbar.show_more_colors = self.ui_toolbar_show_more_colors;
        config.ui.toolbar.show_preset_toasts = self.ui_toolbar_show_preset_toasts;
        config.ui.toolbar.layout_mode = self.ui_toolbar_layout_mode.to_mode();
        config.ui.toolbar.mode_overrides = self.ui_toolbar_mode_overrides.to_config();
        config.ui.toolbar.show_presets = self.ui_toolbar_show_presets;
        config.ui.toolbar.show_actions_section = self.ui_toolbar_show_actions_section;
        config.ui.toolbar.show_actions_advanced = self.ui_toolbar_show_actions_advanced;
        config.ui.toolbar.show_step_section = self.ui_toolbar_show_step_section;
        config.ui.toolbar.show_text_controls = self.ui_toolbar_show_text_controls;
        config.ui.toolbar.show_settings_section = self.ui_toolbar_show_settings_section;
        config.ui.toolbar.show_delay_sliders = self.ui_toolbar_show_delay_sliders;
        config.ui.toolbar.show_marker_opacity_section = self.ui_toolbar_show_marker_opacity_section;
        config.ui.toolbar.show_tool_preview = self.ui_toolbar_show_tool_preview;
        config.ui.toolbar.force_inline = self.ui_toolbar_force_inline;
        parse_field(
            &self.ui_toolbar_top_offset,
            "ui.toolbar.top_offset",
            &mut errors,
            |value| config.ui.toolbar.top_offset = value,
        );
        parse_field(
            &self.ui_toolbar_top_offset_y,
            "ui.toolbar.top_offset_y",
            &mut errors,
            |value| config.ui.toolbar.top_offset_y = value,
        );
        parse_field(
            &self.ui_toolbar_side_offset,
            "ui.toolbar.side_offset",
            &mut errors,
            |value| config.ui.toolbar.side_offset = value,
        );
        parse_field(
            &self.ui_toolbar_side_offset_x,
            "ui.toolbar.side_offset_x",
            &mut errors,
            |value| config.ui.toolbar.side_offset_x = value,
        );
        config.ui.status_bar_position = self.ui_status_position.to_status_position();
        parse_field(
            &self.status_font_size,
            "ui.status_bar_style.font_size",
            &mut errors,
            |value| config.ui.status_bar_style.font_size = value,
        );
        parse_field(
            &self.status_padding,
            "ui.status_bar_style.padding",
            &mut errors,
            |value| config.ui.status_bar_style.padding = value,
        );
        match self
            .status_bar_bg_color
            .to_array("ui.status_bar_style.bg_color")
        {
            Ok(values) => config.ui.status_bar_style.bg_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .status_bar_text_color
            .to_array("ui.status_bar_style.text_color")
        {
            Ok(values) => config.ui.status_bar_style.text_color = values,
            Err(err) => errors.push(err),
        }
        parse_field(
            &self.status_dot_radius,
            "ui.status_bar_style.dot_radius",
            &mut errors,
            |value| config.ui.status_bar_style.dot_radius = value,
        );

        config.ui.click_highlight.enabled = self.click_highlight_enabled;
        config.ui.click_highlight.use_pen_color = self.click_highlight_use_pen_color;
        parse_field(
            &self.click_highlight_radius,
            "ui.click_highlight.radius",
            &mut errors,
            |value| config.ui.click_highlight.radius = value,
        );
        parse_field(
            &self.click_highlight_outline_thickness,
            "ui.click_highlight.outline_thickness",
            &mut errors,
            |value| config.ui.click_highlight.outline_thickness = value,
        );
        parse_u64_field(
            &self.click_highlight_duration_ms,
            "ui.click_highlight.duration_ms",
            &mut errors,
            |value| config.ui.click_highlight.duration_ms = value,
        );
        match self
            .click_highlight_fill_color
            .to_array("ui.click_highlight.fill_color")
        {
            Ok(values) => config.ui.click_highlight.fill_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .click_highlight_outline_color
            .to_array("ui.click_highlight.outline_color")
        {
            Ok(values) => config.ui.click_highlight.outline_color = values,
            Err(err) => errors.push(err),
        }

        parse_field(
            &self.help_font_size,
            "ui.help_overlay_style.font_size",
            &mut errors,
            |value| config.ui.help_overlay_style.font_size = value,
        );
        parse_field(
            &self.help_line_height,
            "ui.help_overlay_style.line_height",
            &mut errors,
            |value| config.ui.help_overlay_style.line_height = value,
        );
        parse_field(
            &self.help_padding,
            "ui.help_overlay_style.padding",
            &mut errors,
            |value| config.ui.help_overlay_style.padding = value,
        );
        match self
            .help_bg_color
            .to_array("ui.help_overlay_style.bg_color")
        {
            Ok(values) => config.ui.help_overlay_style.bg_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .help_border_color
            .to_array("ui.help_overlay_style.border_color")
        {
            Ok(values) => config.ui.help_overlay_style.border_color = values,
            Err(err) => errors.push(err),
        }
        parse_field(
            &self.help_border_width,
            "ui.help_overlay_style.border_width",
            &mut errors,
            |value| config.ui.help_overlay_style.border_width = value,
        );
        match self
            .help_text_color
            .to_array("ui.help_overlay_style.text_color")
        {
            Ok(values) => config.ui.help_overlay_style.text_color = values,
            Err(err) => errors.push(err),
        }

        config.board.enabled = self.board_enabled;
        config.board.default_mode = self.board_default_mode.as_str().to_string();
        match self
            .board_whiteboard_color
            .to_array("board.whiteboard_color")
        {
            Ok(values) => config.board.whiteboard_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .board_blackboard_color
            .to_array("board.blackboard_color")
        {
            Ok(values) => config.board.blackboard_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .board_whiteboard_pen
            .to_array("board.whiteboard_pen_color")
        {
            Ok(values) => config.board.whiteboard_pen_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .board_blackboard_pen
            .to_array("board.blackboard_pen_color")
        {
            Ok(values) => config.board.blackboard_pen_color = values,
            Err(err) => errors.push(err),
        }
        config.board.auto_adjust_pen = self.board_auto_adjust_pen;

        config.capture.enabled = self.capture_enabled;
        config.capture.save_directory = self.capture_save_directory.clone();
        config.capture.filename_template = self.capture_filename_template.clone();
        config.capture.format = self.capture_format.clone();
        config.capture.copy_to_clipboard = self.capture_copy_to_clipboard;
        config.capture.exit_after_capture = self.capture_exit_after;

        config.session.persist_transparent = self.session_persist_transparent;
        config.session.persist_whiteboard = self.session_persist_whiteboard;
        config.session.persist_blackboard = self.session_persist_blackboard;
        config.session.persist_history = self.session_persist_history;
        config.session.restore_tool_state = self.session_restore_tool_state;
        config.session.per_output = self.session_per_output;
        config.session.storage = self.session_storage_mode.to_mode();
        let custom_dir = self.session_custom_directory.trim();
        config.session.custom_directory = if custom_dir.is_empty() {
            None
        } else {
            Some(custom_dir.to_string())
        };
        parse_usize_field(
            &self.session_max_shapes_per_frame,
            "session.max_shapes_per_frame",
            &mut errors,
            |value| config.session.max_shapes_per_frame = value,
        );
        parse_u64_field(
            &self.session_max_file_size_mb,
            "session.max_file_size_mb",
            &mut errors,
            |value| config.session.max_file_size_mb = value,
        );
        config.session.compress = self.session_compression.to_compression();
        parse_u64_field(
            &self.session_auto_compress_threshold_kb,
            "session.auto_compress_threshold_kb",
            &mut errors,
            |value| config.session.auto_compress_threshold_kb = value,
        );
        parse_optional_usize_field(
            &self.session_max_persisted_undo_depth,
            "session.max_persisted_undo_depth",
            &mut errors,
            |value| config.session.max_persisted_undo_depth = value,
        );
        parse_usize_field(
            &self.session_backup_retention,
            "session.backup_retention",
            &mut errors,
            |value| config.session.backup_retention = value,
        );

        #[cfg(feature = "tablet-input")]
        {
            config.tablet.enabled = self.tablet_enabled;
            config.tablet.pressure_enabled = self.tablet_pressure_enabled;
            parse_field(
                &self.tablet_min_thickness,
                "tablet.min_thickness",
                &mut errors,
                |value| config.tablet.min_thickness = value,
            );
            parse_field(
                &self.tablet_max_thickness,
                "tablet.max_thickness",
                &mut errors,
                |value| config.tablet.max_thickness = value,
            );
        }

        config.presets = self.presets.to_config(&mut errors);

        match self.keybindings.to_config() {
            Ok(cfg) => config.keybindings = cfg,
            Err(errs) => errors.extend(errs),
        }

        if errors.is_empty() {
            Ok(config)
        } else {
            Err(errors)
        }
    }

    pub fn apply_toolbar_layout_mode(&mut self, mode: ToolbarLayoutModeOption) {
        self.ui_toolbar_layout_mode = mode;
        let defaults = mode.to_mode().section_defaults();
        self.ui_toolbar_show_actions_section = defaults.show_actions_section;
        self.ui_toolbar_show_actions_advanced = defaults.show_actions_advanced;
        self.ui_toolbar_show_presets = defaults.show_presets;
        self.ui_toolbar_show_step_section = defaults.show_step_section;
        self.ui_toolbar_show_text_controls = defaults.show_text_controls;
        self.ui_toolbar_show_settings_section = defaults.show_settings_section;
    }

    pub fn set_toolbar_override(
        &mut self,
        mode: ToolbarLayoutModeOption,
        field: ToolbarOverrideField,
        value: OverrideOption,
    ) {
        self.ui_toolbar_mode_overrides
            .for_mode_mut(mode)
            .set(field, value);
    }

    pub fn set_toggle(&mut self, field: ToggleField, value: bool) {
        match field {
            ToggleField::DrawingTextBackground => {
                self.drawing_text_background_enabled = value;
            }
            ToggleField::DrawingFillEnabled => {
                self.drawing_default_fill_enabled = value;
            }
            ToggleField::PerformanceVsync => self.performance_enable_vsync = value,
            ToggleField::UiShowStatusBar => self.ui_show_status_bar = value,
            ToggleField::UiShowFrozenBadge => self.ui_show_frozen_badge = value,
            ToggleField::UiContextMenuEnabled => self.ui_context_menu_enabled = value,
            ToggleField::UiXdgFullscreen => self.ui_xdg_fullscreen = value,
            ToggleField::UiToolbarTopPinned => self.ui_toolbar_top_pinned = value,
            ToggleField::UiToolbarSidePinned => self.ui_toolbar_side_pinned = value,
            ToggleField::UiToolbarUseIcons => self.ui_toolbar_use_icons = value,
            ToggleField::UiToolbarShowMoreColors => self.ui_toolbar_show_more_colors = value,
            ToggleField::UiToolbarPresetToasts => self.ui_toolbar_show_preset_toasts = value,
            ToggleField::UiToolbarShowPresets => self.ui_toolbar_show_presets = value,
            ToggleField::UiToolbarShowActionsSection => {
                self.ui_toolbar_show_actions_section = value;
            }
            ToggleField::UiToolbarShowActionsAdvanced => {
                self.ui_toolbar_show_actions_advanced = value;
            }
            ToggleField::UiToolbarShowStepSection => self.ui_toolbar_show_step_section = value,
            ToggleField::UiToolbarShowTextControls => self.ui_toolbar_show_text_controls = value,
            ToggleField::UiToolbarShowSettingsSection => {
                self.ui_toolbar_show_settings_section = value;
            }
            ToggleField::UiToolbarShowDelaySliders => {
                self.ui_toolbar_show_delay_sliders = value;
            }
            ToggleField::UiToolbarShowMarkerOpacitySection => {
                self.ui_toolbar_show_marker_opacity_section = value;
            }
            ToggleField::UiToolbarShowToolPreview => {
                self.ui_toolbar_show_tool_preview = value;
            }
            ToggleField::UiToolbarForceInline => {
                self.ui_toolbar_force_inline = value;
            }
            ToggleField::UiClickHighlightEnabled => self.click_highlight_enabled = value,
            ToggleField::UiClickHighlightUsePenColor => self.click_highlight_use_pen_color = value,
            ToggleField::BoardEnabled => self.board_enabled = value,
            ToggleField::BoardAutoAdjust => self.board_auto_adjust_pen = value,
            ToggleField::CaptureEnabled => self.capture_enabled = value,
            ToggleField::CaptureCopyToClipboard => self.capture_copy_to_clipboard = value,
            ToggleField::CaptureExitAfter => self.capture_exit_after = value,
            ToggleField::SessionPersistTransparent => {
                self.session_persist_transparent = value;
            }
            ToggleField::SessionPersistWhiteboard => {
                self.session_persist_whiteboard = value;
            }
            ToggleField::SessionPersistBlackboard => {
                self.session_persist_blackboard = value;
            }
            ToggleField::SessionPersistHistory => {
                self.session_persist_history = value;
            }
            ToggleField::SessionRestoreToolState => {
                self.session_restore_tool_state = value;
            }
            ToggleField::SessionPerOutput => {
                self.session_per_output = value;
            }
            ToggleField::HistoryCustomSectionEnabled => {
                self.history_custom_section_enabled = value;
            }
            ToggleField::ArrowHeadAtEnd => {
                self.arrow_head_at_end = value;
            }
            #[cfg(feature = "tablet-input")]
            ToggleField::TabletEnabled => self.tablet_enabled = value,
            #[cfg(feature = "tablet-input")]
            ToggleField::TabletPressureEnabled => self.tablet_pressure_enabled = value,
        }
    }

    pub fn set_text(&mut self, field: TextField, value: String) {
        match field {
            TextField::DrawingColorName => {
                self.drawing_color.name = value;
                self.drawing_color.update_named_from_current();
            }
            TextField::DrawingThickness => self.drawing_default_thickness = value,
            TextField::DrawingEraserSize => self.drawing_default_eraser_size = value,
            TextField::DrawingFontSize => self.drawing_default_font_size = value,
            TextField::DrawingMarkerOpacity => self.drawing_marker_opacity = value,
            TextField::DrawingFontFamily => self.drawing_font_family = value,
            TextField::DrawingFontWeight => {
                self.drawing_font_weight = value;
                self.drawing_font_weight_option = FontWeightOption::Custom;
            }
            TextField::DrawingFontStyle => {
                self.drawing_font_style = value;
                self.drawing_font_style_option = FontStyleOption::Custom;
            }
            TextField::DrawingHitTestTolerance => self.drawing_hit_test_tolerance = value,
            TextField::DrawingHitTestThreshold => self.drawing_hit_test_linear_threshold = value,
            TextField::DrawingUndoStackLimit => self.drawing_undo_stack_limit = value,
            TextField::ArrowLength => self.arrow_length = value,
            TextField::ArrowAngle => self.arrow_angle = value,
            TextField::PerformanceUiAnimationFps => self.performance_ui_animation_fps = value,
            TextField::HistoryUndoAllDelayMs => self.history_undo_all_delay_ms = value,
            TextField::HistoryRedoAllDelayMs => self.history_redo_all_delay_ms = value,
            TextField::HistoryCustomUndoDelayMs => self.history_custom_undo_delay_ms = value,
            TextField::HistoryCustomRedoDelayMs => self.history_custom_redo_delay_ms = value,
            TextField::HistoryCustomUndoSteps => self.history_custom_undo_steps = value,
            TextField::HistoryCustomRedoSteps => self.history_custom_redo_steps = value,
            TextField::UiPreferredOutput => self.ui_preferred_output = value,
            TextField::StatusFontSize => self.status_font_size = value,
            TextField::StatusPadding => self.status_padding = value,
            TextField::StatusDotRadius => self.status_dot_radius = value,
            TextField::HighlightRadius => self.click_highlight_radius = value,
            TextField::HighlightOutlineThickness => self.click_highlight_outline_thickness = value,
            TextField::HighlightDurationMs => self.click_highlight_duration_ms = value,
            TextField::HelpFontSize => self.help_font_size = value,
            TextField::HelpLineHeight => self.help_line_height = value,
            TextField::HelpPadding => self.help_padding = value,
            TextField::HelpBorderWidth => self.help_border_width = value,
            TextField::CaptureSaveDirectory => self.capture_save_directory = value,
            TextField::CaptureFilename => self.capture_filename_template = value,
            TextField::CaptureFormat => self.capture_format = value,
            TextField::ToolbarTopOffset => self.ui_toolbar_top_offset = value,
            TextField::ToolbarTopOffsetY => self.ui_toolbar_top_offset_y = value,
            TextField::ToolbarSideOffset => self.ui_toolbar_side_offset = value,
            TextField::ToolbarSideOffsetX => self.ui_toolbar_side_offset_x = value,
            TextField::SessionCustomDirectory => self.session_custom_directory = value,
            TextField::SessionMaxShapesPerFrame => self.session_max_shapes_per_frame = value,
            TextField::SessionMaxFileSizeMb => self.session_max_file_size_mb = value,
            TextField::SessionAutoCompressThresholdKb => {
                self.session_auto_compress_threshold_kb = value
            }
            TextField::SessionMaxPersistedUndoDepth => {
                self.session_max_persisted_undo_depth = value
            }
            TextField::SessionBackupRetention => self.session_backup_retention = value,
            #[cfg(feature = "tablet-input")]
            TextField::TabletMinThickness => self.tablet_min_thickness = value,
            #[cfg(feature = "tablet-input")]
            TextField::TabletMaxThickness => self.tablet_max_thickness = value,
        }
    }

    pub fn set_triplet(&mut self, field: TripletField, index: usize, value: String) {
        match field {
            TripletField::DrawingColorRgb => {
                if let Some(slot) = self.drawing_color.rgb.get_mut(index) {
                    *slot = value;
                }
            }
            TripletField::BoardWhiteboard => {
                self.board_whiteboard_color.set_component(index, value)
            }
            TripletField::BoardBlackboard => {
                self.board_blackboard_color.set_component(index, value)
            }
            TripletField::BoardWhiteboardPen => {
                self.board_whiteboard_pen.set_component(index, value)
            }
            TripletField::BoardBlackboardPen => {
                self.board_blackboard_pen.set_component(index, value)
            }
        }
    }

    pub fn set_quad(&mut self, field: QuadField, index: usize, value: String) {
        match field {
            QuadField::StatusBarBg => self.status_bar_bg_color.set_component(index, value),
            QuadField::StatusBarText => self.status_bar_text_color.set_component(index, value),
            QuadField::HelpBg => self.help_bg_color.set_component(index, value),
            QuadField::HelpBorder => self.help_border_color.set_component(index, value),
            QuadField::HelpText => self.help_text_color.set_component(index, value),
            QuadField::HighlightFill => self.click_highlight_fill_color.set_component(index, value),
            QuadField::HighlightOutline => self
                .click_highlight_outline_color
                .set_component(index, value),
        }
    }
}

fn parse_field<F>(value: &str, field: &'static str, errors: &mut Vec<FormError>, apply: F)
where
    F: FnOnce(f64),
{
    match parse_f64(value.trim()) {
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err)),
    }
}

fn parse_usize_field<F>(value: &str, field: &'static str, errors: &mut Vec<FormError>, apply: F)
where
    F: FnOnce(usize),
{
    match value.trim().parse::<usize>() {
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}

fn parse_optional_usize_field<F>(
    value: &str,
    field: &'static str,
    errors: &mut Vec<FormError>,
    apply: F,
) where
    F: FnOnce(Option<usize>),
{
    let trimmed = value.trim();
    if trimmed.is_empty() {
        apply(None);
        return;
    }
    match trimmed.parse::<usize>() {
        Ok(parsed) => apply(Some(parsed)),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}

fn parse_required_f64(value: &str, field: String, errors: &mut Vec<FormError>) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        errors.push(FormError::new(field, "Value is required"));
        return None;
    }
    match parse_f64(trimmed) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            errors.push(FormError::new(field, err));
            None
        }
    }
}

fn parse_optional_f64(value: &str, field: String, errors: &mut Vec<FormError>) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    match parse_f64(trimmed) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            errors.push(FormError::new(field, err));
            None
        }
    }
}

fn parse_u64_field<F>(value: &str, field: &'static str, errors: &mut Vec<FormError>, apply: F)
where
    F: FnOnce(u64),
{
    match value.trim().parse::<u64>() {
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}

fn parse_u32_field<F>(value: &str, field: &'static str, errors: &mut Vec<FormError>, apply: F)
where
    F: FnOnce(u32),
{
    match value.trim().parse::<u32>() {
        Ok(parsed) => apply(parsed),
        Err(err) => errors.push(FormError::new(field, err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ColorMode, NamedColorOption};
    use wayscriber::config::Config;

    #[test]
    fn config_draft_to_config_reports_errors() {
        let mut draft = ConfigDraft::from_config(&Config::default());
        draft.drawing_default_thickness = "nope".to_string();
        draft.click_highlight_duration_ms = "nan".to_string();
        draft.drawing_color = ColorInput {
            mode: ColorMode::Named,
            name: " ".to_string(),
            rgb: ["0".to_string(), "0".to_string(), "0".to_string()],
            selected_named: NamedColorOption::Custom,
        };

        let errors = draft
            .to_config(&Config::default())
            .expect_err("expected validation errors");
        let fields: Vec<&str> = errors.iter().map(|err| err.field.as_str()).collect();

        assert!(fields.contains(&"drawing.default_thickness"));
        assert!(fields.contains(&"ui.click_highlight.duration_ms"));
        assert!(fields.contains(&"drawing.default_color"));
    }

    #[test]
    fn config_draft_to_config_trims_custom_directory() {
        let mut draft = ConfigDraft::from_config(&Config::default());
        draft.session_storage_mode = SessionStorageModeOption::Custom;
        draft.session_custom_directory = "   ".to_string();

        let config = draft
            .to_config(&Config::default())
            .expect("to_config should succeed");
        assert!(config.session.custom_directory.is_none());
    }

    #[test]
    fn setters_update_draft_state() {
        let mut draft = ConfigDraft::from_config(&Config::default());

        draft.set_text(TextField::DrawingFontWeight, "weird".to_string());
        assert_eq!(draft.drawing_font_weight, "weird");
        assert_eq!(draft.drawing_font_weight_option, FontWeightOption::Custom);

        draft.set_text(TextField::DrawingColorName, "green".to_string());
        assert_eq!(draft.drawing_color.name, "green");
        assert_eq!(draft.drawing_color.selected_named, NamedColorOption::Green);

        draft.set_triplet(TripletField::BoardWhiteboard, 1, "0.5".to_string());
        assert_eq!(draft.board_whiteboard_color.components[1], "0.5");

        draft.set_quad(QuadField::StatusBarBg, 2, "0.75".to_string());
        assert_eq!(draft.status_bar_bg_color.components[2], "0.75");

        draft.set_toggle(ToggleField::BoardEnabled, true);
        draft.set_toggle(ToggleField::ArrowHeadAtEnd, true);
        assert!(draft.board_enabled);
        assert!(draft.arrow_head_at_end);
    }
}
