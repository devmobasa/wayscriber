use crate::config::StatusPosition;
use crate::input::{BoardMode, DrawingState, InputState, TextInputMode, Tool};
use std::f64::consts::PI;

use super::primitives::{draw_rounded_rect, fallback_text_extents};

// ============================================================================
// UI Layout Constants (not configurable)
// ============================================================================

/// Background rectangle X offset
const STATUS_BG_OFFSET_X: f64 = 5.0;
/// Background rectangle Y offset
const STATUS_BG_OFFSET_Y: f64 = 3.0;
/// Background rectangle width padding
const STATUS_BG_WIDTH_PAD: f64 = 10.0;
/// Background rectangle height padding
const STATUS_BG_HEIGHT_PAD: f64 = 8.0;
/// Color indicator dot X offset
const STATUS_DOT_OFFSET_X: f64 = 3.0;

/// Render status bar showing current color, thickness, and tool
pub fn render_status_bar(
    ctx: &cairo::Context,
    input_state: &InputState,
    position: StatusPosition,
    style: &crate::config::StatusBarStyle,
    screen_width: u32,
    screen_height: u32,
) {
    let color = &input_state.current_color;
    let tool = input_state.active_tool();
    let thickness = if tool == Tool::Eraser {
        input_state.eraser_size
    } else {
        input_state.current_thickness
    };

    // Determine tool name
    let tool_name = match &input_state.state {
        DrawingState::TextInput { .. } => match input_state.text_input_mode {
            TextInputMode::Plain => "Text",
            TextInputMode::StickyNote => "Sticky Note",
        },
        DrawingState::Drawing { tool, .. } => match tool {
            Tool::Select => "Select",
            Tool::Pen => "Pen",
            Tool::Line => "Line",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Circle",
            Tool::Arrow => "Arrow",
            Tool::Marker => "Marker",
            Tool::Highlight => "Highlight",
            Tool::Eraser => "Eraser",
        },
        DrawingState::MovingSelection { .. } => "Move",
        DrawingState::Selecting { .. } => "Select",
        DrawingState::ResizingText { .. } => "Resize",
        DrawingState::PendingTextClick { .. } | DrawingState::Idle => match tool {
            Tool::Select => "Select",
            Tool::Pen => "Pen",
            Tool::Line => "Line",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Circle",
            Tool::Arrow => "Arrow",
            Tool::Marker => "Marker",
            Tool::Highlight => "Highlight",
            Tool::Eraser => "Eraser",
        },
    };

    // Determine color name
    let color_name = crate::util::color_to_name(color);

    // Get board mode indicator
    let mode_badge = match input_state.board_mode() {
        BoardMode::Transparent => "",
        BoardMode::Whiteboard => "[WHITEBOARD] ",
        BoardMode::Blackboard => "[BLACKBOARD] ",
    };
    let page_count = input_state
        .canvas_set
        .page_count(input_state.board_mode())
        .max(1);
    let page_index = input_state
        .canvas_set
        .active_page_index(input_state.board_mode());
    let page_badge = format!("[Page {}/{}] ", page_index + 1, page_count);

    // Build status text with mode badge and font size
    let font_size = input_state.current_font_size;
    let highlight_badge = if input_state.click_highlight_enabled() {
        " [Click HL]"
    } else {
        ""
    };
    let highlight_tool_badge = if input_state.highlight_tool_active() {
        " [Highlight pen]"
    } else {
        ""
    };

    let frozen_badge = if input_state.frozen_active() {
        "[FROZEN] "
    } else {
        ""
    };
    let zoom_badge = if input_state.zoom_active() {
        let pct = (input_state.zoom_scale() * 100.0).round() as i32;
        if input_state.zoom_locked() {
            format!("[ZOOM {}% LOCKED] ", pct)
        } else {
            format!("[ZOOM {}%] ", pct)
        }
    } else {
        String::new()
    };

    let status_text = format!(
        "{}{}{}{}[{}] [{}px] [{}] [Text {}px]{}{}  F1=Help",
        frozen_badge,
        zoom_badge,
        mode_badge,
        page_badge,
        color_name,
        thickness as i32,
        tool_name,
        font_size as i32,
        highlight_badge,
        highlight_tool_badge
    );

    // Set font
    log::debug!("Status bar font_size from config: {}", style.font_size);
    ctx.set_font_size(style.font_size);
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);

    // Measure text
    let extents = match ctx.text_extents(&status_text) {
        Ok(ext) => ext,
        Err(e) => {
            log::warn!(
                "Failed to measure status bar text: {}, skipping status bar",
                e
            );
            return; // Gracefully skip rendering if font measurement fails
        }
    };
    let text_width = extents.width();
    let text_height = extents.height();

    // Calculate position using configurable padding
    let padding = style.padding;
    let (x, y) = match position {
        StatusPosition::TopLeft => (padding, padding + text_height),
        StatusPosition::TopRight => (
            screen_width as f64 - text_width - padding,
            padding + text_height,
        ),
        StatusPosition::BottomLeft => (padding, screen_height as f64 - padding),
        StatusPosition::BottomRight => (
            screen_width as f64 - text_width - padding,
            screen_height as f64 - padding,
        ),
    };

    // Adjust colors based on board mode for better contrast
    let (bg_color, text_color) = match input_state.board_mode() {
        BoardMode::Transparent => {
            // Use config colors for transparent mode
            (style.bg_color, style.text_color)
        }
        BoardMode::Whiteboard => {
            // Dark text and background on white board
            ([0.2, 0.2, 0.2, 0.85], [0.0, 0.0, 0.0, 1.0])
        }
        BoardMode::Blackboard => {
            // Light text and background on dark board
            ([0.8, 0.8, 0.8, 0.85], [1.0, 1.0, 1.0, 1.0])
        }
    };

    // Draw semi-transparent background with adaptive color
    let [r, g, b, a] = bg_color;
    ctx.set_source_rgba(r, g, b, a);
    ctx.rectangle(
        x - STATUS_BG_OFFSET_X,
        y - text_height - STATUS_BG_OFFSET_Y,
        text_width + STATUS_BG_WIDTH_PAD,
        text_height + STATUS_BG_HEIGHT_PAD,
    );
    let _ = ctx.fill();

    // Draw color indicator dot
    let dot_x = x + STATUS_DOT_OFFSET_X;
    let dot_y = y - text_height / 2.0;
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.arc(dot_x, dot_y, style.dot_radius, 0.0, 2.0 * PI);
    let _ = ctx.fill();

    // Draw text with adaptive color
    let [r, g, b, a] = text_color;
    ctx.set_source_rgba(r, g, b, a);
    ctx.move_to(x, y);
    let _ = ctx.show_text(&status_text);
}

