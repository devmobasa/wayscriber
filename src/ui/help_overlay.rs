use crate::input::HelpOverlayView;
use crate::toolbar_icons;
use pango::prelude::*;
use std::collections::HashSet;
use std::sync::OnceLock;

use super::primitives::{draw_rounded_rect, fallback_text_extents, text_extents_for};

fn resolve_help_font_family(family_list: &str) -> String {
    let mut fallback = None;
    for raw in family_list.split(',') {
        let candidate = raw.trim();
        if candidate.is_empty() {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(candidate);
        }
        let key = candidate.to_ascii_lowercase();
        if help_font_families().contains(&key) {
            return candidate.to_string();
        }
    }
    fallback.unwrap_or("Sans").to_string()
}

fn help_font_families() -> &'static HashSet<String> {
    static CACHE: OnceLock<HashSet<String>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let font_map = pangocairo::FontMap::default();
        font_map
            .list_families()
            .into_iter()
            .map(|family| family.name().to_ascii_lowercase())
            .collect()
    })
}

/// Draw a keyboard key with keycap styling
fn draw_keycap(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    text: &str,
    font_family: &str,
    font_size: f64,
    text_color: [f64; 4],
) -> f64 {
    let padding_x = 8.0;
    let padding_y = 4.0;
    let radius = 5.0;
    let shadow_offset = 2.0;

    ctx.select_font_face(
        font_family,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    ctx.set_font_size(font_size);
    let extents = ctx
        .text_extents(text)
        .unwrap_or_else(|_| fallback_text_extents(font_size, text));

    let cap_width = extents.width() + padding_x * 2.0;
    let cap_height = font_size + padding_y * 2.0;
    let cap_y = y - font_size - padding_y;

    // Drop shadow for 3D depth effect
    draw_rounded_rect(
        ctx,
        x + 1.0,
        cap_y + shadow_offset,
        cap_width,
        cap_height,
        radius,
    );
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    let _ = ctx.fill();

    // Keycap main background
    draw_rounded_rect(ctx, x, cap_y, cap_width, cap_height, radius);
    ctx.set_source_rgba(0.18, 0.22, 0.3, 1.0);
    let _ = ctx.fill();

    // Inner highlight for depth
    draw_rounded_rect(
        ctx,
        x + 1.0,
        cap_y + 1.0,
        cap_width - 2.0,
        cap_height - 2.0,
        radius - 1.0,
    );
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.12);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    // Outer border
    draw_rounded_rect(ctx, x, cap_y, cap_width, cap_height, radius);
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.2);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    // Text
    ctx.select_font_face(
        font_family,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    ctx.set_font_size(font_size);
    ctx.set_source_rgba(text_color[0], text_color[1], text_color[2], text_color[3]);
    ctx.move_to(x + padding_x, y);
    let _ = ctx.show_text(text);

    cap_width
}

struct KeyComboStyle<'a> {
    font_family: &'a str,
    font_size: f64,
    text_color: [f64; 4],
    separator_color: [f64; 4],
}

