use wayscriber::config::keybindings::KeybindingsConfig;

use super::error::FormError;

#[derive(Debug, Clone, PartialEq)]
pub struct KeybindingsDraft {
    pub entries: Vec<KeybindingEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeybindingEntry {
    pub field: KeybindingField,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeybindingField {
    Exit,
    EnterTextMode,
    ClearCanvas,
    Undo,
    IncreaseThickness,
    DecreaseThickness,
    SelectPenTool,
    SelectEraserTool,
    SelectMarkerTool,
    IncreaseFontSize,
    DecreaseFontSize,
    ToggleWhiteboard,
    ToggleBlackboard,
    ReturnToTransparent,
    ToggleHelp,
    OpenConfigurator,
    SetColorRed,
    SetColorGreen,
    SetColorBlue,
    SetColorYellow,
    SetColorOrange,
    SetColorPink,
    SetColorWhite,
    SetColorBlack,
    CaptureFullScreen,
    CaptureActiveWindow,
    CaptureSelection,
    CaptureClipboardFull,
    CaptureFileFull,
    CaptureClipboardSelection,
    CaptureFileSelection,
    CaptureClipboardRegion,
    CaptureFileRegion,
}

impl KeybindingsDraft {
    pub fn from_config(config: &KeybindingsConfig) -> Self {
        let entries = KeybindingField::all()
            .into_iter()
            .map(|field| KeybindingEntry {
                value: field.get(config).join(", "),
                field,
            })
            .collect();
        Self { entries }
    }

    pub fn set(&mut self, field: KeybindingField, value: String) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.field == field) {
            entry.value = value;
        }
    }

    pub fn to_config(&self) -> Result<KeybindingsConfig, Vec<FormError>> {
        let mut config = KeybindingsConfig::default();
        let mut errors = Vec::new();

        for entry in &self.entries {
            match parse_keybinding_list(&entry.value) {
                Ok(list) => entry.field.set(&mut config, list),
                Err(err) => errors.push(FormError::new(
                    format!("keybindings.{}", entry.field.field_key()),
                    err,
                )),
            }
        }

        if errors.is_empty() {
            Ok(config)
        } else {
            Err(errors)
        }
    }

    pub fn value_for(&self, field: KeybindingField) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.field == field)
            .map(|entry| entry.value.as_str())
    }
}

