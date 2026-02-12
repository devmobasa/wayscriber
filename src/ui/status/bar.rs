use std::f64::consts::PI;

use crate::config::{Action, StatusPosition, action_display_label};
use crate::input::{BoardBackground, DrawingState, InputState, TextInputMode, Tool};
use crate::label_format::format_binding_labels;
use crate::ui::toolbar::bindings::action_for_tool;
use crate::ui_text::{UiTextStyle, text_layout};

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
    let thickness = input_state.size_for_active_tool();

    let tool_name = tool_display_name(input_state, tool);
    let color_name = crate::util::color_to_name(color);

    let board_badge = if input_state.show_status_board_badge && input_state.boards.show_badge() {
        let board_index = input_state.boards.active_index() + 1;
        let board_count = input_state.boards.board_count();
        let board_name = crate::util::truncate_with_ellipsis(input_state.board_name(), 20);
        format!("[Board {}/{}: {}] ", board_index, board_count, board_name)
    } else {
        String::new()
    };
    let page_count = input_state.boards.page_count().max(1);
    let page_index = input_state.boards.active_page_index();
    let page_name = input_state
        .boards
        .board_states()
        .get(input_state.boards.active_index())
        .and_then(|board| board.pages.page_name(page_index))
        .map(|name| crate::util::truncate_with_ellipsis(name, 20));
    let page_badge = if input_state.show_status_page_badge {
        if let Some(name) = page_name {
            format!("[Page {}/{}: {}] ", page_index + 1, page_count, name)
        } else {
            format!("[Page {}/{}] ", page_index + 1, page_count)
        }
    } else {
        String::new()
    };
    let output_badge = if input_state.show_active_output_badge {
        input_state
            .active_output_label
            .as_ref()
            .map(|label| {
                let label = crate::util::truncate_with_ellipsis(label, 28);
                format!("[Output: {label}] ")
            })
            .unwrap_or_default()
    } else {
        String::new()
    };

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
    let help_binding = help_binding_label(input_state);

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

    let selection_badge = if let Some(bounds) = input_state.selection_bounds() {
        let count = input_state.selected_shape_ids().len();
        if count == 1 {
            format!("[{}×{}px] ", bounds.width, bounds.height)
        } else {
            format!("[{} items: {}×{}px] ", count, bounds.width, bounds.height)
        }
    } else {
        String::new()
    };

    let status_text = format!(
        "{}{}{}{}{}{}[{}] [{}px] [{}] [Text {}px]{}{}  {}={}",
        frozen_badge,
        zoom_badge,
        selection_badge,
        output_badge,
        board_badge,
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

    // Limit status bar to 80% of screen width to prevent overflow
    let max_width = (screen_width as f64 * 0.8) - style.padding * 2.0;

    let layout = text_layout(
        ctx,
        UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Bold,
            size: style.font_size,
        },
        &status_text,
        Some(max_width),
    );
    let extents = layout.ink_extents();
    let text_width = extents.width().min(max_width);
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

    let (bg_color, text_color) = match input_state.boards.active_background() {
        BoardBackground::Transparent => (style.bg_color, style.text_color),
        BoardBackground::Solid(color) => status_bar_palette_for_background(*color),
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
    layout.show_at_baseline(ctx, x, y);
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
        DrawingState::ResizingText { .. } | DrawingState::ResizingSelection { .. } => "Resize",
        DrawingState::PendingTextClick { .. } | DrawingState::Idle => tool_action_label(tool),
    }
}

fn help_binding_label(input_state: &InputState) -> String {
    let mut labels = input_state.action_binding_labels(Action::ToggleHelp);
    if labels.iter().any(|label| label == "F1") {
        // Prefer showing F1 in the status bar when both defaults are bound.
        labels.retain(|label| label != "F10");
    }
    format_binding_labels(&labels)
}

fn tool_action_label(tool: Tool) -> &'static str {
    action_for_tool(tool)
        .map(action_display_label)
        .unwrap_or("Select")
}

fn status_bar_palette_for_background(color: crate::draw::Color) -> ([f64; 4], [f64; 4]) {
    let luminance = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
    if luminance > 0.5 {
        ([0.15, 0.15, 0.15, 0.85], [1.0, 1.0, 1.0, 1.0])
    } else {
        ([0.85, 0.85, 0.85, 0.85], [0.0, 0.0, 0.0, 1.0])
    }
}
