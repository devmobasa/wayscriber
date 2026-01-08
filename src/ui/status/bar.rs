use std::f64::consts::PI;

use crate::config::{Action, StatusPosition, action_display_label};
use crate::input::{BoardMode, DrawingState, InputState, TextInputMode, Tool};
use crate::label_format::format_binding_labels;
use crate::ui::toolbar::bindings::action_for_tool;

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

    let tool_name = tool_display_name(input_state, tool);
    let color_name = crate::util::color_to_name(color);

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

    let font_size = input_state.current_font_size;
    let highlight_badge = if input_state.click_highlight_enabled() {
        format!(" [{}]", action_display_label(Action::ToggleClickHighlight))
    } else {
        String::new()
    };
    let highlight_tool_badge = if input_state.highlight_tool_active() {
        format!(" [{}]", action_display_label(Action::SelectHighlightTool))
    } else {
        String::new()
    };
    let help_binding =
        format_binding_labels(&input_state.action_binding_labels(Action::ToggleHelp));

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
        "{}{}{}{}[{}] [{}px] [{}] [Text {}px]{}{}  {}={}",
        frozen_badge,
        zoom_badge,
        mode_badge,
        page_badge,
        color_name,
        thickness as i32,
        tool_name,
        font_size as i32,
        highlight_badge,
        highlight_tool_badge,
        help_binding,
        action_display_label(Action::ToggleHelp)
    );

    log::debug!("Status bar font_size from config: {}", style.font_size);
    ctx.set_font_size(style.font_size);
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);

    let extents = match ctx.text_extents(&status_text) {
        Ok(ext) => ext,
        Err(e) => {
            log::warn!(
                "Failed to measure status bar text: {}, skipping status bar",
                e
            );
            return;
        }
    };
    let text_width = extents.width();
    let text_height = extents.height();

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

    let (bg_color, text_color) = match input_state.board_mode() {
        BoardMode::Transparent => (style.bg_color, style.text_color),
        BoardMode::Whiteboard => ([0.2, 0.2, 0.2, 0.85], [0.0, 0.0, 0.0, 1.0]),
        BoardMode::Blackboard => ([0.8, 0.8, 0.8, 0.85], [1.0, 1.0, 1.0, 1.0]),
    };

    let [r, g, b, a] = bg_color;
    ctx.set_source_rgba(r, g, b, a);
    ctx.rectangle(
        x - STATUS_BG_OFFSET_X,
        y - text_height - STATUS_BG_OFFSET_Y,
        text_width + STATUS_BG_WIDTH_PAD,
        text_height + STATUS_BG_HEIGHT_PAD,
    );
    let _ = ctx.fill();

    let dot_x = x + STATUS_DOT_OFFSET_X;
    let dot_y = y - text_height / 2.0;
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.arc(dot_x, dot_y, style.dot_radius, 0.0, 2.0 * PI);
    let _ = ctx.fill();

    let [r, g, b, a] = text_color;
    ctx.set_source_rgba(r, g, b, a);
    ctx.move_to(x, y);
    let _ = ctx.show_text(&status_text);
}

fn tool_display_name(input_state: &InputState, tool: Tool) -> &'static str {
    match &input_state.state {
        DrawingState::TextInput { .. } => match input_state.text_input_mode {
            TextInputMode::Plain => action_display_label(Action::EnterTextMode),
            TextInputMode::StickyNote => action_display_label(Action::EnterStickyNoteMode),
        },
        DrawingState::Drawing { tool, .. } => tool_action_label(*tool),
        DrawingState::MovingSelection { .. } => "Move",
        DrawingState::Selecting { .. } => "Select",
        DrawingState::ResizingText { .. } => "Resize",
        DrawingState::PendingTextClick { .. } | DrawingState::Idle => tool_action_label(tool),
    }
}

fn tool_action_label(tool: Tool) -> &'static str {
    action_for_tool(tool)
        .map(action_display_label)
        .unwrap_or("Select")
}
