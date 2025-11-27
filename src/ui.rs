pub mod toolbar;

/// UI rendering: status bar, help overlay, visual indicators
use crate::config::StatusPosition;
use crate::input::{BoardMode, DrawingState, InputState, Tool, state::ContextMenuState};
use std::f64::consts::{FRAC_PI_2, PI};

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

fn fallback_text_extents(font_size: f64, text: &str) -> cairo::TextExtents {
    let width = text.len() as f64 * font_size * 0.5;
    cairo::TextExtents::new(0.0, -font_size, width, font_size, width, 0.0)
}

fn text_extents_for(
    ctx: &cairo::Context,
    family: &str,
    slant: cairo::FontSlant,
    weight: cairo::FontWeight,
    size: f64,
    text: &str,
) -> cairo::TextExtents {
    ctx.select_font_face(family, slant, weight);
    ctx.set_font_size(size);
    match ctx.text_extents(text) {
        Ok(extents) => extents,
        Err(err) => {
            log::warn!(
                "Failed to measure text '{}': {}, using fallback metrics",
                text,
                err
            );
            fallback_text_extents(size, text)
        }
    }
}

fn draw_rounded_rect(ctx: &cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let r = radius.min(width / 2.0).min(height / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + width - r, y + r, r, -FRAC_PI_2, 0.0);
    ctx.arc(x + width - r, y + height - r, r, 0.0, FRAC_PI_2);
    ctx.arc(x + r, y + height - r, r, FRAC_PI_2, PI);
    ctx.arc(x + r, y + r, r, PI, 3.0 * FRAC_PI_2);
    ctx.close_path();
}

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
        DrawingState::TextInput { .. } => "Text",
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
        DrawingState::Idle => match tool {
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

    let status_text = format!(
        "{}{}[{}] [{}px] [{}] [Text {}px]{}{}  F1=Help",
        frozen_badge,
        mode_badge,
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

    // Background with a vivid color to stand out
    ctx.set_source_rgba(0.98, 0.55, 0.26, 0.92); // orange-ish
    draw_rounded_rect(ctx, x, y - height, width, height, radius);
    let _ = ctx.fill();

    // Text
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    ctx.move_to(x + (padding * 0.7), y - (padding * 0.35));
    let _ = ctx.show_text(label);
}

/// Render help overlay showing all keybindings
pub fn render_help_overlay(
    ctx: &cairo::Context,
    style: &crate::config::HelpOverlayStyle,
    screen_width: u32,
    screen_height: u32,
    frozen_enabled: bool,
) {
    struct Row {
        key: &'static str,
        action: &'static str,
    }

    struct Badge {
        label: &'static str,
        color: [f64; 3],
    }

    struct Section {
        title: &'static str,
        rows: Vec<Row>,
        badges: Vec<Badge>,
    }

    struct MeasuredSection {
        section: Section,
        width: f64,
        height: f64,
        key_column_width: f64,
    }

    let mut board_rows = vec![
        Row {
            key: "Ctrl+W",
            action: "Toggle Whiteboard",
        },
        Row {
            key: "Ctrl+B",
            action: "Toggle Blackboard",
        },
        Row {
            key: "Ctrl+Shift+T",
            action: "Return to Transparent",
        },
    ];

    if frozen_enabled {
        board_rows.push(Row {
            key: "Ctrl+Shift+F",
            action: "Freeze/unfreeze active monitor",
        });
    }

    let sections = vec![
        Section {
            title: "Board Modes",
            rows: board_rows,
            badges: Vec::new(),
        },
        Section {
            title: "Selection",
            rows: vec![
                Row {
                    key: "Alt+Click",
                    action: "Select & move shape",
                },
                Row {
                    key: "Shift+Alt+Click",
                    action: "Add to selection",
                },
                Row {
                    key: "Delete",
                    action: "Delete selection",
                },
                Row {
                    key: "Ctrl+D",
                    action: "Duplicate selection",
                },
            ],
            badges: Vec::new(),
        },
        Section {
            title: "Drawing Tools",
            rows: vec![
                Row {
                    key: "F / Drag",
                    action: "Freehand pen",
                },
                Row {
                    key: "Shift+Drag",
                    action: "Straight line",
                },
                Row {
                    key: "Ctrl+Drag",
                    action: "Rectangle",
                },
                Row {
                    key: "Tab+Drag",
                    action: "Circle",
                },
                Row {
                    key: "Ctrl+Shift+Drag",
                    action: "Arrow",
                },
                Row {
                    key: "Ctrl+Alt+H",
                    action: "Toggle highlight-only tool",
                },
                Row {
                    key: "T",
                    action: "Text mode",
                },
                Row {
                    key: "D",
                    action: "Eraser tool",
                },
                Row {
                    key: "H",
                    action: "Marker tool",
                },
            ],
            badges: Vec::new(),
        },
        Section {
            title: "Pen & Text",
            rows: vec![
                Row {
                    key: "+/- or Scroll",
                    action: "Adjust size (pen/eraser)",
                },
                Row {
                    key: "Ctrl+Shift+/-",
                    action: "Font size",
                },
                Row {
                    key: "Shift+Scroll",
                    action: "Font size",
                },
            ],
            badges: vec![
                Badge {
                    label: "R",
                    color: [0.94, 0.36, 0.36],
                },
                Badge {
                    label: "G",
                    color: [0.30, 0.78, 0.51],
                },
                Badge {
                    label: "B",
                    color: [0.36, 0.60, 0.95],
                },
                Badge {
                    label: "Y",
                    color: [0.98, 0.80, 0.10],
                },
                Badge {
                    label: "O",
                    color: [0.98, 0.55, 0.26],
                },
                Badge {
                    label: "P",
                    color: [0.78, 0.47, 0.96],
                },
                Badge {
                    label: "W",
                    color: [0.90, 0.92, 0.96],
                },
                Badge {
                    label: "K",
                    color: [0.28, 0.30, 0.38],
                },
            ],
        },
        Section {
            title: "Actions",
            rows: vec![
                Row {
                    key: "E",
                    action: "Clear frame",
                },
                Row {
                    key: "Ctrl+Z",
                    action: "Undo",
                },
                Row {
                    key: "Ctrl+Shift+H",
                    action: "Toggle click highlight",
                },
                Row {
                    key: "Right Click / Shift+F10",
                    action: "Context menu",
                },
                Row {
                    key: "Escape / Ctrl+Q",
                    action: "Exit",
                },
                Row {
                    key: "F1 / F10",
                    action: "Toggle help",
                },
                Row {
                    key: "F2 / F9",
                    action: "Toggle toolbar",
                },
                Row {
                    key: "F11",
                    action: "Open configurator",
                },
                Row {
                    key: "F4 / F12",
                    action: "Toggle status bar",
                },
            ],
            badges: Vec::new(),
        },
        Section {
            title: "Screenshots",
            rows: vec![
                Row {
                    key: "Ctrl+C",
                    action: "Full screen → clipboard",
                },
                Row {
                    key: "Ctrl+S",
                    action: "Full screen → file",
                },
                Row {
                    key: "Ctrl+Shift+C",
                    action: "Region → clipboard",
                },
                Row {
                    key: "Ctrl+Shift+S",
                    action: "Region → file",
                },
                Row {
                    key: "Ctrl+Shift+O",
                    action: "Active window (Hyprland)",
                },
                Row {
                    key: "Ctrl+Shift+I",
                    action: "Selection (capture defaults)",
                },
            ],
            badges: Vec::new(),
        },
    ];

    let title_text = "Wayscriber Controls";
    let commit_hash = option_env!("WAYSCRIBER_GIT_HASH").unwrap_or("unknown");
    let version_line = format!(
        "Wayscriber {} ({})  •  F11 → Open Configurator",
        env!("CARGO_PKG_VERSION"),
        commit_hash
    );
    let note_text = "Note: Each board mode has independent drawings";

    let body_font_size = style.font_size;
    let heading_font_size = body_font_size + 6.0;
    let title_font_size = heading_font_size + 6.0;
    let subtitle_font_size = body_font_size;
    let row_line_height = style.line_height.max(body_font_size + 4.0);
    let heading_line_height = heading_font_size + 6.0;
    let row_gap_after_heading = 6.0;
    let key_desc_gap = 20.0;
    let row_gap = 28.0;
    let column_gap = 48.0;
    let badge_font_size = (body_font_size - 2.0).max(12.0);
    let badge_padding_x = 12.0;
    let badge_padding_y = 6.0;
    let badge_gap = 12.0;
    let badge_height = badge_font_size + badge_padding_y * 2.0;
    let badge_corner_radius = 10.0;
    let badge_top_gap = 10.0;
    let accent_line_height = 2.0;
    let accent_line_bottom_spacing = 16.0;
    let title_bottom_spacing = 8.0;
    let subtitle_bottom_spacing = 28.0;
    let columns_bottom_spacing = 28.0;

    let lerp = |a: f64, b: f64, t: f64| a * (1.0 - t) + b * t;

    let [bg_r, bg_g, bg_b, bg_a] = style.bg_color;
    let bg_top = [
        (bg_r + 0.04).min(1.0),
        (bg_g + 0.04).min(1.0),
        (bg_b + 0.04).min(1.0),
        bg_a,
    ];
    let bg_bottom = [
        (bg_r - 0.03).max(0.0),
        (bg_g - 0.03).max(0.0),
        (bg_b - 0.03).max(0.0),
        bg_a,
    ];

    let accent_color = [0.96, 0.78, 0.38, 1.0];
    let subtitle_color = [0.62, 0.66, 0.76, 1.0];
    let body_text_color = style.text_color;
    let description_color = [
        lerp(body_text_color[0], subtitle_color[0], 0.35),
        lerp(body_text_color[1], subtitle_color[1], 0.35),
        lerp(body_text_color[2], subtitle_color[2], 0.35),
        body_text_color[3],
    ];
    let note_color = [subtitle_color[0], subtitle_color[1], subtitle_color[2], 0.9];

    let mut measured_sections = Vec::with_capacity(sections.len());
    for section in sections {
        let mut key_max_width: f64 = 0.0;
        for row in &section.rows {
            if row.key.is_empty() {
                continue;
            }
            let key_extents = text_extents_for(
                ctx,
                "Sans",
                cairo::FontSlant::Normal,
                cairo::FontWeight::Bold,
                body_font_size,
                row.key,
            );
            key_max_width = key_max_width.max(key_extents.width());
        }

        let mut section_width: f64 = 0.0;
        let mut section_height: f64 = 0.0;

        let heading_extents = text_extents_for(
            ctx,
            "Sans",
            cairo::FontSlant::Normal,
            cairo::FontWeight::Bold,
            heading_font_size,
            section.title,
        );
        section_width = section_width.max(heading_extents.width());
        section_height += heading_line_height;

        if !section.rows.is_empty() {
            section_height += row_gap_after_heading;
            for row in &section.rows {
                let desc_extents = text_extents_for(
                    ctx,
                    "Sans",
                    cairo::FontSlant::Normal,
                    cairo::FontWeight::Normal,
                    body_font_size,
                    row.action,
                );
                let row_width = key_max_width + key_desc_gap + desc_extents.width();
                section_width = section_width.max(row_width);
                section_height += row_line_height;
            }
        }

        if !section.badges.is_empty() {
            section_height += badge_top_gap;
            let mut badges_width = 0.0;

            for (index, badge) in section.badges.iter().enumerate() {
                let badge_extents = text_extents_for(
                    ctx,
                    "Sans",
                    cairo::FontSlant::Normal,
                    cairo::FontWeight::Bold,
                    badge_font_size,
                    badge.label,
                );
                let badge_width = badge_extents.width() + badge_padding_x * 2.0;
                if index > 0 {
                    badges_width += badge_gap;
                }
                badges_width += badge_width;
            }

            section_width = section_width.max(badges_width);
            section_height += badge_height;
        }

        measured_sections.push(MeasuredSection {
            section,
            width: section_width,
            height: section_height,
            key_column_width: key_max_width,
        });
    }

    let mut rows: Vec<Vec<MeasuredSection>> = Vec::new();
    if measured_sections.is_empty() {
        rows.push(Vec::new());
    } else if measured_sections.len() <= 2 {
        rows.push(measured_sections);
    } else {
        let mut split = measured_sections;
        let first_row_len = (split.len() + 1) / 2;
        let second_row = split.split_off(first_row_len);
        rows.push(split);
        rows.push(second_row);
    }

    let mut row_widths: Vec<f64> = Vec::with_capacity(rows.len());
    let mut row_heights: Vec<f64> = Vec::with_capacity(rows.len());
    let mut grid_width: f64 = 0.0;
    for row in &rows {
        if row.is_empty() {
            row_widths.push(0.0);
            row_heights.push(0.0);
            continue;
        }

        let mut width: f64 = 0.0;
        let mut height: f64 = 0.0;
        for (index, section) in row.iter().enumerate() {
            if index > 0 {
                width += column_gap;
            }
            width += section.width;
            height = height.max(section.height);
        }
        grid_width = grid_width.max(width);
        row_widths.push(width);
        row_heights.push(height);
    }

    let mut grid_height: f64 = 0.0;
    for (index, height) in row_heights.iter().enumerate() {
        grid_height += *height;
        if index + 1 < row_heights.len() {
            grid_height += row_gap;
        }
    }

    let title_extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        title_font_size,
        title_text,
    );
    let subtitle_extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        subtitle_font_size,
        &version_line,
    );
    let note_font_size = (body_font_size - 2.0).max(12.0);
    let note_extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        note_font_size,
        note_text,
    );

    let mut content_width = grid_width
        .max(title_extents.width())
        .max(subtitle_extents.width())
        .max(note_extents.width());
    if rows.is_empty() {
        content_width = content_width
            .max(title_extents.width())
            .max(subtitle_extents.width());
    }

    let box_width = content_width + style.padding * 2.0;
    let content_height = accent_line_height
        + accent_line_bottom_spacing
        + title_font_size
        + title_bottom_spacing
        + subtitle_font_size
        + subtitle_bottom_spacing
        + grid_height
        + columns_bottom_spacing
        + note_font_size;
    let box_height = content_height + style.padding * 2.0;

    let box_x = (screen_width as f64 - box_width) / 2.0;
    let box_y = (screen_height as f64 - box_height) / 2.0;

    // Dim background behind overlay
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.55);
    ctx.rectangle(0.0, 0.0, screen_width as f64, screen_height as f64);
    let _ = ctx.fill();

    // Drop shadow
    let shadow_offset = 10.0;
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.45);
    ctx.rectangle(
        box_x + shadow_offset,
        box_y + shadow_offset,
        box_width,
        box_height,
    );
    let _ = ctx.fill();

    // Background gradient
    let gradient = cairo::LinearGradient::new(box_x, box_y, box_x, box_y + box_height);
    gradient.add_color_stop_rgba(0.0, bg_top[0], bg_top[1], bg_top[2], bg_top[3]);
    gradient.add_color_stop_rgba(1.0, bg_bottom[0], bg_bottom[1], bg_bottom[2], bg_bottom[3]);
    let _ = ctx.set_source(&gradient);
    ctx.rectangle(box_x, box_y, box_width, box_height);
    let _ = ctx.fill();

    // Border
    let [br, bg, bb, ba] = style.border_color;
    ctx.set_source_rgba(br, bg, bb, ba);
    ctx.set_line_width(style.border_width);
    ctx.rectangle(box_x, box_y, box_width, box_height);
    let _ = ctx.stroke();

    let inner_x = box_x + style.padding;
    let mut cursor_y = box_y + style.padding;
    let inner_width = box_width - style.padding * 2.0;

    // Accent line
    ctx.set_source_rgba(
        accent_color[0],
        accent_color[1],
        accent_color[2],
        accent_color[3],
    );
    ctx.rectangle(inner_x, cursor_y, inner_width, accent_line_height);
    let _ = ctx.fill();
    cursor_y += accent_line_height + accent_line_bottom_spacing;

    // Title
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(title_font_size);
    ctx.set_source_rgba(
        body_text_color[0],
        body_text_color[1],
        body_text_color[2],
        body_text_color[3],
    );
    let title_baseline = cursor_y + title_font_size;
    ctx.move_to(inner_x, title_baseline);
    let _ = ctx.show_text(title_text);
    cursor_y += title_font_size + title_bottom_spacing;

    // Subtitle / version line
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(subtitle_font_size);
    ctx.set_source_rgba(
        subtitle_color[0],
        subtitle_color[1],
        subtitle_color[2],
        subtitle_color[3],
    );
    let subtitle_baseline = cursor_y + subtitle_font_size;
    ctx.move_to(inner_x, subtitle_baseline);
    let _ = ctx.show_text(&version_line);
    cursor_y += subtitle_font_size + subtitle_bottom_spacing;

    let grid_start_y = cursor_y;

    let mut row_y = grid_start_y;
    for (row_index, row) in rows.iter().enumerate() {
        let row_height = *row_heights.get(row_index).unwrap_or(&0.0);
        let row_width = *row_widths.get(row_index).unwrap_or(&inner_width);
        if row.is_empty() {
            row_y += row_height;
            if row_index + 1 < rows.len() {
                row_y += row_gap;
            }
            continue;
        }

        let mut section_x = inner_x + (inner_width - row_width) / 2.0;
        for (section_index, measured) in row.iter().enumerate() {
            if section_index > 0 {
                section_x += column_gap;
            }

            let mut section_y = row_y;
            let desc_x = section_x + measured.key_column_width + key_desc_gap;
            let section = &measured.section;

            ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            ctx.set_font_size(heading_font_size);
            ctx.set_source_rgba(
                accent_color[0],
                accent_color[1],
                accent_color[2],
                accent_color[3],
            );
            let heading_baseline = section_y + heading_font_size;
            ctx.move_to(section_x, heading_baseline);
            let _ = ctx.show_text(section.title);
            section_y += heading_line_height;

            if !section.rows.is_empty() {
                section_y += row_gap_after_heading;
                for row_data in &section.rows {
                    let baseline = section_y + body_font_size;

                    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
                    ctx.set_font_size(body_font_size);
                    ctx.set_source_rgba(accent_color[0], accent_color[1], accent_color[2], 0.95);
                    ctx.move_to(section_x, baseline);
                    let _ = ctx.show_text(row_data.key);

                    ctx.select_font_face(
                        "Sans",
                        cairo::FontSlant::Normal,
                        cairo::FontWeight::Normal,
                    );
                    ctx.set_font_size(body_font_size);
                    ctx.set_source_rgba(
                        description_color[0],
                        description_color[1],
                        description_color[2],
                        description_color[3],
                    );
                    ctx.move_to(desc_x, baseline);
                    let _ = ctx.show_text(row_data.action);

                    section_y += row_line_height;
                }
            }

            if !section.badges.is_empty() {
                section_y += badge_top_gap;
                let mut badge_x = section_x;

                for (badge_index, badge) in section.badges.iter().enumerate() {
                    if badge_index > 0 {
                        badge_x += badge_gap;
                    }

                    ctx.new_path();
                    let badge_text_extents = text_extents_for(
                        ctx,
                        "Sans",
                        cairo::FontSlant::Normal,
                        cairo::FontWeight::Bold,
                        badge_font_size,
                        badge.label,
                    );
                    let badge_width = badge_text_extents.width() + badge_padding_x * 2.0;

                    draw_rounded_rect(
                        ctx,
                        badge_x,
                        section_y,
                        badge_width,
                        badge_height,
                        badge_corner_radius,
                    );
                    ctx.set_source_rgba(badge.color[0], badge.color[1], badge.color[2], 0.25);
                    let _ = ctx.fill_preserve();

                    ctx.set_source_rgba(badge.color[0], badge.color[1], badge.color[2], 0.85);
                    ctx.set_line_width(1.0);
                    let _ = ctx.stroke();

                    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
                    ctx.set_font_size(badge_font_size);
                    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.92);
                    let text_x = badge_x + badge_padding_x;
                    let text_y = section_y + (badge_height - badge_text_extents.height()) / 2.0
                        - badge_text_extents.y_bearing();
                    ctx.move_to(text_x, text_y);
                    let _ = ctx.show_text(badge.label);

                    badge_x += badge_width;
                }
            }

            section_x += measured.width;
        }

        row_y += row_height;
        if row_index + 1 < rows.len() {
            row_y += row_gap;
        }
    }

    cursor_y = grid_start_y + grid_height + columns_bottom_spacing;

    // Note
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(note_font_size);
    ctx.set_source_rgba(note_color[0], note_color[1], note_color[2], note_color[3]);
    let note_x = inner_x + (inner_width - note_extents.width()) / 2.0;
    let note_baseline = cursor_y + note_font_size;
    ctx.move_to(note_x, note_baseline);
    let _ = ctx.show_text(note_text);
}

