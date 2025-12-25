use wayscriber::config::{Config, PresetSlotsConfig};

use super::color::{ColorInput, ColorQuadInput, ColorTripletInput};
use super::error::FormError;
use super::fields::{
    BoardModeOption, FontStyleOption, FontWeightOption, QuadField, SessionCompressionOption,
    SessionStorageModeOption, StatusPositionOption, TextField, ToggleField, ToolbarLayoutModeOption,
    TripletField,
};
use super::keybindings::KeybindingsDraft;
use super::util::{format_float, parse_f64};

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigDraft {
    pub drawing_color: ColorInput,
    pub drawing_default_thickness: String,
    pub drawing_default_font_size: String,
    pub drawing_font_family: String,
    pub drawing_font_weight: String,
    pub drawing_font_style: String,
    pub drawing_text_background_enabled: bool,
    pub drawing_font_style_option: FontStyleOption,
    pub drawing_font_weight_option: FontWeightOption,

    pub arrow_length: String,
    pub arrow_angle: String,
    pub arrow_head_at_end: bool,

    pub performance_buffer_count: u32,
    pub performance_enable_vsync: bool,

    pub ui_show_status_bar: bool,
    pub ui_show_frozen_badge: bool,
    pub ui_toolbar_show_preset_toasts: bool,
    pub ui_toolbar_layout_mode: ToolbarLayoutModeOption,
    pub ui_toolbar_show_presets: bool,
    pub ui_toolbar_show_actions_section: bool,
    pub ui_toolbar_show_actions_advanced: bool,
    pub ui_toolbar_show_step_section: bool,
    pub ui_toolbar_show_text_controls: bool,
    pub ui_toolbar_show_settings_section: bool,
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
    pub session_restore_tool_state: bool,
    pub session_per_output: bool,
    pub session_storage_mode: SessionStorageModeOption,
    pub session_custom_directory: String,
    pub session_max_shapes_per_frame: String,
    pub session_max_file_size_mb: String,
    pub session_compression: SessionCompressionOption,
    pub session_auto_compress_threshold_kb: String,
    pub session_backup_retention: String,

    pub presets: PresetSlotsConfig,

    pub keybindings: KeybindingsDraft,
}

impl ConfigDraft {
    pub fn from_config(config: &Config) -> Self {
        let (style_option, style_value) = FontStyleOption::from_value(&config.drawing.font_style);
        let (weight_option, weight_value) =
            FontWeightOption::from_value(&config.drawing.font_weight);
        Self {
            drawing_color: ColorInput::from_color(&config.drawing.default_color),
            drawing_default_thickness: format_float(config.drawing.default_thickness),
            drawing_default_font_size: format_float(config.drawing.default_font_size),
            drawing_font_family: config.drawing.font_family.clone(),
            drawing_font_weight: weight_value,
            drawing_font_style: style_value,
            drawing_text_background_enabled: config.drawing.text_background_enabled,
            drawing_font_style_option: style_option,
            drawing_font_weight_option: weight_option,

            arrow_length: format_float(config.arrow.length),
            arrow_angle: format_float(config.arrow.angle_degrees),
            arrow_head_at_end: config.arrow.head_at_end,

            performance_buffer_count: config.performance.buffer_count,
            performance_enable_vsync: config.performance.enable_vsync,

            ui_show_status_bar: config.ui.show_status_bar,
            ui_show_frozen_badge: config.ui.show_frozen_badge,
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
            session_backup_retention: config.session.backup_retention.to_string(),

            presets: config.presets.clone(),

            keybindings: KeybindingsDraft::from_config(&config.keybindings),
        }
    }