impl KeybindingField {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Exit,
            Self::EnterTextMode,
            Self::ClearCanvas,
            Self::Undo,
            Self::IncreaseThickness,
            Self::DecreaseThickness,
            Self::SelectPenTool,
            Self::SelectEraserTool,
            Self::SelectMarkerTool,
            Self::IncreaseFontSize,
            Self::DecreaseFontSize,
            Self::ToggleWhiteboard,
            Self::ToggleBlackboard,
            Self::ReturnToTransparent,
            Self::ToggleHelp,
            Self::OpenConfigurator,
            Self::SetColorRed,
            Self::SetColorGreen,
            Self::SetColorBlue,
            Self::SetColorYellow,
            Self::SetColorOrange,
            Self::SetColorPink,
            Self::SetColorWhite,
            Self::SetColorBlack,
            Self::CaptureFullScreen,
            Self::CaptureActiveWindow,
            Self::CaptureSelection,
            Self::CaptureClipboardFull,
            Self::CaptureFileFull,
            Self::CaptureClipboardSelection,
            Self::CaptureFileSelection,
            Self::CaptureClipboardRegion,
            Self::CaptureFileRegion,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Exit => "Exit",
            Self::EnterTextMode => "Enter text mode",
            Self::ClearCanvas => "Clear canvas",
            Self::Undo => "Undo",
            Self::IncreaseThickness => "Increase thickness",
            Self::DecreaseThickness => "Decrease thickness",
            Self::SelectPenTool => "Select pen tool",
            Self::SelectEraserTool => "Select eraser tool",
            Self::SelectMarkerTool => "Select marker tool",
            Self::IncreaseFontSize => "Increase font size",
            Self::DecreaseFontSize => "Decrease font size",
            Self::ToggleWhiteboard => "Toggle whiteboard",
            Self::ToggleBlackboard => "Toggle blackboard",
            Self::ReturnToTransparent => "Return to transparent",
            Self::ToggleHelp => "Toggle help",
            Self::OpenConfigurator => "Open configurator",
            Self::SetColorRed => "Color: red",
            Self::SetColorGreen => "Color: green",
            Self::SetColorBlue => "Color: blue",
            Self::SetColorYellow => "Color: yellow",
            Self::SetColorOrange => "Color: orange",
            Self::SetColorPink => "Color: pink",
            Self::SetColorWhite => "Color: white",
            Self::SetColorBlack => "Color: black",
            Self::CaptureFullScreen => "Capture full screen",
            Self::CaptureActiveWindow => "Capture active window",
            Self::CaptureSelection => "Capture selection",
            Self::CaptureClipboardFull => "Clipboard full screen",
            Self::CaptureFileFull => "File full screen",
            Self::CaptureClipboardSelection => "Clipboard selection",
            Self::CaptureFileSelection => "File selection",
            Self::CaptureClipboardRegion => "Clipboard region",
            Self::CaptureFileRegion => "File region",
        }
    }

    pub fn field_key(&self) -> &'static str {
        match self {
            Self::Exit => "exit",
            Self::EnterTextMode => "enter_text_mode",
            Self::ClearCanvas => "clear_canvas",
            Self::Undo => "undo",
            Self::IncreaseThickness => "increase_thickness",
            Self::DecreaseThickness => "decrease_thickness",
            Self::SelectPenTool => "select_pen_tool",
            Self::SelectEraserTool => "select_eraser_tool",
            Self::SelectMarkerTool => "select_marker_tool",
            Self::IncreaseFontSize => "increase_font_size",
            Self::DecreaseFontSize => "decrease_font_size",
            Self::ToggleWhiteboard => "toggle_whiteboard",
            Self::ToggleBlackboard => "toggle_blackboard",
            Self::ReturnToTransparent => "return_to_transparent",
            Self::ToggleHelp => "toggle_help",
            Self::OpenConfigurator => "open_configurator",
            Self::SetColorRed => "set_color_red",
            Self::SetColorGreen => "set_color_green",
            Self::SetColorBlue => "set_color_blue",
            Self::SetColorYellow => "set_color_yellow",
            Self::SetColorOrange => "set_color_orange",
            Self::SetColorPink => "set_color_pink",
            Self::SetColorWhite => "set_color_white",
            Self::SetColorBlack => "set_color_black",
            Self::CaptureFullScreen => "capture_full_screen",
            Self::CaptureActiveWindow => "capture_active_window",
            Self::CaptureSelection => "capture_selection",
            Self::CaptureClipboardFull => "capture_clipboard_full",
            Self::CaptureFileFull => "capture_file_full",
            Self::CaptureClipboardSelection => "capture_clipboard_selection",
            Self::CaptureFileSelection => "capture_file_selection",
            Self::CaptureClipboardRegion => "capture_clipboard_region",
            Self::CaptureFileRegion => "capture_file_region",
        }
    }

    fn get<'a>(&self, config: &'a KeybindingsConfig) -> &'a Vec<String> {
        match self {
            Self::Exit => &config.exit,
            Self::EnterTextMode => &config.enter_text_mode,
            Self::ClearCanvas => &config.clear_canvas,
            Self::Undo => &config.undo,
            Self::IncreaseThickness => &config.increase_thickness,
            Self::DecreaseThickness => &config.decrease_thickness,
            Self::SelectPenTool => &config.select_pen_tool,
            Self::SelectEraserTool => &config.select_eraser_tool,
            Self::SelectMarkerTool => &config.select_marker_tool,
            Self::IncreaseFontSize => &config.increase_font_size,
            Self::DecreaseFontSize => &config.decrease_font_size,
            Self::ToggleWhiteboard => &config.toggle_whiteboard,
            Self::ToggleBlackboard => &config.toggle_blackboard,
            Self::ReturnToTransparent => &config.return_to_transparent,
            Self::ToggleHelp => &config.toggle_help,
            Self::OpenConfigurator => &config.open_configurator,
            Self::SetColorRed => &config.set_color_red,
            Self::SetColorGreen => &config.set_color_green,
            Self::SetColorBlue => &config.set_color_blue,
            Self::SetColorYellow => &config.set_color_yellow,
            Self::SetColorOrange => &config.set_color_orange,
            Self::SetColorPink => &config.set_color_pink,
            Self::SetColorWhite => &config.set_color_white,
            Self::SetColorBlack => &config.set_color_black,
            Self::CaptureFullScreen => &config.capture_full_screen,
            Self::CaptureActiveWindow => &config.capture_active_window,
            Self::CaptureSelection => &config.capture_selection,
            Self::CaptureClipboardFull => &config.capture_clipboard_full,
            Self::CaptureFileFull => &config.capture_file_full,
            Self::CaptureClipboardSelection => &config.capture_clipboard_selection,
            Self::CaptureFileSelection => &config.capture_file_selection,
            Self::CaptureClipboardRegion => &config.capture_clipboard_region,
            Self::CaptureFileRegion => &config.capture_file_region,
        }
    }

    fn set(&self, config: &mut KeybindingsConfig, value: Vec<String>) {
        match self {
            Self::Exit => config.exit = value,
            Self::EnterTextMode => config.enter_text_mode = value,
            Self::ClearCanvas => config.clear_canvas = value,
            Self::Undo => config.undo = value,
            Self::IncreaseThickness => config.increase_thickness = value,
            Self::DecreaseThickness => config.decrease_thickness = value,
            Self::SelectPenTool => config.select_pen_tool = value,
            Self::SelectEraserTool => config.select_eraser_tool = value,
            Self::SelectMarkerTool => config.select_marker_tool = value,
            Self::IncreaseFontSize => config.increase_font_size = value,
            Self::DecreaseFontSize => config.decrease_font_size = value,
            Self::ToggleWhiteboard => config.toggle_whiteboard = value,
            Self::ToggleBlackboard => config.toggle_blackboard = value,
            Self::ReturnToTransparent => config.return_to_transparent = value,
            Self::ToggleHelp => config.toggle_help = value,
            Self::OpenConfigurator => config.open_configurator = value,
            Self::SetColorRed => config.set_color_red = value,
            Self::SetColorGreen => config.set_color_green = value,
            Self::SetColorBlue => config.set_color_blue = value,
            Self::SetColorYellow => config.set_color_yellow = value,
            Self::SetColorOrange => config.set_color_orange = value,
            Self::SetColorPink => config.set_color_pink = value,
            Self::SetColorWhite => config.set_color_white = value,
            Self::SetColorBlack => config.set_color_black = value,
            Self::CaptureFullScreen => config.capture_full_screen = value,
            Self::CaptureActiveWindow => config.capture_active_window = value,
            Self::CaptureSelection => config.capture_selection = value,
            Self::CaptureClipboardFull => config.capture_clipboard_full = value,
            Self::CaptureFileFull => config.capture_file_full = value,
            Self::CaptureClipboardSelection => config.capture_clipboard_selection = value,
            Self::CaptureFileSelection => config.capture_file_selection = value,
            Self::CaptureClipboardRegion => config.capture_clipboard_region = value,
            Self::CaptureFileRegion => config.capture_file_region = value,
        }
    }
}

pub fn parse_keybinding_list(value: &str) -> Result<Vec<String>, String> {
    let mut entries = Vec::new();

    for part in value.split(',') {
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            entries.push(trimmed.to_string());
        }
    }

    Ok(entries)
}
