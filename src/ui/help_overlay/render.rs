use crate::input::HelpOverlayView;

use super::super::primitives::{draw_rounded_rect, text_extents_for};
use super::fonts::resolve_help_font_family;
use super::grid::{GridColors, GridStyle, draw_sections_grid};
use super::keycaps::KeyComboStyle;
use super::layout::{build_grid, measure_sections};
use super::nav::{NavDrawStyle, build_nav_state, draw_nav};
use super::sections::{build_section_sets, filter_sections_for_search};

const BULLET: &str = "\u{2022}";
const ARROW: &str = "\u{2192}";

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

    let section_sets = build_section_sets(
        frozen_enabled,
        context_filter,
        board_enabled,
        capture_enabled,
        page_prev_label,
        page_next_label,
    );

    let sections = if search_active {
        filter_sections_for_search(section_sets.all, &search_lower)
    } else if matches!(view, HelpOverlayView::Quick) || page_index == 0 {
        section_sets.page1
    } else {
        section_sets.page2
    };

    let title_text = "Wayscriber Controls";
    let commit_hash = option_env!("WAYSCRIBER_GIT_HASH").unwrap_or("unknown");
    let version_line = format!(
        "Wayscriber {} ({})  {}  F11 {} Open Configurator",
        env!("CARGO_PKG_VERSION"),
        commit_hash,
        BULLET,
        ARROW
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

    let nav_font_size = (body_font_size - 1.0).max(12.0);
    let max_search_width = (screen_width as f64 * 0.9 - style.padding * 2.0).max(0.0);
    let nav_state = build_nav_state(
        ctx,
        help_font_family.as_str(),
        view_label,
        view,
        search_active,
        page_index,
        page_count,
        search_query,
        nav_key_color,
        subtitle_color,
        nav_font_size,
        nav_line_gap,
        nav_bottom_spacing,
        extra_line_gap,
        extra_line_bottom_spacing,
        max_search_width,
    );

    let measured_sections = measure_sections(
        ctx,
        sections,
        help_font_family.as_str(),
        body_font_size,
        heading_font_size,
        heading_line_height,
        heading_icon_size,
        heading_icon_gap,
        row_line_height,
        row_gap_after_heading,
        key_desc_gap,
        badge_font_size,
        badge_padding_x,
        badge_gap,
        badge_height,
        badge_top_gap,
        section_card_padding,
    );

    let max_content_width = (max_box_width - style.padding * 2.0).max(0.0);
    let grid = build_grid(
        measured_sections,
        screen_width,
        max_content_width,
        column_gap,
        row_gap,
    );

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
    let header_height = accent_line_height
        + accent_line_bottom_spacing
        + title_font_size
        + title_bottom_spacing
        + subtitle_font_size
        + subtitle_bottom_spacing
        + nav_state.nav_block_height;
    let footer_height =
        columns_bottom_spacing + note_font_size + note_to_close_gap + note_font_size;
    let content_height = header_height + grid.grid_height + footer_height;
    let max_inner_height = (max_box_height - style.padding * 2.0).max(0.0);
    let inner_height = content_height.min(max_inner_height);
    let grid_view_height = (inner_height - header_height - footer_height).max(0.0);
    let scroll_max = (grid.grid_height - grid_view_height).max(0.0);
    let scroll_offset = scroll_offset.clamp(0.0, scroll_max);
    let note_text = if scroll_max > 0.0 {
        format!("{}  {}  Scroll: Mouse wheel", note_text_base, BULLET)
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

    let mut content_width = grid
        .grid_width
        .max(title_extents.width())
        .max(subtitle_extents.width())
        .max(nav_state.nav_primary_width)
        .max(nav_state.nav_secondary_width)
        .max(nav_state.nav_tertiary_width.unwrap_or(0.0))
        .max(note_extents.width())
        .max(close_hint_extents.width());
    // Don't let search text expand the overlay - it will be clamped/elided
    if grid.rows.is_empty() {
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

    let nav_draw_style = NavDrawStyle {
        font_family: help_font_family.as_str(),
        subtitle_color,
        search_color,
        nav_line_gap,
        nav_bottom_spacing,
        extra_line_gap,
        extra_line_bottom_spacing,
    };
    cursor_y = draw_nav(
        ctx,
        inner_x,
        cursor_y,
        inner_width,
        &nav_state,
        &nav_draw_style,
    );

    let grid_start_y = cursor_y;

    let grid_style = GridStyle {
        help_font_family: help_font_family.as_str(),
        body_font_size,
        heading_font_size,
        heading_line_height,
        heading_icon_size,
        heading_icon_gap,
        row_line_height,
        row_gap_after_heading,
        key_desc_gap,
        badge_font_size,
        badge_padding_x,
        badge_gap,
        badge_height,
        badge_corner_radius,
        badge_top_gap,
        section_card_padding,
        section_card_radius,
        row_gap,
        column_gap,
    };
    let grid_colors = GridColors {
        accent: accent_color,
        heading_icon: heading_icon_color,
        description: description_color,
        highlight: highlight_color,
        section_card_bg,
        section_card_border,
    };

    draw_sections_grid(
        ctx,
        &grid,
        grid_start_y,
        inner_x,
        inner_width,
        grid_view_height,
        scroll_offset,
        search_active,
        &search_lower,
        &grid_style,
        &grid_colors,
        &key_combo_style,
    );

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