    pub fn to_config(&self) -> Result<Config, Vec<FormError>> {
        let mut errors = Vec::new();
        let mut config = Config::default();

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
            &self.drawing_default_font_size,
            "drawing.default_font_size",
            &mut errors,
            |value| config.drawing.default_font_size = value,
        );
        config.drawing.font_family = self.drawing_font_family.clone();
        config.drawing.font_weight = self.drawing_font_weight.clone();
        config.drawing.font_style = self.drawing_font_style.clone();
        config.drawing.text_background_enabled = self.drawing_text_background_enabled;

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

        config.performance.buffer_count = self.performance_buffer_count;
        config.performance.enable_vsync = self.performance_enable_vsync;

        config.ui.show_status_bar = self.ui_show_status_bar;
        config.ui.show_frozen_badge = self.ui_show_frozen_badge;
        config.ui.toolbar.show_preset_toasts = self.ui_toolbar_show_preset_toasts;
        config.ui.toolbar.layout_mode = self.ui_toolbar_layout_mode.to_mode();
        config.ui.toolbar.show_presets = self.ui_toolbar_show_presets;
        config.ui.toolbar.show_actions_section = self.ui_toolbar_show_actions_section;
        config.ui.toolbar.show_actions_advanced = self.ui_toolbar_show_actions_advanced;
        config.ui.toolbar.show_step_section = self.ui_toolbar_show_step_section;
        config.ui.toolbar.show_text_controls = self.ui_toolbar_show_text_controls;
        config.ui.toolbar.show_settings_section = self.ui_toolbar_show_settings_section;
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
        parse_usize_field(
            &self.session_backup_retention,
            "session.backup_retention",
            &mut errors,
            |value| config.session.backup_retention = value,
        );

        config.presets = self.presets.clone();

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

    pub fn set_toggle(&mut self, field: ToggleField, value: bool) {
        match field {
            ToggleField::DrawingTextBackground => {
                self.drawing_text_background_enabled = value;
            }
            ToggleField::PerformanceVsync => self.performance_enable_vsync = value,
            ToggleField::UiShowStatusBar => self.ui_show_status_bar = value,
            ToggleField::UiShowFrozenBadge => self.ui_show_frozen_badge = value,
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
            ToggleField::SessionRestoreToolState => {
                self.session_restore_tool_state = value;
            }
            ToggleField::SessionPerOutput => {
                self.session_per_output = value;
            }
            ToggleField::ArrowHeadAtEnd => {
                self.arrow_head_at_end = value;
            }
        }
    }

    pub fn set_text(&mut self, field: TextField, value: String) {
        match field {
            TextField::DrawingColorName => {
                self.drawing_color.name = value;
                self.drawing_color.update_named_from_current();
            }
            TextField::DrawingThickness => self.drawing_default_thickness = value,
            TextField::DrawingFontSize => self.drawing_default_font_size = value,
            TextField::DrawingFontFamily => self.drawing_font_family = value,
            TextField::DrawingFontWeight => {
                self.drawing_font_weight = value;
                self.drawing_font_weight_option = FontWeightOption::Custom;
            }
            TextField::DrawingFontStyle => {
                self.drawing_font_style = value;
                self.drawing_font_style_option = FontStyleOption::Custom;
            }
            TextField::ArrowLength => self.arrow_length = value,
            TextField::ArrowAngle => self.arrow_angle = value,
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
            TextField::SessionCustomDirectory => self.session_custom_directory = value,
            TextField::SessionMaxShapesPerFrame => self.session_max_shapes_per_frame = value,
            TextField::SessionMaxFileSizeMb => self.session_max_file_size_mb = value,
            TextField::SessionAutoCompressThresholdKb => {
                self.session_auto_compress_threshold_kb = value
            }
            TextField::SessionBackupRetention => self.session_backup_retention = value,
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

fn parse_u64_field<F>(value: &str, field: &'static str, errors: &mut Vec<FormError>, apply: F)
where
    F: FnOnce(u64),
{
    match value.trim().parse::<u64>() {
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

        let errors = draft.to_config().expect_err("expected validation errors");
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

        let config = draft.to_config().expect("to_config should succeed");
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