/// Renders a floating context menu for shape or canvas actions.
pub fn render_context_menu(
    ctx: &cairo::Context,
    input_state: &InputState,
    _screen_width: u32,
    _screen_height: u32,
) {
    let (hover_index, focus_index) = match &input_state.context_menu_state {
        ContextMenuState::Open {
            hover_index,
            keyboard_focus,
            ..
        } => (*hover_index, *keyboard_focus),
        ContextMenuState::Hidden => return,
    };

    let entries = input_state.context_menu_entries();
    if entries.is_empty() {
        return;
    }

    let layout = match input_state.context_menu_layout() {
        Some(layout) => *layout,
        None => return,
    };

    let _ = ctx.save();
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(layout.font_size);

    // Background and border
    ctx.set_source_rgba(0.1, 0.13, 0.17, 0.95);
    ctx.rectangle(
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
    );
    let _ = ctx.fill();

    ctx.set_source_rgba(0.18, 0.22, 0.28, 0.9);
    ctx.set_line_width(1.0);
    ctx.rectangle(
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
    );
    let _ = ctx.stroke();

    let active_index = hover_index.or(focus_index);

    for (index, entry) in entries.iter().enumerate() {
        let row_top = layout.origin_y + layout.padding_y + layout.row_height * index as f64;
        let row_center = row_top + layout.row_height * 0.5;

        if active_index == Some(index) && !entry.disabled {
            ctx.set_source_rgba(0.25, 0.32, 0.45, 0.9);
            ctx.rectangle(layout.origin_x, row_top, layout.width, layout.row_height);
            let _ = ctx.fill();
        }

        let (text_r, text_g, text_b, text_a) = if entry.disabled {
            (0.6, 0.64, 0.68, 0.5)
        } else {
            (0.9, 0.92, 0.97, 1.0)
        };

        ctx.set_source_rgba(text_r, text_g, text_b, text_a);
        ctx.move_to(
            layout.origin_x + layout.padding_x,
            row_center + layout.font_size * 0.35,
        );
        let _ = ctx.show_text(&entry.label);

        if let Some(shortcut) = &entry.shortcut {
            ctx.set_source_rgba(0.7, 0.73, 0.78, text_a);
            let shortcut_x = layout.origin_x + layout.width
                - layout.padding_x
                - layout.arrow_width
                - layout.shortcut_width;
            ctx.move_to(shortcut_x, row_center + layout.font_size * 0.35);
            let _ = ctx.show_text(shortcut);
        }

        if entry.has_submenu {
            let arrow_x =
                layout.origin_x + layout.width - layout.padding_x - layout.arrow_width * 0.6;
            let arrow_y = row_center;
            ctx.set_source_rgba(0.75, 0.78, 0.84, text_a);
            ctx.move_to(arrow_x, arrow_y - 5.0);
            ctx.line_to(arrow_x + 6.0, arrow_y);
            ctx.line_to(arrow_x, arrow_y + 5.0);
            let _ = ctx.fill();
        }
    }

    let _ = ctx.restore();
}

