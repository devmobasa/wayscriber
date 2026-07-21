//! Color picker popup state and methods.
//!
//! Provides a modal popup for selecting colors with a larger gradient picker
//! and editable hex input field.

mod state;

use crate::draw::Color;
use crate::input::Tool;

/// Width of the popup panel.
pub const POPUP_WIDTH: f64 = 300.0;
/// Height of the popup panel.
pub const POPUP_HEIGHT: f64 = 340.0;
/// Width of the gradient picker.
pub const GRADIENT_WIDTH: f64 = 260.0;
/// Height of the gradient picker.
pub const GRADIENT_HEIGHT: f64 = 180.0;
/// Size of the preview swatch.
pub const PREVIEW_SIZE: f64 = 32.0;
/// Width of the hex input field.
pub const HEX_INPUT_WIDTH: f64 = 100.0;
/// Height of buttons (OK/Cancel).
pub const BUTTON_HEIGHT: f64 = 28.0;
/// Button width.
pub const BUTTON_WIDTH: f64 = 70.0;
/// Padding inside the popup.
pub const PADDING: f64 = 20.0;
/// Title bar height.
pub const TITLE_HEIGHT: f64 = 24.0;
/// Gap between elements.
pub const ELEMENT_GAP: f64 = 12.0;
/// Gap between the hex input and the trailing action-button cluster
/// (copy / paste / eyedropper).
pub const ACTION_ROW_GAP: f64 = 8.0;
/// Gap between adjacent action buttons in the trailing cluster.
pub const ACTION_BTN_GAP: f64 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorPickerPopupAction {
    Copy,
    Paste,
    Eyedropper,
    Ok,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HexPasteTarget {
    ActiveTool,
    ColorPickerPopup { generation: u64 },
}

/// State of the color picker popup.
#[derive(Debug, Clone, Default)]
pub enum ColorPickerPopupState {
    /// Popup is not visible.
    #[default]
    Hidden,
    /// Popup is open with current editing state.
    Open {
        /// Tool whose color is being edited.
        tool: Tool,
        /// Original color when popup was opened (for cancel restoration).
        original_color: Color,
        /// Currently selected color (live updates).
        current_color: Color,
        /// Whether the hex input field is focused for editing.
        hex_editing: bool,
        /// Text buffer for hex input field.
        hex_buffer: String,
        /// Whether we're currently dragging on the gradient.
        dragging: bool,
        /// Whether the hex text is selected (first keystroke replaces all).
        hex_selected: bool,
        /// Current hover position (for button hover states).
        hover_pos: Option<(f64, f64)>,
    },
}

/// Cached layout metrics for the color picker popup.
#[derive(Debug, Clone, Copy)]
pub struct ColorPickerPopupLayout {
    /// Top-left X of the popup panel.
    pub origin_x: f64,
    /// Top-left Y of the popup panel.
    pub origin_y: f64,
    /// Width of the popup panel.
    pub width: f64,
    /// Height of the popup panel.
    pub height: f64,
    /// X position of the gradient picker.
    pub gradient_x: f64,
    /// Y position of the gradient picker.
    pub gradient_y: f64,
    /// Width of the gradient picker.
    pub gradient_w: f64,
    /// Height of the gradient picker.
    pub gradient_h: f64,
    /// X position of the preview swatch.
    pub preview_x: f64,
    /// Y position of the preview swatch.
    pub preview_y: f64,
    /// X position of the hex input.
    pub hex_input_x: f64,
    /// Y position of the hex input.
    pub hex_input_y: f64,
    /// Width of the hex input.
    pub hex_input_w: f64,
    /// Height of the hex input.
    pub hex_input_h: f64,
    /// X position of the copy-hex button.
    pub copy_btn_x: f64,
    /// Y position of the copy-hex button.
    pub copy_btn_y: f64,
    /// X position of the paste-hex button.
    pub paste_btn_x: f64,
    /// Y position of the paste-hex button.
    pub paste_btn_y: f64,
    /// X position of the screen eyedropper button.
    pub eyedropper_btn_x: f64,
    /// Y position of the screen eyedropper button.
    pub eyedropper_btn_y: f64,
    /// Size of the square action buttons (copy / paste / eyedropper).
    pub action_btn_size: f64,
    /// X position of the OK button.
    pub ok_btn_x: f64,
    /// Y position of the OK button.
    pub ok_btn_y: f64,
    /// X position of the Cancel button.
    pub cancel_btn_x: f64,
    /// Y position of the Cancel button.
    pub cancel_btn_y: f64,
    /// Button width.
    pub btn_width: f64,
    /// Button height.
    pub btn_height: f64,
}

impl ColorPickerPopupLayout {
    /// Compute the layout for given screen dimensions.
    pub fn compute(screen_width: u32, screen_height: u32) -> Self {
        let width = POPUP_WIDTH;
        let height = POPUP_HEIGHT;

        // Center the popup on screen
        let origin_x = (screen_width as f64 - width) / 2.0;
        let origin_y = (screen_height as f64 - height) / 2.0;

        // Content starts after padding and title
        let content_x = origin_x + PADDING;
        let content_y = origin_y + PADDING + TITLE_HEIGHT;

        // Gradient picker (centered horizontally)
        let gradient_x = origin_x + (width - GRADIENT_WIDTH) / 2.0;
        let gradient_y = content_y;

        // Preview row (preview swatch + hex input)
        let preview_row_y = gradient_y + GRADIENT_HEIGHT + ELEMENT_GAP;
        let preview_x = content_x;
        let preview_y = preview_row_y;

        // Hex input (to the right of preview)
        let hex_input_x = preview_x + PREVIEW_SIZE + 12.0;
        let hex_input_y = preview_row_y + (PREVIEW_SIZE - 24.0) / 2.0;
        let hex_input_w = HEX_INPUT_WIDTH;
        let hex_input_h = 24.0;

        // Trailing action-button cluster: copy, paste, eyedropper. All three
        // share the preview swatch's size and sit on the preview row; the
        // cluster ends flush with the content's right edge.
        let action_btn_size = PREVIEW_SIZE;
        let copy_btn_x = hex_input_x + hex_input_w + ACTION_ROW_GAP;
        let paste_btn_x = copy_btn_x + action_btn_size + ACTION_BTN_GAP;
        let eyedropper_btn_x = paste_btn_x + action_btn_size + ACTION_BTN_GAP;
        let copy_btn_y = preview_row_y;
        let paste_btn_y = preview_row_y;
        let eyedropper_btn_y = preview_row_y;

        // Buttons at the bottom (centered)
        let btn_row_y = origin_y + height - PADDING - BUTTON_HEIGHT;
        let total_btn_width = BUTTON_WIDTH * 2.0 + 12.0;
        let btn_start_x = origin_x + (width - total_btn_width) / 2.0;
        let ok_btn_x = btn_start_x;
        let cancel_btn_x = btn_start_x + BUTTON_WIDTH + 12.0;

        Self {
            origin_x,
            origin_y,
            width,
            height,
            gradient_x,
            gradient_y,
            gradient_w: GRADIENT_WIDTH,
            gradient_h: GRADIENT_HEIGHT,
            preview_x,
            preview_y,
            hex_input_x,
            hex_input_y,
            hex_input_w,
            hex_input_h,
            copy_btn_x,
            copy_btn_y,
            paste_btn_x,
            paste_btn_y,
            eyedropper_btn_x,
            eyedropper_btn_y,
            action_btn_size,
            ok_btn_x,
            ok_btn_y: btn_row_y,
            cancel_btn_x,
            cancel_btn_y: btn_row_y,
            btn_width: BUTTON_WIDTH,
            btn_height: BUTTON_HEIGHT,
        }
    }

    /// Check if a point is within the gradient picker.
    pub fn point_in_gradient(&self, x: f64, y: f64) -> bool {
        x >= self.gradient_x
            && x <= self.gradient_x + self.gradient_w
            && y >= self.gradient_y
            && y <= self.gradient_y + self.gradient_h
    }

    /// Check if a point is within the hex input field.
    pub fn point_in_hex_input(&self, x: f64, y: f64) -> bool {
        x >= self.hex_input_x
            && x <= self.hex_input_x + self.hex_input_w
            && y >= self.hex_input_y
            && y <= self.hex_input_y + self.hex_input_h
    }

    /// Check if a point is within the OK button.
    pub fn point_in_ok_button(&self, x: f64, y: f64) -> bool {
        x >= self.ok_btn_x
            && x <= self.ok_btn_x + self.btn_width
            && y >= self.ok_btn_y
            && y <= self.ok_btn_y + self.btn_height
    }

    /// Check if a point is within the copy-hex button.
    pub fn point_in_copy_button(&self, x: f64, y: f64) -> bool {
        x >= self.copy_btn_x
            && x <= self.copy_btn_x + self.action_btn_size
            && y >= self.copy_btn_y
            && y <= self.copy_btn_y + self.action_btn_size
    }

    /// Check if a point is within the paste-hex button.
    pub fn point_in_paste_button(&self, x: f64, y: f64) -> bool {
        x >= self.paste_btn_x
            && x <= self.paste_btn_x + self.action_btn_size
            && y >= self.paste_btn_y
            && y <= self.paste_btn_y + self.action_btn_size
    }

    /// Check if a point is within the screen eyedropper button.
    pub fn point_in_eyedropper_button(&self, x: f64, y: f64) -> bool {
        x >= self.eyedropper_btn_x
            && x <= self.eyedropper_btn_x + self.action_btn_size
            && y >= self.eyedropper_btn_y
            && y <= self.eyedropper_btn_y + self.action_btn_size
    }

    /// Check if a point is within the Cancel button.
    pub fn point_in_cancel_button(&self, x: f64, y: f64) -> bool {
        x >= self.cancel_btn_x
            && x <= self.cancel_btn_x + self.btn_width
            && y >= self.cancel_btn_y
            && y <= self.cancel_btn_y + self.btn_height
    }

    pub(crate) fn action_at(&self, x: f64, y: f64) -> Option<ColorPickerPopupAction> {
        if self.point_in_copy_button(x, y) {
            Some(ColorPickerPopupAction::Copy)
        } else if self.point_in_paste_button(x, y) {
            Some(ColorPickerPopupAction::Paste)
        } else if self.point_in_eyedropper_button(x, y) {
            Some(ColorPickerPopupAction::Eyedropper)
        } else if self.point_in_ok_button(x, y) {
            Some(ColorPickerPopupAction::Ok)
        } else if self.point_in_cancel_button(x, y) {
            Some(ColorPickerPopupAction::Cancel)
        } else {
            None
        }
    }

    /// Check if a point is within the popup panel.
    pub fn point_in_panel(&self, x: f64, y: f64) -> bool {
        x >= self.origin_x
            && x <= self.origin_x + self.width
            && y >= self.origin_y
            && y <= self.origin_y + self.height
    }

    /// Determine the cursor type for a given point within the popup.
    /// Returns the cursor hint for different UI regions.
    pub fn cursor_hint_at(&self, x: f64, y: f64) -> ColorPickerCursorHint {
        if self.point_in_hex_input(x, y) {
            ColorPickerCursorHint::Text
        } else if self.point_in_gradient(x, y) {
            ColorPickerCursorHint::Crosshair
        } else if self.point_in_ok_button(x, y)
            || self.point_in_cancel_button(x, y)
            || self.point_in_copy_button(x, y)
            || self.point_in_paste_button(x, y)
            || self.point_in_eyedropper_button(x, y)
        {
            ColorPickerCursorHint::Pointer
        } else {
            ColorPickerCursorHint::Default
        }
    }
}

/// Cursor hint for different regions of the color picker popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPickerCursorHint {
    /// Default arrow cursor.
    Default,
    /// Text editing cursor (I-beam).
    Text,
    /// Crosshair for color selection.
    Crosshair,
    /// Pointer/hand cursor for buttons.
    Pointer,
}