/// Measure the width of a key combination string with keycap styling
fn measure_key_combo(
    ctx: &cairo::Context,
    key_str: &str,
    font_family: &str,
    font_size: f64,
) -> f64 {
    let keycap_padding_x = 8.0;
    let key_gap = 5.0;
    let separator_gap = 6.0;

    let mut total_width = 0.0;

    ctx.select_font_face(
        font_family,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    ctx.set_font_size(font_size);

    // Split by " / " for alternate bindings
    let alternatives: Vec<&str> = key_str.split(" / ").collect();

    for (alt_idx, alt) in alternatives.iter().enumerate() {
        if alt_idx > 0 {
            // Add separator "/" width
            ctx.select_font_face(
                font_family,
                cairo::FontSlant::Normal,
                cairo::FontWeight::Normal,
            );
            ctx.set_font_size(font_size);
            let slash_ext = ctx
                .text_extents("/")
                .unwrap_or_else(|_| fallback_text_extents(font_size, "/"));
            total_width += separator_gap * 2.0 + slash_ext.width();
        }

        // Split by "+" for key combinations
        let keys: Vec<&str> = alt.split('+').collect();
        for (key_idx, key) in keys.iter().enumerate() {
            if key_idx > 0 {
                // Add "+" separator width (matches draw_key_combo)
                ctx.select_font_face(
                    font_family,
                    cairo::FontSlant::Normal,
                    cairo::FontWeight::Bold,
                );
                ctx.set_font_size(font_size * 0.9);
                let plus_ext = ctx
                    .text_extents("+")
                    .unwrap_or_else(|_| fallback_text_extents(font_size, "+"));
                total_width += 6.0 + plus_ext.width();
            }

            ctx.select_font_face(
                font_family,
                cairo::FontSlant::Normal,
                cairo::FontWeight::Bold,
            );
            ctx.set_font_size(font_size);
            let ext = ctx
                .text_extents(key.trim())
                .unwrap_or_else(|_| fallback_text_extents(font_size, key.trim()));
            total_width += ext.width() + keycap_padding_x * 2.0 + key_gap;
        }
    }

    total_width - key_gap // Remove trailing gap
}

/// Draw a key combination string with keycap styling, returns total width
fn draw_key_combo(
    ctx: &cairo::Context,
    x: f64,
    baseline: f64,
    key_str: &str,
    style: &KeyComboStyle<'_>,
) -> f64 {
    let mut cursor_x = x;
    let key_gap = 5.0;
    let separator_gap = 6.0;

    // Split by " / " for alternate bindings
    let alternatives: Vec<&str> = key_str.split(" / ").collect();

    for (alt_idx, alt) in alternatives.iter().enumerate() {
        if alt_idx > 0 {
            // Draw separator "/"
            ctx.select_font_face(
                style.font_family,
                cairo::FontSlant::Normal,
                cairo::FontWeight::Normal,
            );
            ctx.set_font_size(style.font_size);
            ctx.set_source_rgba(
                style.separator_color[0],
                style.separator_color[1],
                style.separator_color[2],
                style.separator_color[3],
            );
            cursor_x += separator_gap;
            ctx.move_to(cursor_x, baseline);
            let _ = ctx.show_text("/");
            let slash_ext = ctx
                .text_extents("/")
                .unwrap_or_else(|_| fallback_text_extents(style.font_size, "/"));
            cursor_x += slash_ext.width() + separator_gap;
        }

        // Split by "+" for key combinations
        let keys: Vec<&str> = alt.split('+').collect();
        for (key_idx, key) in keys.iter().enumerate() {
            if key_idx > 0 {
                // Draw "+" separator (bold and visible)
                ctx.select_font_face(
                    style.font_family,
                    cairo::FontSlant::Normal,
                    cairo::FontWeight::Bold,
                );
                ctx.set_font_size(style.font_size * 0.9);
                ctx.set_source_rgba(
                    style.separator_color[0],
                    style.separator_color[1],
                    style.separator_color[2],
                    0.85,
                );
                cursor_x += 3.0;
                ctx.move_to(cursor_x, baseline);
                let _ = ctx.show_text("+");
                let plus_ext = ctx
                    .text_extents("+")
                    .unwrap_or_else(|_| fallback_text_extents(style.font_size, "+"));
                cursor_x += plus_ext.width() + 3.0;
            }

            let cap_width = draw_keycap(
                ctx,
                cursor_x,
                baseline,
                key.trim(),
                style.font_family,
                style.font_size,
                style.text_color,
            );
            cursor_x += cap_width + key_gap;
        }
    }

    cursor_x - x - key_gap // Return total width minus trailing gap
}

/// Render help overlay showing all keybindings
#[allow(clippy::too_many_arguments)]
pub fn render_help_overlay(
    ctx: &cairo::Context,
    style: &crate::config::HelpOverlayStyle,
    screen_width: u32,
    screen_height: u32,
    frozen_enabled: bool,
    view: HelpOverlayView,
    page_index: usize,
    page_prev_label: &str,
    page_next_label: &str,
    search_query: &str,
    context_filter: bool,
    board_enabled: bool,
    capture_enabled: bool,
    scroll_offset: f64,
) -> f64 {
    type IconFn = fn(&cairo::Context, f64, f64, f64);

    #[derive(Clone)]
    struct Row {
        key: String,
        action: &'static str,
    }

    #[derive(Clone)]
    struct Badge {
        label: &'static str,
        color: [f64; 3],
    }

    #[derive(Clone)]
    struct Section {
        title: &'static str,
        rows: Vec<Row>,
        badges: Vec<Badge>,
        icon: Option<IconFn>,
    }

    struct MeasuredSection {
        section: Section,
        width: f64,
        height: f64,
        key_column_width: f64,
    }

    fn row<T: Into<String>>(key: T, action: &'static str) -> Row {
        Row {
            key: key.into(),
            action,
        }
    }

    fn grid_width_for_columns(
        sections: &[MeasuredSection],
        columns: usize,
        column_gap: f64,
    ) -> f64 {
        if columns == 0 || sections.is_empty() {
            return 0.0;
        }

        let mut max_width: f64 = 0.0;
        for chunk in sections.chunks(columns) {
            let mut width = 0.0;
            for (index, section) in chunk.iter().enumerate() {
                if index > 0 {
                    width += column_gap;
                }
                width += section.width;
            }
            max_width = max_width.max(width);
        }

        max_width
    }

    fn draw_highlight(
        ctx: &cairo::Context,
        x: f64,
        baseline: f64,
        font_size: f64,
        weight: cairo::FontWeight,
        text: &str,
        font_family: &str,
        range: (usize, usize),
        color: [f64; 4],
    ) {
        let (start, end) = range;
        if start >= end || end > text.len() {
            return;
        }
        if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            return;
        }
        let prefix = &text[..start];
        let matched = &text[start..end];
        if matched.is_empty() {
            return;
        }

        let prefix_extents = text_extents_for(
            ctx,
            font_family,
            cairo::FontSlant::Normal,
            weight,
            font_size,
            prefix,
        );
        let match_extents = text_extents_for(
            ctx,
            font_family,
            cairo::FontSlant::Normal,
            weight,
            font_size,
            matched,
        );

        let pad_x = 2.0;
        let pad_y = 2.0;
        let highlight_x = x + prefix_extents.width() - pad_x;
        let highlight_y = baseline + match_extents.y_bearing() - pad_y;
        let highlight_width = match_extents.width() + pad_x * 2.0;
        let highlight_height = match_extents.height() + pad_y * 2.0;

        ctx.set_source_rgba(color[0], color[1], color[2], color[3]);
        ctx.rectangle(highlight_x, highlight_y, highlight_width, highlight_height);
        let _ = ctx.fill();
    }

    fn draw_key_combo_highlight(
        ctx: &cairo::Context,
        x: f64,
        baseline: f64,
        font_size: f64,
        key_width: f64,
        color: [f64; 4],
    ) {
        if key_width <= 0.0 {
            return;
        }

        let padding_y = 4.0;
        let pad_x = 3.0;
        let pad_y = 3.0;
        let highlight_x = x - pad_x;
        let highlight_y = baseline - font_size - padding_y - pad_y;
        let highlight_width = key_width + pad_x * 2.0;
        let highlight_height = font_size + padding_y * 2.0 + pad_y * 2.0;

        ctx.set_source_rgba(color[0], color[1], color[2], color[3]);
        draw_rounded_rect(
            ctx,
            highlight_x,
            highlight_y,
            highlight_width,
            highlight_height,
            6.0,
        );
        let _ = ctx.fill();
    }

    fn draw_segmented_text(
        ctx: &cairo::Context,
        x: f64,
        baseline: f64,
        font_size: f64,
        weight: cairo::FontWeight,
        font_family: &str,
        segments: &[(String, [f64; 4])],
    ) {
        let mut cursor_x = x;
        for (text, color) in segments {
            ctx.set_source_rgba(color[0], color[1], color[2], color[3]);
            ctx.move_to(cursor_x, baseline);
            let _ = ctx.show_text(text);

            let extents = text_extents_for(
                ctx,
                font_family,
                cairo::FontSlant::Normal,
                weight,
                font_size,
                text,
            );
            cursor_x += extents.width();
        }
    }

    fn ellipsize_to_fit(
        ctx: &cairo::Context,
        text: &str,
        font_family: &str,
        font_size: f64,
        weight: cairo::FontWeight,
        max_width: f64,
    ) -> String {
        let extents = text_extents_for(
            ctx,
            font_family,
            cairo::FontSlant::Normal,
            weight,
            font_size,
            text,
        );
        if extents.width() <= max_width {
            return text.to_string();
        }

        let ellipsis = "…";
        let ellipsis_extents = text_extents_for(
            ctx,
            font_family,
            cairo::FontSlant::Normal,
            weight,
            font_size,
            ellipsis,
        );
        if ellipsis_extents.width() > max_width {
            return String::new();
        }

        let mut end = text.len();
        while end > 0 {
            if !text.is_char_boundary(end) {
                end -= 1;
                continue;
            }
            let candidate = format!("{}{}", &text[..end], ellipsis);
            let candidate_extents = text_extents_for(
                ctx,
                font_family,
                cairo::FontSlant::Normal,
                weight,
                font_size,
                &candidate,
            );
            if candidate_extents.width() <= max_width {
                return candidate;
            }
            end -= 1;
        }

        ellipsis.to_string()
    }

    fn find_match_range(haystack: &str, needle_lower: &str) -> Option<(usize, usize)> {
        if needle_lower.is_empty() {
            return None;
        }
        let haystack_lower = haystack.to_ascii_lowercase();
        haystack_lower
            .find(needle_lower)
            .map(|start| (start, start + needle_lower.len()))
    }

    fn row_matches(row: &Row, needle_lower: &str) -> bool {
        find_match_range(&row.key, needle_lower).is_some()
            || find_match_range(row.action, needle_lower).is_some()
    }

    let search_query = search_query.trim();
    let search_active = !search_query.is_empty();
    let search_lower = search_query.to_ascii_lowercase();
    let help_font_family = resolve_help_font_family(&style.font_family);

    let page_count = view.page_count().max(1);
    let page_index = page_index.min(page_count - 1);
    let view_label = match view {
        HelpOverlayView::Quick => "Essentials",
        HelpOverlayView::Full => "Complete",
    };

    let board_modes_section = (!context_filter || board_enabled).then(|| Section {
        title: "Board Modes",
        rows: vec![
            row("Ctrl+W", "Toggle Whiteboard"),
            row("Ctrl+B", "Toggle Blackboard"),
            row("Ctrl+Shift+T", "Return to Transparent"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_settings),
    });

    let pages_section = Section {
        title: "Pages",
        rows: vec![
            row(page_prev_label, "Previous page"),
            row(page_next_label, "Next page"),
            row("Ctrl+Alt+N", "New page"),
            row("Ctrl+Alt+D", "Duplicate page"),
            row("Ctrl+Alt+Delete", "Delete page"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_file),
    };

    let drawing_section = Section {
        title: "Drawing",
        rows: vec![
            row("P", "Freehand pen"),
            row("Shift+Drag", "Line"),
            row("Ctrl+Drag", "Rectangle"),
            row("Tab+Drag", "Circle"),
            row("Ctrl+Shift+Drag", "Arrow"),
            row("Ctrl+Alt+H", "Highlight"),
            row("Ctrl+Alt+M", "Marker"),
            row("Ctrl+Alt+E", "Eraser"),
            row("+ / -", "Adjust thickness"),
        ],
        badges: vec![
            Badge {
                label: "R",
                color: [0.95, 0.41, 0.38],
            },
            Badge {
                label: "G",
                color: [0.46, 0.82, 0.45],
            },
            Badge {
                label: "B",
                color: [0.32, 0.58, 0.92],
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
        icon: Some(toolbar_icons::draw_icon_pen),
    };

    let selection_section = Section {
        title: "Selection",
        rows: vec![
            row("S", "Selection tool"),
            row("Ctrl+A", "Select all"),
            row("Ctrl+D", "Deselect"),
            row("Ctrl+C", "Copy selection"),
            row("Ctrl+V", "Paste selection"),
            row("Delete", "Delete selection"),
            row("Ctrl+Alt+P", "Selection properties"),
            row("Shift+Scroll", "Adjust font size"),
        ],
        badges: vec![
            Badge {
                label: "F",
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
        icon: Some(toolbar_icons::draw_icon_select),
    };

    let pen_text_section = Section {
        title: "Pen & Text",
        rows: vec![
            row("T", "Text tool"),
            row("Shift+T", "Sticky note"),
            row("Shift+Scroll", "Adjust font size"),
            row("Ctrl+Alt+F", "Toggle fill"),
            row("Ctrl+Alt+B", "Text background"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_text),
    };

    let zoom_section = Section {
        title: "Zoom",
        rows: vec![
            row("Ctrl+Alt+Scroll", "Zoom in/out"),
            row("Ctrl+Alt+0", "Reset zoom"),
            row("Ctrl+Alt+L", "Lock zoom"),
            row("Ctrl+Alt+Arrow", "Pan view"),
            row("Middle drag", "Pan view"),
            row("Ctrl+Alt+R", "Refresh zoom capture"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_zoom_in),
    };

    let mut action_rows = vec![
        row("E", "Clear frame"),
        row("Ctrl+Z", "Undo"),
        row("Ctrl+Shift+H", "Toggle click highlight"),
        row("Right Click / Shift+F10", "Context menu"),
        row("Escape / Ctrl+Q", "Exit"),
        row("F1 / F10", "Toggle help"),
        row("F2 / F9", "Toggle toolbar"),
        row("F11", "Open configurator"),
        row("F4 / F12", "Toggle status bar"),
    ];
    if frozen_enabled {
        action_rows.push(row("Ctrl+Shift+F", "Freeze/unfreeze active monitor"));
    }
    let actions_section = Section {
        title: "Actions",
        rows: action_rows,
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_undo),
    };

    let screenshots_section = (!context_filter || capture_enabled).then(|| Section {
        title: "Screenshots",
        rows: vec![
            row("Ctrl+C", "Full screen → clipboard"),
            row("Ctrl+S", "Full screen → file"),
            row("Ctrl+Shift+C", "Region → clipboard"),
            row("Ctrl+Shift+S", "Region → file"),
            row("Ctrl+Shift+O", "Active window (Hyprland)"),
            row("Ctrl+Shift+I", "Selection (capture defaults)"),
            row("Ctrl+Alt+O", "Open capture folder"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_save),
    });

    let mut all_sections = Vec::new();
    if let Some(section) = board_modes_section.clone() {
        all_sections.push(section);
    }
    all_sections.push(actions_section.clone());
    all_sections.push(drawing_section.clone());
    all_sections.push(pen_text_section.clone());
    all_sections.push(zoom_section.clone());
    all_sections.push(selection_section.clone());
    all_sections.push(pages_section.clone());
    if let Some(section) = screenshots_section.clone() {
        all_sections.push(section);
    }

    let mut page1_sections = Vec::new();
    if let Some(section) = board_modes_section {
        page1_sections.push(section);
    }
    page1_sections.push(actions_section);
    page1_sections.push(drawing_section);
    page1_sections.push(pen_text_section);

    let mut page2_sections = vec![pages_section, zoom_section, selection_section];
    if let Some(section) = screenshots_section {
        page2_sections.push(section);
    }

    let sections = if search_active {
        let mut filtered = Vec::new();
        for mut section in all_sections {
            let title_match = find_match_range(section.title, &search_lower).is_some();
            if !title_match {
                section.rows.retain(|row| row_matches(row, &search_lower));
            }
            if !section.rows.is_empty() {
                filtered.push(section);
            }
        }

        if filtered.is_empty() {
            filtered.push(Section {
                title: "No results",
                rows: vec![
                    row("", "Try: zoom, page, selection, capture"),
                    row("", "Tip: search by key or action name"),
                ],
                badges: Vec::new(),
                icon: None,
            });
        }

        filtered
    } else if matches!(view, HelpOverlayView::Quick) || page_index == 0 {
        page1_sections
    } else {
        page2_sections
    };

    let title_text = "Wayscriber Controls";
    let commit_hash = option_env!("WAYSCRIBER_GIT_HASH").unwrap_or("unknown");
    let version_line = format!(
        "Wayscriber {} ({})  •  F11 → Open Configurator",
        env!("CARGO_PKG_VERSION"),
        commit_hash
    );
    let note_text_base = "Note: Each board mode has independent pages";
    let close_hint_text = "F1 / Esc to close";

    let body_font_size = style.font_size;
    let heading_font_size = body_font_size + 6.0;
    let title_font_size = heading_font_size + 6.0;
    let subtitle_font_size = body_font_size;
    let row_extra_gap = 4.0;
    let row_line_height = style.line_height.max(body_font_size + 8.0) + row_extra_gap;
    let heading_line_height = heading_font_size + 10.0;
    let heading_icon_size = heading_font_size * 0.9;
    let heading_icon_gap = 10.0;
    let row_gap_after_heading = 10.0;
    let key_desc_gap = 24.0;
    let row_gap = 36.0;
    let column_gap = 56.0;
    let section_card_padding = 14.0;
    let section_card_radius = 10.0;
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
    let nav_line_gap = 6.0;
    let nav_bottom_spacing = 18.0;
    let extra_line_gap = 6.0;
    let extra_line_bottom_spacing = 18.0;
    let columns_bottom_spacing = 28.0;
    let max_box_width = screen_width as f64 * 0.92;
    let max_box_height = screen_height as f64 * 0.92;

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

    // Warmer, softer accent gold
    let accent_color = [0.91, 0.73, 0.42, 1.0];
    let accent_muted = [accent_color[0], accent_color[1], accent_color[2], 0.85];
    let highlight_color = [accent_color[0], accent_color[1], accent_color[2], 0.22];
    let heading_icon_color = [accent_color[0], accent_color[1], accent_color[2], 0.9];
    let nav_key_color = [0.58, 0.82, 0.88, 1.0];
    let search_color = [0.92, 0.58, 0.28, 1.0];
    let subtitle_color = [0.58, 0.62, 0.72, 1.0];
    let section_card_bg = [1.0, 1.0, 1.0, 0.04];
    let section_card_border = [1.0, 1.0, 1.0, 0.08];
    let body_text_color = style.text_color;
    let description_color = [
        lerp(body_text_color[0], subtitle_color[0], 0.35),
        lerp(body_text_color[1], subtitle_color[1], 0.35),
        lerp(body_text_color[2], subtitle_color[2], 0.35),
        body_text_color[3],
    ];
    let note_color = [subtitle_color[0], subtitle_color[1], subtitle_color[2], 0.9];
    let key_combo_style = KeyComboStyle {
        font_family: help_font_family.as_str(),
        font_size: body_font_size,
        text_color: accent_muted,
        separator_color: subtitle_color,
    };

    let nav_text_primary = if !search_active && matches!(view, HelpOverlayView::Full) {
        format!(
            "{} view  •  Page {}/{}",
            view_label,
            page_index + 1,
            page_count
        )
    } else {
        format!("{} view", view_label)
    };
    let nav_separator = "   •   ";
    let nav_secondary_segments: Vec<(String, [f64; 4])> = if search_active {
        vec![
            ("Esc".to_string(), nav_key_color),
            (": Close".to_string(), subtitle_color),
            (nav_separator.to_string(), subtitle_color),
            ("Backspace".to_string(), nav_key_color),
            (": Remove".to_string(), subtitle_color),
            (nav_separator.to_string(), subtitle_color),
            ("Tab".to_string(), nav_key_color),
            (": Toggle view".to_string(), subtitle_color),
        ]
    } else if page_count > 1 {
        vec![
            ("Switch pages:  ".to_string(), subtitle_color),
            (
                "Left/Right, PageUp/PageDown, Home/End".to_string(),
                nav_key_color,
            ),
        ]
    } else {
        vec![
            ("Tab".to_string(), nav_key_color),
            (": Toggle view".to_string(), subtitle_color),
        ]
    };
    // Third nav line for multi-page view (separate from switch pages)
    let nav_tertiary_segments: Option<Vec<(String, [f64; 4])>> = if !search_active && page_count > 1
    {
        Some(vec![
            ("Tab".to_string(), nav_key_color),
            (": Toggle view".to_string(), subtitle_color),
        ])
    } else {
        None
    };
    let nav_text_secondary: String = nav_secondary_segments
        .iter()
        .map(|(text, _)| text.as_str())
        .collect();
    let mut measured_sections = Vec::with_capacity(sections.len());
    for section in sections {
        let mut key_max_width: f64 = 0.0;
        for row in &section.rows {
            if row.key.is_empty() {
                continue;
            }
            // Measure with keycap styling padding
            let key_width = measure_key_combo(
                ctx,
                row.key.as_str(),
                help_font_family.as_str(),
                body_font_size,
            );
            key_max_width = key_max_width.max(key_width);
        }

        let mut section_width: f64 = 0.0;
        let mut section_height: f64 = 0.0;

        let heading_extents = text_extents_for(
            ctx,
            help_font_family.as_str(),
            cairo::FontSlant::Normal,
            cairo::FontWeight::Bold,
            heading_font_size,
            section.title,
        );
        let mut heading_width = heading_extents.width();
        if section.icon.is_some() {
            heading_width += heading_icon_size + heading_icon_gap;
        }
        section_width = section_width.max(heading_width);
        section_height += heading_line_height;

        if !section.rows.is_empty() {
            section_height += row_gap_after_heading;
            for row in &section.rows {
                let desc_extents = text_extents_for(
                    ctx,
                    help_font_family.as_str(),
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
                    help_font_family.as_str(),
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
            width: section_width + section_card_padding * 2.0,
            height: section_height + section_card_padding * 2.0,
            key_column_width: key_max_width,
        });
    }

    let max_content_width = (max_box_width - style.padding * 2.0).max(0.0);
    let max_columns = measured_sections.len().clamp(1, 3);
    let base_columns = if screen_width < 1200 {
        1
    } else if screen_width > 1920 {
        3
    } else {
        2
    };
    let mut columns = base_columns.min(max_columns).max(1);
    while columns > 1 {
        let grid_width = grid_width_for_columns(&measured_sections, columns, column_gap);
        if grid_width <= max_content_width {
            break;
        }
        columns -= 1;
    }

    let mut rows: Vec<Vec<MeasuredSection>> = Vec::new();
    if measured_sections.is_empty() {
        rows.push(Vec::new());
    } else {
        let mut current_row = Vec::new();
        for section in measured_sections {
            current_row.push(section);
            if current_row.len() == columns {
                rows.push(current_row);
                current_row = Vec::new();
            }
        }
        if !current_row.is_empty() {
            rows.push(current_row);
        }
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
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        title_font_size,
        title_text,
    );
    let subtitle_extents = text_extents_for(
        ctx,
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        subtitle_font_size,
        &version_line,
    );
    let nav_font_size = (body_font_size - 1.0).max(12.0);
    let nav_primary_extents = text_extents_for(
        ctx,
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        nav_font_size,
        &nav_text_primary,
    );
    let nav_secondary_extents = text_extents_for(
        ctx,
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        nav_font_size,
        &nav_text_secondary,
    );
    let nav_tertiary_text: String = nav_tertiary_segments
        .as_ref()
        .map(|segs| segs.iter().map(|(t, _)| t.as_str()).collect())
        .unwrap_or_default();
    let nav_tertiary_extents = if nav_tertiary_segments.is_some() {
        Some(text_extents_for(
            ctx,
            help_font_family.as_str(),
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
            nav_font_size,
            &nav_tertiary_text,
        ))
    } else {
        None
    };
    let max_search_width = (screen_width as f64 * 0.9 - style.padding * 2.0).max(0.0);
    let search_text = if search_active {
        let prefix = "Search: ";
        let prefix_extents = text_extents_for(
            ctx,
            help_font_family.as_str(),
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
            nav_font_size,
            prefix,
        );
        let max_query_width = (max_search_width - prefix_extents.width()).max(0.0);
        let query_display = ellipsize_to_fit(
            ctx,
            search_query,
            help_font_family.as_str(),
            nav_font_size,
            cairo::FontWeight::Normal,
            max_query_width,
        );
        Some(format!("{}{}", prefix, query_display))
    } else {
        None
    };
    let search_hint_text = (!search_active).then(|| "Type to search".to_string());
    let extra_line_text = search_text.as_deref().or(search_hint_text.as_deref());
    let extra_line_extents = extra_line_text.map(|text| {
        text_extents_for(
            ctx,
            help_font_family.as_str(),
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
            nav_font_size,
            text,
        )
    });
    let note_font_size = (body_font_size - 2.0).max(12.0);
    let close_hint_extents = text_extents_for(
        ctx,
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        note_font_size,
        close_hint_text,
    );
    let note_to_close_gap = 12.0;

    let nav_tertiary_height = if nav_tertiary_segments.is_some() {
        nav_line_gap + nav_font_size
    } else {
        0.0
    };
    let nav_block_height = if extra_line_text.is_some() {
        nav_font_size * 2.0
            + nav_line_gap
            + nav_tertiary_height
            + extra_line_gap
            + nav_font_size
            + extra_line_bottom_spacing
    } else {
        nav_font_size * 2.0 + nav_line_gap + nav_tertiary_height + nav_bottom_spacing
    };
    let header_height = accent_line_height
        + accent_line_bottom_spacing
        + title_font_size
        + title_bottom_spacing
        + subtitle_font_size
        + subtitle_bottom_spacing
        + nav_block_height;
    let footer_height =
        columns_bottom_spacing + note_font_size + note_to_close_gap + note_font_size;
    let content_height = header_height + grid_height + footer_height;
    let max_inner_height = (max_box_height - style.padding * 2.0).max(0.0);
    let inner_height = content_height.min(max_inner_height);
    let grid_view_height = (inner_height - header_height - footer_height).max(0.0);
    let scroll_max = (grid_height - grid_view_height).max(0.0);
    let scroll_offset = scroll_offset.clamp(0.0, scroll_max);
    let note_text = if scroll_max > 0.0 {
        format!("{}  •  Scroll: Mouse wheel", note_text_base)
    } else {
        note_text_base.to_string()
    };
    let note_extents = text_extents_for(
        ctx,
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        note_font_size,
        note_text.as_str(),
    );

    let mut content_width = grid_width
        .max(title_extents.width())
        .max(subtitle_extents.width())
        .max(nav_primary_extents.width())
        .max(nav_secondary_extents.width())
        .max(
            nav_tertiary_extents
                .as_ref()
                .map(|e| e.width())
                .unwrap_or(0.0),
        )
        .max(note_extents.width())
        .max(close_hint_extents.width());
    // Don't let search text expand the overlay - it will be clamped/elided
    if rows.is_empty() {
        content_width = content_width
            .max(title_extents.width())
            .max(subtitle_extents.width());
    }
    // Ensure minimum width for search box
    content_width = content_width.max(300.0);
    let box_width = content_width + style.padding * 2.0;
    let box_height = inner_height + style.padding * 2.0;

    let box_x = (screen_width as f64 - box_width) / 2.0;
    let box_y = (screen_height as f64 - box_height) / 2.0;

    // Dim background behind overlay
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.55);
    ctx.rectangle(0.0, 0.0, screen_width as f64, screen_height as f64);
    let _ = ctx.fill();

    let corner_radius = 16.0;

    // Drop shadow (layered for softer effect)
    let shadow_offset = 12.0;
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.25);
    draw_rounded_rect(
        ctx,
        box_x + shadow_offset + 4.0,
        box_y + shadow_offset + 4.0,
        box_width,
        box_height,
        corner_radius,
    );
    let _ = ctx.fill();
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    draw_rounded_rect(
        ctx,
        box_x + shadow_offset,
        box_y + shadow_offset,
        box_width,
        box_height,
        corner_radius,
    );
    let _ = ctx.fill();

    // Background gradient
    let gradient = cairo::LinearGradient::new(box_x, box_y, box_x, box_y + box_height);
    gradient.add_color_stop_rgba(0.0, bg_top[0], bg_top[1], bg_top[2], bg_top[3]);
    gradient.add_color_stop_rgba(1.0, bg_bottom[0], bg_bottom[1], bg_bottom[2], bg_bottom[3]);
    let _ = ctx.set_source(&gradient);
    draw_rounded_rect(ctx, box_x, box_y, box_width, box_height, corner_radius);
    let _ = ctx.fill();

    // Border
    let [br, bg, bb, ba] = style.border_color;
    ctx.set_source_rgba(br, bg, bb, ba);
    ctx.set_line_width(style.border_width);
    draw_rounded_rect(ctx, box_x, box_y, box_width, box_height, corner_radius);
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
    ctx.select_font_face(
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
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
    ctx.select_font_face(
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
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

    // Navigation lines
    ctx.select_font_face(
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    ctx.set_font_size(nav_font_size);
    ctx.set_source_rgba(
        subtitle_color[0],
        subtitle_color[1],
        subtitle_color[2],
        subtitle_color[3],
    );
    let nav_baseline = cursor_y + nav_font_size;
    ctx.move_to(inner_x, nav_baseline);
    let _ = ctx.show_text(&nav_text_primary);
    cursor_y += nav_font_size + nav_line_gap;
    let nav_secondary_baseline = cursor_y + nav_font_size;
    draw_segmented_text(
        ctx,
        inner_x,
        nav_secondary_baseline,
        nav_font_size,
        cairo::FontWeight::Normal,
        help_font_family.as_str(),
        &nav_secondary_segments,
    );
    cursor_y += nav_font_size;

    // Draw tertiary nav line (for multi-page Complete view)
    if let Some(ref tertiary_segments) = nav_tertiary_segments {
        cursor_y += nav_line_gap;
        let nav_tertiary_baseline = cursor_y + nav_font_size;
        draw_segmented_text(
            ctx,
            inner_x,
            nav_tertiary_baseline,
            nav_font_size,
            cairo::FontWeight::Normal,
            help_font_family.as_str(),
            tertiary_segments,
        );
        cursor_y += nav_font_size;
    }

    if let Some(extra_line_text) = extra_line_text {
        cursor_y += extra_line_gap;

        // Draw search input field style
        let search_padding_x = 12.0;
        let search_padding_y = 6.0;
        let search_box_height = nav_font_size + search_padding_y * 2.0;
        // Clamp search box to available width
        let search_box_width = inner_width.min(if let Some(ext) = &extra_line_extents {
            (ext.width() + search_padding_x * 2.0 + 20.0).min(inner_width)
        } else {
            200.0
        });
        let search_box_radius = 6.0;

        // Search box background
        draw_rounded_rect(
            ctx,
            inner_x,
            cursor_y,
            search_box_width,
            search_box_height,
            search_box_radius,
        );
        ctx.set_source_rgba(0.0, 0.0, 0.0, 0.3);
        let _ = ctx.fill_preserve();
        ctx.set_source_rgba(search_color[0], search_color[1], search_color[2], 0.5);
        ctx.set_line_width(1.0);
        let _ = ctx.stroke();

        // Search text with clipping
        let extra_line_baseline = cursor_y + search_padding_y + nav_font_size;
        let max_text_width = search_box_width - search_padding_x * 2.0;

        let display_text = ellipsize_to_fit(
            ctx,
            extra_line_text,
            help_font_family.as_str(),
            nav_font_size,
            cairo::FontWeight::Normal,
            max_text_width,
        );

        ctx.set_source_rgba(
            search_color[0],
            search_color[1],
            search_color[2],
            search_color[3],
        );
        ctx.move_to(inner_x + search_padding_x, extra_line_baseline);
        let _ = ctx.show_text(&display_text);
        cursor_y += search_box_height + extra_line_bottom_spacing;
    } else {
        cursor_y += nav_bottom_spacing;
    }

    let grid_start_y = cursor_y;

    if grid_view_height > 0.0 {
        let _ = ctx.save();
        ctx.rectangle(inner_x, grid_start_y, inner_width, grid_view_height);
        ctx.clip();

        let mut row_y = grid_start_y - scroll_offset;
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

                let section = &measured.section;

                // Draw section card background
                draw_rounded_rect(
                    ctx,
                    section_x,
                    row_y,
                    measured.width,
                    measured.height,
                    section_card_radius,
                );
                ctx.set_source_rgba(
                    section_card_bg[0],
                    section_card_bg[1],
                    section_card_bg[2],
                    section_card_bg[3],
                );
                let _ = ctx.fill_preserve();
                ctx.set_source_rgba(
                    section_card_border[0],
                    section_card_border[1],
                    section_card_border[2],
                    section_card_border[3],
                );
                ctx.set_line_width(1.0);
                let _ = ctx.stroke();

                // Content starts inside card padding
                let content_x = section_x + section_card_padding;
                let mut section_y = row_y + section_card_padding;
                let desc_x = content_x + measured.key_column_width + key_desc_gap;

                ctx.select_font_face(
                    help_font_family.as_str(),
                    cairo::FontSlant::Normal,
                    cairo::FontWeight::Bold,
                );
                ctx.set_font_size(heading_font_size);
                ctx.set_source_rgba(
                    accent_color[0],
                    accent_color[1],
                    accent_color[2],
                    accent_color[3],
                );
                let mut heading_text_x = content_x;
                if let Some(icon) = section.icon {
                    let icon_y = section_y + (heading_line_height - heading_icon_size) * 0.5;
                    let _ = ctx.save();
                    ctx.set_source_rgba(
                        heading_icon_color[0],
                        heading_icon_color[1],
                        heading_icon_color[2],
                        heading_icon_color[3],
                    );
                    icon(ctx, content_x, icon_y, heading_icon_size);
                    let _ = ctx.restore();
                    heading_text_x += heading_icon_size + heading_icon_gap;
                }
                let heading_baseline = section_y + heading_font_size;
                ctx.move_to(heading_text_x, heading_baseline);
                let _ = ctx.show_text(section.title);
                section_y += heading_line_height;

                if !section.rows.is_empty() {
                    section_y += row_gap_after_heading;
                    for row_data in &section.rows {
                        let baseline = section_y + body_font_size;

                        let key_match = search_active
                            && find_match_range(&row_data.key, &search_lower).is_some();
                        if key_match && !row_data.key.is_empty() {
                            let key_width = measure_key_combo(
                                ctx,
                                row_data.key.as_str(),
                                help_font_family.as_str(),
                                body_font_size,
                            );
                            draw_key_combo_highlight(
                                ctx,
                                content_x,
                                baseline,
                                body_font_size,
                                key_width,
                                highlight_color,
                            );
                        }
                        if search_active
                            && let Some(range) = find_match_range(row_data.action, &search_lower)
                        {
                            draw_highlight(
                                ctx,
                                desc_x,
                                baseline,
                                body_font_size,
                                cairo::FontWeight::Normal,
                                row_data.action,
                                help_font_family.as_str(),
                                range,
                                highlight_color,
                            );
                        }

                        // Draw key with keycap styling
                        let _ = draw_key_combo(
                            ctx,
                            content_x,
                            baseline,
                            row_data.key.as_str(),
                            &key_combo_style,
                        );

                        // Draw action description
                        ctx.select_font_face(
                            help_font_family.as_str(),
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
                    let mut badge_x = content_x;

                    for (badge_index, badge) in section.badges.iter().enumerate() {
                        if badge_index > 0 {
                            badge_x += badge_gap;
                        }

                        ctx.new_path();
                        let badge_text_extents = text_extents_for(
                            ctx,
                            help_font_family.as_str(),
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

                        ctx.select_font_face(
                            help_font_family.as_str(),
                            cairo::FontSlant::Normal,
                            cairo::FontWeight::Bold,
                        );
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

        let _ = ctx.restore();
    }

    cursor_y = grid_start_y + grid_view_height + columns_bottom_spacing;

    // Note
    ctx.select_font_face(
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    ctx.set_font_size(note_font_size);
    ctx.set_source_rgba(note_color[0], note_color[1], note_color[2], note_color[3]);
    let note_x = inner_x + (inner_width - note_extents.width()) / 2.0;
    let note_baseline = cursor_y + note_font_size;
    ctx.move_to(note_x, note_baseline);
    let _ = ctx.show_text(note_text.as_str());
    cursor_y += note_font_size + note_to_close_gap;

    // Close hint
    ctx.set_source_rgba(subtitle_color[0], subtitle_color[1], subtitle_color[2], 0.7);
    let close_x = inner_x + (inner_width - close_hint_extents.width()) / 2.0;
    let close_baseline = cursor_y + note_font_size;
    ctx.move_to(close_x, close_baseline);
    let _ = ctx.show_text(close_hint_text);

    scroll_max
}
