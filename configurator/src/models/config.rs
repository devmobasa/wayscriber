use hyprmarker::config::Config;

use super::color::{ColorInput, ColorQuadInput, ColorTripletInput};
use super::error::FormError;
use super::fields::{
    BoardModeOption, FontStyleOption, FontWeightOption, QuadField, StatusPositionOption, TextField,
    ToggleField, TripletField,
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

    pub performance_buffer_count: u32,
    pub performance_enable_vsync: bool,

    pub ui_show_status_bar: bool,
    pub ui_status_position: StatusPositionOption,
    pub status_font_size: String,
    pub status_padding: String,
    pub status_bar_bg_color: ColorQuadInput,
    pub status_bar_text_color: ColorQuadInput,
    pub status_dot_radius: String,

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

            performance_buffer_count: config.performance.buffer_count,
            performance_enable_vsync: config.performance.enable_vsync,

            ui_show_status_bar: config.ui.show_status_bar,
            ui_status_position: StatusPositionOption::from_status_position(
                config.ui.status_bar_position,
            ),
            status_font_size: format_float(config.ui.status_bar_style.font_size),
            status_padding: format_float(config.ui.status_bar_style.padding),
            status_bar_bg_color: ColorQuadInput::from(config.ui.status_bar_style.bg_color),
            status_bar_text_color: ColorQuadInput::from(config.ui.status_bar_style.text_color),
            status_dot_radius: format_float(config.ui.status_bar_style.dot_radius),

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

        config.performance.buffer_count = self.performance_buffer_count;
        config.performance.enable_vsync = self.performance_enable_vsync;

        config.ui.show_status_bar = self.ui_show_status_bar;
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

    pub fn set_toggle(&mut self, field: ToggleField, value: bool) {
        match field {
            ToggleField::DrawingTextBackground => {
                self.drawing_text_background_enabled = value;
            }
            ToggleField::PerformanceVsync => self.performance_enable_vsync = value,
            ToggleField::UiShowStatusBar => self.ui_show_status_bar = value,
            ToggleField::BoardEnabled => self.board_enabled = value,
            ToggleField::BoardAutoAdjust => self.board_auto_adjust_pen = value,
            ToggleField::CaptureEnabled => self.capture_enabled = value,
            ToggleField::CaptureCopyToClipboard => self.capture_copy_to_clipboard = value,
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
            TextField::HelpFontSize => self.help_font_size = value,
            TextField::HelpLineHeight => self.help_line_height = value,
            TextField::HelpPadding => self.help_padding = value,
            TextField::HelpBorderWidth => self.help_border_width = value,
            TextField::CaptureSaveDirectory => self.capture_save_directory = value,
            TextField::CaptureFilename => self.capture_filename_template = value,
            TextField::CaptureFormat => self.capture_format = value,
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