pub fn render_properties_panel(
    ctx: &cairo::Context,
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) {
    let panel = match input_state.properties_panel() {
        Some(panel) => panel,
        None => return,
    };

    let title_font_size = 15.0;
    let body_font_size = 13.0;
    let line_height = 18.0;
    let padding_x = 16.0;
    let padding_y = 12.0;
    let margin = 12.0;

    let _ = ctx.save();
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(title_font_size);
    let mut max_width = ctx
        .text_extents(&panel.title)
        .map(|ext| ext.width())
        .unwrap_or(0.0);

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(body_font_size);
    for line in &panel.lines {
        if let Ok(extents) = ctx.text_extents(line) {
            max_width = max_width.max(extents.width());
        }
    }

    let content_height = if panel.lines.is_empty() {
        0.0
    } else {
        line_height * panel.lines.len() as f64
    };
    let title_height = title_font_size + 4.0;
    let panel_width = max_width + padding_x * 2.0;
    let panel_height = padding_y * 2.0 + title_height + content_height;

    let screen_w = screen_width as f64;
    let screen_h = screen_height as f64;
    let mut origin_x = panel.anchor.0;
    let mut origin_y = panel.anchor.1;

    if origin_x + panel_width > screen_w - margin {
        origin_x = (screen_w - panel_width - margin).max(margin);
    }
    if origin_y + panel_height > screen_h - margin {
        origin_y = (screen_h - panel_height - margin).max(margin);
    }
    if origin_x < margin {
        origin_x = margin;
    }
    if origin_y < margin {
        origin_y = margin;
    }

    ctx.set_source_rgba(0.08, 0.11, 0.17, 0.92);
    ctx.rectangle(origin_x, origin_y, panel_width, panel_height);
    let _ = ctx.fill();

    ctx.set_source_rgba(0.18, 0.22, 0.3, 0.95);
    ctx.set_line_width(1.0);
    ctx.rectangle(origin_x, origin_y, panel_width, panel_height);
    let _ = ctx.stroke();

    let mut text_y = origin_y + padding_y + title_font_size;

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(title_font_size);
    if panel.multiple_selection {
        ctx.set_source_rgba(0.88, 0.91, 0.97, 1.0);
    } else {
        ctx.set_source_rgba(0.93, 0.95, 0.99, 1.0);
    }
    ctx.move_to(origin_x + padding_x, text_y);
    let _ = ctx.show_text(&panel.title);

    ctx.set_source_rgba(0.35, 0.4, 0.5, 0.9);
    ctx.move_to(origin_x + padding_x, text_y + 4.0);
    ctx.line_to(origin_x + panel_width - padding_x, text_y + 4.0);
    let _ = ctx.stroke();

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(body_font_size);
    ctx.set_source_rgba(0.86, 0.89, 0.95, 1.0);
    text_y += 12.0;
    for line in &panel.lines {
        ctx.move_to(origin_x + padding_x, text_y);
        let _ = ctx.show_text(line);
        text_y += line_height;
    }

    let _ = ctx.restore();
}
