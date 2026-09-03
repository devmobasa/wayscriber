//! Color picker popup state and methods.
//!
//! Provides a modal popup for selecting colors with a larger gradient picker
//! and editable hex input field.

mod panel;
mod state;

use crate::draw::Color;
use crate::input::Tool;

pub use panel::ColorPickerPopupPanel;

/// Width of the popup panel.
pub const POPUP_WIDTH: f64 = 300.0;
/// Height of the popup panel.
pub const POPUP_HEIGHT: f64 = 394.0;
/// Width of the saturation/value square and the hue bar below it.
pub const GRADIENT_WIDTH: f64 = 260.0;
/// Height of the saturation/value square.
pub const SV_HEIGHT: f64 = 150.0;
/// Height of the hue bar.
pub const HUE_HEIGHT: f64 = 14.0;
/// Height of the alpha bar.
pub const ALPHA_HEIGHT: f64 = 14.0;
/// Gap between the square and the bar beneath it.
pub const SLIDER_GAP: f64 = 8.0;
/// Edge length of a recent-color swatch.
pub const RECENT_SWATCH_SIZE: f64 = 24.0;
/// Gap between recent-color swatches.
pub const RECENT_SWATCH_GAP: f64 = 6.0;
/// Most recent colors the strip shows. Matches the recents cap so the strip
/// never has to elide entries.
pub const RECENT_SWATCH_COUNT: usize = 6;
/// Size of the preview swatch.
pub const PREVIEW_SIZE: f64 = 32.0;
/// Width of the hex input field.
pub const HEX_INPUT_WIDTH: f64 = 100.0;
/// Height of buttons (OK/Cancel).
pub const BUTTON_HEIGHT: f64 = 28.0;
/// Button width.
pub const BUTTON_WIDTH: f64 = 70.0;
/// Gap between the bottom-row buttons.
pub const BUTTON_GAP: f64 = 12.0;
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
    /// Load the slot's built-in color as the candidate. Only offered while
    /// recoloring a quick-color slot the shipped palette defines.
    RestoreDefault,
    Ok,
    Cancel,
}