/// Render a small badge indicating frozen mode (visible even when status bar is hidden).
pub fn render_frozen_badge(ctx: &cairo::Context, screen_width: u32, _screen_height: u32) {
    let label = "FROZEN";
    let padding = 12.0;
    let radius = 8.0;
    let font_size = 16.0;

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(font_size);

    let extents = ctx
        .text_extents(label)
        .unwrap_or_else(|_| fallback_text_extents(font_size, label));

    let width = extents.width() + padding * 1.4;
    let height = extents.height() + padding;

    let x = screen_width as f64 - width - padding;
    let y = padding + height;

    // Background with warning tint
    ctx.set_source_rgba(0.82, 0.22, 0.2, 0.9);
    draw_rounded_rect(ctx, x, y - height, width, height, radius);
    let _ = ctx.fill();

    // Text
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    ctx.move_to(x + (padding * 0.7), y - (padding * 0.35));
    let _ = ctx.show_text(label);
}

/// Render a small badge indicating zoom mode (visible even when status bar is hidden).
pub fn render_zoom_badge(
    ctx: &cairo::Context,
    screen_width: u32,
    _screen_height: u32,
    zoom_scale: f64,
    locked: bool,
) {
    let zoom_pct = (zoom_scale * 100.0).round() as i32;
    let label = if locked {
        format!("ZOOM {}% LOCKED", zoom_pct)
    } else {
        format!("ZOOM {}%", zoom_pct)
    };
    let padding = 12.0;
    let radius = 8.0;
    let font_size = 15.0;

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(font_size);

    let extents = ctx
        .text_extents(&label)
        .unwrap_or_else(|_| fallback_text_extents(font_size, &label));

    let width = extents.width() + padding * 1.4;
    let height = extents.height() + padding;

    let x = screen_width as f64 - width - padding;
    let y = padding + height;

    // Background with teal tint
    ctx.set_source_rgba(0.2, 0.52, 0.7, 0.9);
    draw_rounded_rect(ctx, x, y - height, width, height, radius);
    let _ = ctx.fill();

    // Text
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    ctx.move_to(x + (padding * 0.7), y - (padding * 0.35));
    let _ = ctx.show_text(&label);
}

/// Render a small badge indicating the current page (visible even when status bar is hidden).
pub fn render_page_badge(
    ctx: &cairo::Context,
    _screen_width: u32,
    _screen_height: u32,
    page_index: usize,
    page_count: usize,
) {
    let label = format!("Page {}/{}", page_index + 1, page_count.max(1));
    let padding = 12.0;
    let radius = 8.0;
    let font_size = 15.0;

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(font_size);

    let extents = ctx
        .text_extents(&label)
        .unwrap_or_else(|_| fallback_text_extents(font_size, &label));

    let width = extents.width() + padding * 1.4;
    let height = extents.height() + padding;

    let x = padding;
    let y = padding + height;

    // Background with a neutral cool tone.
    ctx.set_source_rgba(0.2, 0.32, 0.45, 0.92);
    draw_rounded_rect(ctx, x, y - height, width, height, radius);
    let _ = ctx.fill();

    // Text
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    ctx.move_to(x + (padding * 0.7), y - (padding * 0.35));
    let _ = ctx.show_text(&label);
}
