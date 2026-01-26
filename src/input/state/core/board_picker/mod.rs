#![allow(dead_code)]

mod layout;
mod search;
mod state;

use std::time::Duration;

use crate::draw::{BLACK, BLUE, Color, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};

const TITLE_FONT_SIZE: f64 = 17.0;
const BODY_FONT_SIZE: f64 = 14.0;
const FOOTER_FONT_SIZE: f64 = 12.0;
const ROW_HEIGHT: f64 = 32.0;
const HEADER_HEIGHT: f64 = 28.0;
const FOOTER_HEIGHT: f64 = 22.0;
const PADDING_X: f64 = 16.0;
const PADDING_Y: f64 = 14.0;
const SWATCH_SIZE: f64 = 14.0;
const SWATCH_PADDING: f64 = 10.0;

const COMPACT_TITLE_FONT_SIZE: f64 = 15.0;
const COMPACT_BODY_FONT_SIZE: f64 = 13.0;
const COMPACT_FOOTER_FONT_SIZE: f64 = 11.0;
const COMPACT_ROW_HEIGHT: f64 = 26.0;
const COMPACT_HEADER_HEIGHT: f64 = 22.0;
const COMPACT_FOOTER_HEIGHT: f64 = 18.0;
const COMPACT_PADDING_X: f64 = 12.0;
const COMPACT_PADDING_Y: f64 = 10.0;
const COMPACT_SWATCH_SIZE: f64 = 12.0;
const COMPACT_SWATCH_PADDING: f64 = 8.0;
const HANDLE_WIDTH: f64 = 10.0;
const HANDLE_GAP: f64 = 18.0;
const PIN_OFFSET_FACTOR: f64 = 0.6;
const COLUMN_GAP: f64 = 12.0;
const MAX_BOARD_NAME_LEN: usize = 40;
const PALETTE_SWATCH_SIZE: f64 = 18.0;
const PALETTE_SWATCH_GAP: f64 = 6.0;
const PALETTE_TOP_GAP: f64 = 8.0;
const PALETTE_BOTTOM_GAP: f64 = 8.0;
const BOARD_PICKER_SEARCH_TIMEOUT: Duration = Duration::from_millis(1200);
const BOARD_PICKER_SEARCH_MAX_LEN: usize = 24;
const BOARD_PICKER_RECENT_LINE_HEIGHT: f64 = 16.0;
const BOARD_PICKER_RECENT_LINE_HEIGHT_COMPACT: f64 = 14.0;
const BOARD_PICKER_RECENT_MAX_NAMES: usize = 3;
const BOARD_PICKER_RECENT_LABEL_MAX_CHARS: usize = BOARD_PICKER_SEARCH_MAX_LEN + 6;

#[derive(Debug, Clone)]
pub enum BoardPickerState {
    Hidden,
    Open {
        selected: usize,
        hover_index: Option<usize>,
        edit: Option<BoardPickerEdit>,
        mode: BoardPickerMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardPickerMode {
    Full,
    Quick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardPickerEditMode {
    Name,
    Color,
}

#[derive(Debug, Clone)]
pub struct BoardPickerEdit {
    pub mode: BoardPickerEditMode,
    pub buffer: String,
}

#[derive(Debug, Clone, Copy)]
pub struct BoardPickerDrag {
    pub source_row: usize,
    pub source_board: usize,
    pub current_row: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct BoardPickerLayout {
    pub origin_x: f64,
    pub origin_y: f64,
    pub width: f64,
    pub height: f64,
    pub title_font_size: f64,
    pub body_font_size: f64,
    pub footer_font_size: f64,
    pub row_height: f64,
    pub header_height: f64,
    pub footer_height: f64,
    pub padding_x: f64,
    pub padding_y: f64,
    pub swatch_size: f64,
    pub swatch_padding: f64,
    pub hint_width: f64,
    pub row_count: usize,
    pub palette_top: f64,
    pub palette_rows: usize,
    pub palette_cols: usize,
    pub recent_height: f64,
    pub handle_width: f64,
    pub handle_gap: f64,
}

fn truncate_search_label(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let mut hex = value.trim().trim_start_matches("0x");
    if hex.starts_with('#') {
        hex = &hex[1..];
    }
    if hex.len() != 6 && hex.len() != 3 {
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

fn color_to_hex(color: Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (color.r * 255.0).round() as u8,
        (color.g * 255.0).round() as u8,
        (color.b * 255.0).round() as u8
    )
}

fn contrast_color(background: Color) -> Color {
    let luminance = 0.2126 * background.r + 0.7152 * background.g + 0.0722 * background.b;
    if luminance > 0.5 {
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }
    } else {
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    }
}

const BOARD_PALETTE: [Color; 11] = [
    RED,
    GREEN,
    BLUE,
    YELLOW,
    WHITE,
    BLACK,
    ORANGE,
    PINK,
    Color {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    },
    Color {
        r: 0.6,
        g: 0.4,
        b: 0.8,
        a: 1.0,
    },
    Color {
        r: 0.4,
        g: 0.4,
        b: 0.4,
        a: 1.0,
    },
];

fn board_palette_colors() -> &'static [Color] {
    &BOARD_PALETTE
}

/// Cursor hint for different regions of the board picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardPickerCursorHint {
    /// Default arrow cursor.
    Default,
    /// Pointer/hand cursor for clickable items.
    Pointer,
    /// Grab cursor for drag handles.
    Grab,
    /// Grabbing cursor when actively dragging.
    Grabbing,
    /// Text editing cursor (I-beam) for name/hex editing.
    Text,
}