pub use crate::draw::color::{hsv_to_rgb, rgb_to_hsv};

/// Convert a color to hex string (e.g., "#FF8040").
pub fn color_to_hex(color: Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (color.r * 255.0).round() as u8,
        (color.g * 255.0).round() as u8,
        (color.b * 255.0).round() as u8
    )
}

/// Parse a hex color string (e.g., "#FF8040" or "FF8040").
pub fn parse_hex_color(value: &str) -> Option<Color> {
    let mut hex = value.trim().trim_start_matches("0x");
    if hex.starts_with('#') {
        hex = &hex[1..];
    }
    if hex.len() != 6 && hex.len() != 3 {
        return None;
    }
    if !hex.as_bytes().iter().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let expanded = if hex.len() == 3 {
        let mut out = String::new();
        for ch in hex.chars() {
            out.push(ch);
            out.push(ch);
        }
        out
    } else {
        hex.to_string()
    };
    let r = u8::from_str_radix(&expanded[0..2], 16).ok()?;
    let g = u8::from_str_radix(&expanded[2..4], 16).ok()?;
    let b = u8::from_str_radix(&expanded[4..6], 16).ok()?;
    Some(Color {
        r: r as f64 / 255.0,
        g: g as f64 / 255.0,
        b: b as f64 / 255.0,
        a: 1.0,
    })
}
