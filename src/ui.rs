/// UI rendering: status bar, help overlay, visual indicators
use crate::config::StatusPosition;
use crate::input::{BoardMode, DrawingState, InputState, Tool};
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
    ctx.arc(dot_x, dot_y, style.dot_radius, 0.0, 2.0 * PI);
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

    struct Column {
        sections: Vec<Section>,
    }

    let columns = vec![
        Column {
            sections: vec![
                Section {
                    title: "Board Modes",
                    rows: vec![
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
                    ],
                    badges: Vec::new(),
                },
                Section {
                    title: "Pen & Text",
                    rows: vec![
                        Row {
                            key: "+/- or Scroll",
                            action: "Adjust pen thickness",
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
            ],
        },
        Column {
            sections: vec![
                Section {
                    title: "Drawing Tools",
                    rows: vec![
                        Row {
                            key: "Drag",
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
                            key: "T",
                            action: "Text mode",
                        },
                    ],
                    badges: Vec::new(),
                },
                Section {
                    title: "Colors",
                    rows: Vec::new(),
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
                            key: "Escape / Ctrl+Q",
                            action: "Exit",
                        },
                        Row {
                            key: "F10",
                            action: "Toggle help",
                        },
                    ],
                    badges: Vec::new(),
                },
            ],
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
    let section_gap = 28.0;
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

    let mut column_widths = Vec::with_capacity(columns.len());
    let mut column_heights = Vec::with_capacity(columns.len());
    let mut key_column_widths = Vec::with_capacity(columns.len());

    for column in &columns {
        let mut key_max_width: f64 = 0.0;
        for section in &column.sections {
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
        }
        key_column_widths.push(key_max_width);

        let mut column_width: f64 = 0.0;
        let mut column_height: f64 = 0.0;
        let mut first_section = true;

        for section in &column.sections {
            if !first_section {
                column_height += section_gap;
            }
            first_section = false;

            let heading_extents = text_extents_for(
                ctx,
                "Sans",
                cairo::FontSlant::Normal,
                cairo::FontWeight::Bold,
                heading_font_size,
                section.title,
            );
            column_width = column_width.max(heading_extents.width());
            column_height += heading_line_height;

            if !section.rows.is_empty() {
                column_height += row_gap_after_heading;
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
                    column_width = column_width.max(row_width);
                    column_height += row_line_height;
                }
            }

            if !section.badges.is_empty() {
                column_height += badge_top_gap;
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

                column_width = column_width.max(badges_width);
                column_height += badge_height;
            }
        }

        column_widths.push(column_width);
        column_heights.push(column_height);
    }

    let columns_height = column_heights.iter().copied().fold(0.0, f64::max);
    let mut columns_width_total = column_widths.iter().sum::<f64>();
    if columns.len() > 1 {
        columns_width_total += column_gap * (columns.len() - 1) as f64;
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

    let mut content_width = columns_width_total
        .max(title_extents.width())
        .max(subtitle_extents.width())
        .max(note_extents.width());
    if columns.is_empty() {
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
        + columns_height
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

    let columns_start_y = cursor_y;

    // Columns
    let mut column_x = inner_x;
    for (idx, column) in columns.iter().enumerate() {
        let mut column_y = columns_start_y;
        let key_width = key_column_widths[idx];
        let column_width = column_widths[idx];
        let desc_x = column_x + key_width + key_desc_gap;

        let mut first_section = true;
        for section in &column.sections {
            if !first_section {
                column_y += section_gap;
            }
            first_section = false;

            ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            ctx.set_font_size(heading_font_size);
            ctx.set_source_rgba(
                accent_color[0],
                accent_color[1],
                accent_color[2],
                accent_color[3],
            );
            let heading_baseline = column_y + heading_font_size;
            ctx.move_to(column_x, heading_baseline);
            let _ = ctx.show_text(section.title);
            column_y += heading_line_height;

            if !section.rows.is_empty() {
                column_y += row_gap_after_heading;
                for row in &section.rows {
                    let baseline = column_y + body_font_size;

                    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
                    ctx.set_font_size(body_font_size);
                    ctx.set_source_rgba(accent_color[0], accent_color[1], accent_color[2], 0.95);
                    ctx.move_to(column_x, baseline);
                    let _ = ctx.show_text(row.key);

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
                    let _ = ctx.show_text(row.action);

                    column_y += row_line_height;
                }
            }

            if !section.badges.is_empty() {
                column_y += badge_top_gap;
                let mut badge_x = column_x;

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
                        column_y,
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
                    let text_y = column_y + (badge_height - badge_text_extents.height()) / 2.0
                        - badge_text_extents.y_bearing();
                    ctx.move_to(text_x, text_y);
                    let _ = ctx.show_text(badge.label);

                    badge_x += badge_width;
                }

                column_y += badge_height;
            }
        }

        column_x += column_width + column_gap;
    }

    cursor_y = columns_start_y + columns_height + columns_bottom_spacing;

    // Note
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(note_font_size);
    ctx.set_source_rgba(note_color[0], note_color[1], note_color[2], note_color[3]);
    let note_x = inner_x + (inner_width - note_extents.width()) / 2.0;
    let note_baseline = cursor_y + note_font_size;
    ctx.move_to(note_x, note_baseline);
    let _ = ctx.show_text(note_text);
}