/// Which picker area a pointer drag is currently steering.
///
/// The square and the bar sample different axes, so a drag that starts on one
/// must keep steering that one even after the pointer leaves its bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerDrag {
    /// The saturation (x) by value (y) square.
    SatVal,
    /// The hue bar.
    Hue,
    /// The alpha bar.
    Alpha,
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
        /// Quick-color slot being recolored, when the popup was opened by
        /// secondary-clicking a swatch. `None` edits the tool's own color.
        /// The slot is the edit target for both live preview and accept, so a
        /// recolor never hijacks what the tool is currently painting with.
        slot: Option<usize>,
        /// Original color when popup was opened (for cancel restoration). This
        /// is the edit target's color: the tool's, or the slot's when
        /// recoloring.
        original_color: Color,
        /// Currently selected color (live updates).
        current_color: Color,
        /// Whether the hex input field is focused for editing.
        hex_editing: bool,
        /// Text buffer for hex input field.
        hex_buffer: String,
        /// Which picker area a drag is steering, if any.
        dragging: Option<PickerDrag>,
        /// Hue/saturation/value the picker is showing.
        ///
        /// Kept alongside the RGB color because grey, black and white all
        /// collapse to a hue of zero: without this, dragging value down to
        /// black and back up would silently reset the hue to red.
        picker_hsv: (f64, f64, f64),
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
    /// X position of the saturation/value square.
    pub sv_x: f64,
    /// Y position of the saturation/value square.
    pub sv_y: f64,
    /// Width of the saturation/value square.
    pub sv_w: f64,
    /// Height of the saturation/value square.
    pub sv_h: f64,
    /// X position of the alpha bar.
    pub alpha_x: f64,
    /// Y position of the alpha bar.
    pub alpha_y: f64,
    /// Width of the alpha bar.
    pub alpha_w: f64,
    /// Height of the alpha bar.
    pub alpha_h: f64,
    /// Y position of the recent-color strip.
    pub recents_y: f64,
    /// X position of the recent-color strip's first swatch.
    pub recents_x: f64,
    /// X position of the hue bar.
    pub hue_x: f64,
    /// Y position of the hue bar.
    pub hue_y: f64,
    /// Width of the hue bar.
    pub hue_w: f64,
    /// Height of the hue bar.
    pub hue_h: f64,
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
    /// Top-left of the "Default" button, present only while recoloring a
    /// quick-color slot that the shipped palette defines. It shares the
    /// button row with OK/Cancel, which is why its absence has to change the
    /// row's centering rather than leave a hole.
    pub default_btn: Option<(f64, f64)>,
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
    /// Compute the layout for given screen dimensions. `show_default_button`
    /// comes from the popup's target: recoloring a slot with a built-in color
    /// adds a third button to the bottom row.
    pub fn compute(screen_width: u32, screen_height: u32, show_default_button: bool) -> Self {
        let width = POPUP_WIDTH;
        let height = POPUP_HEIGHT;

        // Center the popup on screen
        let origin_x = (screen_width as f64 - width) / 2.0;
        let origin_y = (screen_height as f64 - height) / 2.0;

        // Content starts after padding and title
        let content_x = origin_x + PADDING;
        let content_y = origin_y + PADDING + TITLE_HEIGHT;

        // Saturation/value square with the hue bar stacked under it, both
        // centered horizontally.
        let sv_x = origin_x + (width - GRADIENT_WIDTH) / 2.0;
        let sv_y = content_y;
        let hue_x = sv_x;
        let hue_y = sv_y + SV_HEIGHT + SLIDER_GAP;
        let alpha_x = sv_x;
        let alpha_y = hue_y + HUE_HEIGHT + SLIDER_GAP;

        // Preview row (preview swatch + hex input)
        let preview_row_y = alpha_y + ALPHA_HEIGHT + ELEMENT_GAP;
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

        // Recent-color strip sits between the action row and the buttons.
        let recents_y = preview_row_y + PREVIEW_SIZE + ELEMENT_GAP;
        let recents_width = RECENT_SWATCH_SIZE * RECENT_SWATCH_COUNT as f64
            + RECENT_SWATCH_GAP * (RECENT_SWATCH_COUNT as f64 - 1.0);
        let recents_x = origin_x + (width - recents_width) / 2.0;

        // Buttons at the bottom (centered as a group, so the optional
        // "Default" button widens the row instead of crowding one edge).
        let btn_row_y = origin_y + height - PADDING - BUTTON_HEIGHT;
        let btn_count = if show_default_button { 3.0 } else { 2.0 };
        let total_btn_width = BUTTON_WIDTH * btn_count + BUTTON_GAP * (btn_count - 1.0);
        let btn_start_x = origin_x + (width - total_btn_width) / 2.0;
        let default_btn = show_default_button.then_some((btn_start_x, btn_row_y));
        let primary_start_x = if show_default_button {
            btn_start_x + BUTTON_WIDTH + BUTTON_GAP
        } else {
            btn_start_x
        };
        let ok_btn_x = primary_start_x;
        let cancel_btn_x = primary_start_x + BUTTON_WIDTH + BUTTON_GAP;

        Self {
            origin_x,
            origin_y,
            width,
            height,
            sv_x,
            sv_y,
            sv_w: GRADIENT_WIDTH,
            sv_h: SV_HEIGHT,
            alpha_x,
            alpha_y,
            alpha_w: GRADIENT_WIDTH,
            alpha_h: ALPHA_HEIGHT,
            recents_y,
            recents_x,
            hue_x,
            hue_y,
            hue_w: GRADIENT_WIDTH,
            hue_h: HUE_HEIGHT,
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
            default_btn,
            ok_btn_x,
            ok_btn_y: btn_row_y,
            cancel_btn_x,
            cancel_btn_y: btn_row_y,
            btn_width: BUTTON_WIDTH,
            btn_height: BUTTON_HEIGHT,
        }
    }

    /// Check if a point is within the saturation/value square.
    pub fn point_in_sv(&self, x: f64, y: f64) -> bool {
        x >= self.sv_x && x <= self.sv_x + self.sv_w && y >= self.sv_y && y <= self.sv_y + self.sv_h
    }

    /// Check if a point is within the hue bar.
    pub fn point_in_hue(&self, x: f64, y: f64) -> bool {
        x >= self.hue_x
            && x <= self.hue_x + self.hue_w
            && y >= self.hue_y
            && y <= self.hue_y + self.hue_h
    }

    /// Saturation/value the square would yield for a pointer position.
    pub fn sv_from_point(&self, x: f64, y: f64) -> (f64, f64) {
        let saturation = ((x - self.sv_x) / self.sv_w).clamp(0.0, 1.0);
        let value = 1.0 - ((y - self.sv_y) / self.sv_h).clamp(0.0, 1.0);
        (saturation, value)
    }

    /// Hue the bar would yield for a pointer position.
    pub fn hue_from_point(&self, x: f64) -> f64 {
        ((x - self.hue_x) / self.hue_w).clamp(0.0, 1.0)
    }

    /// Check if a point is within the alpha bar.
    pub fn point_in_alpha(&self, x: f64, y: f64) -> bool {
        x >= self.alpha_x
            && x <= self.alpha_x + self.alpha_w
            && y >= self.alpha_y
            && y <= self.alpha_y + self.alpha_h
    }

    /// Alpha the bar would yield for a pointer position.
    pub fn alpha_from_point(&self, x: f64) -> f64 {
        ((x - self.alpha_x) / self.alpha_w).clamp(0.0, 1.0)
    }

    /// Top-left of the recent-color swatch at `index`.
    pub fn recent_swatch_origin(&self, index: usize) -> (f64, f64) {
        let x = self.recents_x + index as f64 * (RECENT_SWATCH_SIZE + RECENT_SWATCH_GAP);
        (x, self.recents_y)
    }

    /// Index of the recent-color swatch under a point, if any. `count` bounds
    /// the search so an empty tail is not clickable.
    pub fn recent_swatch_at(&self, x: f64, y: f64, count: usize) -> Option<usize> {
        if y < self.recents_y || y > self.recents_y + RECENT_SWATCH_SIZE {
            return None;
        }
        (0..count.min(RECENT_SWATCH_COUNT)).find(|index| {
            let (sx, _) = self.recent_swatch_origin(*index);
            x >= sx && x <= sx + RECENT_SWATCH_SIZE
        })
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

    /// Check if a point is within the "Default" button. Always false when the
    /// popup is not recoloring a slot with a built-in color.
    pub fn point_in_default_button(&self, x: f64, y: f64) -> bool {
        let Some((btn_x, btn_y)) = self.default_btn else {
            return false;
        };
        x >= btn_x && x <= btn_x + self.btn_width && y >= btn_y && y <= btn_y + self.btn_height
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
        } else if self.point_in_default_button(x, y) {
            Some(ColorPickerPopupAction::RestoreDefault)
        } else if self.point_in_ok_button(x, y) {
            Some(ColorPickerPopupAction::Ok)
        } else if self.point_in_cancel_button(x, y) {
            Some(ColorPickerPopupAction::Cancel)
        } else {
            None
        }
    }

    /// Tooltip for an icon-only action at the given point.
    pub(crate) fn action_tooltip_at(&self, x: f64, y: f64) -> Option<&'static str> {
        if self.point_in_copy_button(x, y) {
            Some("Copy hex color")
        } else if self.point_in_paste_button(x, y) {
            Some("Paste hex color from clipboard")
        } else if self.point_in_eyedropper_button(x, y) {
            Some("Pick color from screen")
        } else {
            None
        }
    }

    /// Tooltip text and a stable button-center anchor for an icon-only action.
    pub(crate) fn action_tooltip_anchor_at(
        &self,
        x: f64,
        y: f64,
    ) -> Option<(&'static str, f64, f64)> {
        let text = self.action_tooltip_at(x, y)?;
        let (button_x, button_y) = if self.point_in_copy_button(x, y) {
            (self.copy_btn_x, self.copy_btn_y)
        } else if self.point_in_paste_button(x, y) {
            (self.paste_btn_x, self.paste_btn_y)
        } else {
            (self.eyedropper_btn_x, self.eyedropper_btn_y)
        };
        Some((
            text,
            button_x + self.action_btn_size / 2.0,
            button_y + self.action_btn_size / 2.0,
        ))
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
    ///
    /// `recent_count` is the number of recent colors actually shown, the same
    /// count rendering and activation use. Passing the maximum instead would
    /// promise a clickable swatch over the empty positions of a fresh session.
    pub fn cursor_hint_at(&self, x: f64, y: f64, recent_count: usize) -> ColorPickerCursorHint {
        if self.point_in_hex_input(x, y) {
            ColorPickerCursorHint::Text
        } else if self.point_in_sv(x, y) || self.point_in_hue(x, y) || self.point_in_alpha(x, y) {
            ColorPickerCursorHint::Crosshair
        } else if self.point_in_ok_button(x, y)
            || self.point_in_cancel_button(x, y)
            || self.point_in_default_button(x, y)
            || self.point_in_copy_button(x, y)
            || self.point_in_paste_button(x, y)
            || self.point_in_eyedropper_button(x, y)
            || self.recent_swatch_at(x, y, recent_count).is_some()
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

/// Longest string [`color_to_hex`] can produce (`#RRGGBBAA`). Toolbar hex
/// fields size and length-limit themselves by this, so a translucent color's
/// eight-digit form is neither truncated on display nor rejected on input.
pub const HEX_INPUT_MAX_CHARS: usize = 9;

/// Convert a color to hex string (e.g., "#FF8040").
pub fn color_to_hex(color: Color) -> String {
    let alpha = (color.a.clamp(0.0, 1.0) * 255.0).round() as u8;
    let rgb = format!(
        "#{:02X}{:02X}{:02X}",
        (color.r * 255.0).round() as u8,
        (color.g * 255.0).round() as u8,
        (color.b * 255.0).round() as u8
    );
    // Only widen to eight digits when the alpha carries information, so an
    // opaque color still round-trips through the familiar six-digit form.
    if alpha == u8::MAX {
        rgb
    } else {
        format!("{rgb}{alpha:02X}")
    }
}

/// Parse a hex color string (e.g., "#FF8040" or "FF8040").
pub fn parse_hex_color(value: &str) -> Option<Color> {
    let mut hex = value.trim().trim_start_matches("0x");
    if hex.starts_with('#') {
        hex = &hex[1..];
    }
    if !matches!(hex.len(), 3 | 6 | 8) {
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
    let a = if expanded.len() == 8 {
        u8::from_str_radix(&expanded[6..8], 16).ok()?
    } else {
        u8::MAX
    };
    Some(Color {
        r: r as f64 / 255.0,
        g: g as f64 / 255.0,
        b: b as f64 / 255.0,
        a: a as f64 / 255.0,
    })
}
