/// UI rendering: status bar, help overlay, visual indicators
use crate::config::StatusPosition;
use crate::input::{BoardMode, DrawingState, InputState, Tool};

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

/// Fallback character width for monospace font estimation
const HELP_CHAR_WIDTH_ESTIMATE: f64 = 9.0;

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
    let thickness = input_state.current_thickness;
    let tool = input_state.modifiers.current_tool();

    // Determine tool name
    let tool_name = match &input_state.state {
        DrawingState::TextInput { .. } => "Text",
        DrawingState::Drawing { tool, .. } => match tool {
            Tool::Pen => "Pen",
            Tool::Line => "Line",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Circle",
            Tool::Arrow => "Arrow",
        },
        DrawingState::Idle => match tool {
            Tool::Pen => "Pen",
            Tool::Line => "Line",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Circle",
            Tool::Arrow => "Arrow",
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

    // Build status text with mode badge and font size
    let font_size = input_state.current_font_size;
    let status_text = format!(
        "{}[{}] [{}px] [{}] [Text {}px]  F10=Help",
        mode_badge, color_name, thickness as i32, tool_name, font_size as i32
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
    ctx.arc(
        dot_x,
        dot_y,
        style.dot_radius,
        0.0,
        2.0 * std::f64::consts::PI,
    );
    let _ = ctx.fill();

    // Draw text with adaptive color
    let [r, g, b, a] = text_color;
    ctx.set_source_rgba(r, g, b, a);
    ctx.move_to(x, y);
    let _ = ctx.show_text(&status_text);
}

/// Render help overlay showing all keybindings
pub fn render_help_overlay(
    ctx: &cairo::Context,
    style: &crate::config::HelpOverlayStyle,
    screen_width: u32,
    screen_height: u32,
) {
    let help_text = vec![
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ HYPRMARKER CONTROLS ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
        "",
        "  BOARD MODES                              DRAWING TOOLS",
        "    Ctrl+W          Toggle Whiteboard        Drag              Freehand pen",
        "    Ctrl+B          Toggle Blackboard        Shift+Drag        Straight line",
        "    Ctrl+Shift+T    Return to Transparent    Ctrl+Drag         Rectangle",
        "                                             Tab+Drag          Circle",
        "  PEN & TEXT                                 Ctrl+Shift+Drag   Arrow",
        "    +/- = Scroll    Pen thickness            T                 Text mode",
        "    Ctrl+Shift+/-   Font size",
        "    Shift+Scroll    Font size                COLORS:  R G B Y O P W K",
        "",
        "  SCREENSHOTS                              ACTIONS",
        "    F9              Full screen (save+clip)  E                 Clear frame",
        "    Ctrl+F9         Active window            Ctrl+Z            Undo",
        "                                             Escape/Ctrl+Q     Exit",
        "                                             F10               Toggle help",
        "",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
        "  Note: Each board mode has independent drawings",
    ];

    // Set font
    ctx.set_font_size(style.font_size);
    ctx.select_font_face(
        "Monospace",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );

    // Find longest line for width
    let mut max_width: f64 = 0.0;
    for line in &help_text {
        let extents = match ctx.text_extents(line) {
            Ok(ext) => ext,
            Err(e) => {
                log::warn!(
                    "Failed to measure help text line '{}': {}, using fallback width",
                    line,
                    e
                );
                // Use a fallback width estimate based on character count
                let fallback_width = line.len() as f64 * HELP_CHAR_WIDTH_ESTIMATE;
                max_width = max_width.max(fallback_width);
                continue;
            }
        };
        if extents.width() > max_width {
            max_width = extents.width();
        }
    }

    let box_width = max_width + style.padding * 2.0;
    let box_height = (help_text.len() as f64) * style.line_height + style.padding * 2.0;

    // Center the box
    let box_x = (screen_width as f64 - box_width) / 2.0;
    let box_y = (screen_height as f64 - box_height) / 2.0;

    // Draw semi-transparent background
    let [r, g, b, a] = style.bg_color;
    ctx.set_source_rgba(r, g, b, a);
    ctx.rectangle(box_x, box_y, box_width, box_height);
    let _ = ctx.fill();

    // Draw border
    let [r, g, b, a] = style.border_color;
    ctx.set_source_rgba(r, g, b, a);
    ctx.set_line_width(style.border_width);
    ctx.rectangle(box_x, box_y, box_width, box_height);
    let _ = ctx.stroke();

    // Draw text
    let [r, g, b, a] = style.text_color;
    ctx.set_source_rgba(r, g, b, a);
    for (i, line) in help_text.iter().enumerate() {
        let text_x = box_x + style.padding;
        let text_y = box_y + style.padding + (i as f64 + 1.0) * style.line_height;

        ctx.move_to(text_x, text_y);
        let _ = ctx.show_text(line);
    }
}
