use crate::input::HelpOverlayView;

use super::super::primitives::text_extents_for;
use super::fonts::resolve_help_font_family;
use super::grid::{GridColors, GridStyle, draw_sections_grid};
use super::keycaps::KeyComboStyle;
use super::layout::{build_grid, measure_sections};
use super::nav::{NavDrawStyle, build_nav_state, draw_nav};
use super::sections::{build_section_sets, filter_sections_for_search};

mod frame;
mod metrics;
mod palette;

use frame::draw_overlay_frame;
use metrics::RenderMetrics;
use palette::RenderPalette;

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

    let metrics = RenderMetrics::from_style(style, screen_width, screen_height);
    let palette = RenderPalette::from_style(style);

    let key_combo_style = KeyComboStyle {
        font_family: help_font_family.as_str(),
        font_size: metrics.body_font_size,
        text_color: palette.accent_muted,
        separator_color: palette.subtitle,
    };

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
        palette.nav_key,
        palette.subtitle,
        metrics.nav_font_size,
        metrics.nav_line_gap,
        metrics.nav_bottom_spacing,
        metrics.extra_line_gap,
        metrics.extra_line_bottom_spacing,
        max_search_width,
    );

    let measured_sections = measure_sections(
        ctx,
        sections,
        help_font_family.as_str(),
        metrics.body_font_size,
        metrics.heading_font_size,
        metrics.heading_line_height,
        metrics.heading_icon_size,
        metrics.heading_icon_gap,
        metrics.row_line_height,
        metrics.row_gap_after_heading,
        metrics.key_desc_gap,
        metrics.badge_font_size,
        metrics.badge_padding_x,
        metrics.badge_gap,
        metrics.badge_height,
        metrics.badge_top_gap,
        metrics.section_card_padding,
    );

    let max_content_width = (metrics.max_box_width - style.padding * 2.0).max(0.0);
    let grid = build_grid(
        measured_sections,
        screen_width,
        max_content_width,
        metrics.column_gap,
        metrics.row_gap,
    );

    let title_extents = text_extents_for(
        ctx,
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        metrics.title_font_size,
        title_text,
    );
    let subtitle_extents = text_extents_for(
        ctx,
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        metrics.subtitle_font_size,
        &version_line,
    );
    let close_hint_extents = text_extents_for(
        ctx,
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        metrics.note_font_size,
        close_hint_text,
    );
    let note_to_close_gap = 12.0;
    let header_height = metrics.accent_line_height
        + metrics.accent_line_bottom_spacing
        + metrics.title_font_size
        + metrics.title_bottom_spacing
        + metrics.subtitle_font_size
        + metrics.subtitle_bottom_spacing
        + nav_state.nav_block_height;
    let footer_height =
        metrics.columns_bottom_spacing + metrics.note_font_size + note_to_close_gap + metrics.note_font_size;
    let content_height = header_height + grid.grid_height + footer_height;
    let max_inner_height = (metrics.max_box_height - style.padding * 2.0).max(0.0);
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
        metrics.note_font_size,
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

    draw_overlay_frame(
        ctx,
        style,
        &palette,
        screen_width,
        screen_height,
        box_x,
        box_y,
        box_width,
        box_height,
    );

    let inner_x = box_x + style.padding;
    let mut cursor_y = box_y + style.padding;
    let inner_width = box_width - style.padding * 2.0;

    // Accent line
    ctx.set_source_rgba(
        palette.accent[0],
        palette.accent[1],
        palette.accent[2],
        palette.accent[3],
    );
    ctx.rectangle(inner_x, cursor_y, inner_width, metrics.accent_line_height);
    let _ = ctx.fill();
    cursor_y += metrics.accent_line_height + metrics.accent_line_bottom_spacing;

    // Title
    ctx.select_font_face(
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    ctx.set_font_size(metrics.title_font_size);
    ctx.set_source_rgba(
        palette.body_text[0],
        palette.body_text[1],
        palette.body_text[2],
        palette.body_text[3],
    );
    let title_baseline = cursor_y + metrics.title_font_size;
    ctx.move_to(inner_x, title_baseline);
    let _ = ctx.show_text(title_text);
    cursor_y += metrics.title_font_size + metrics.title_bottom_spacing;

    // Subtitle / version line
    ctx.select_font_face(
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    ctx.set_font_size(metrics.subtitle_font_size);
    ctx.set_source_rgba(
        palette.subtitle[0],
        palette.subtitle[1],
        palette.subtitle[2],
        palette.subtitle[3],
    );
    let subtitle_baseline = cursor_y + metrics.subtitle_font_size;
    ctx.move_to(inner_x, subtitle_baseline);
    let _ = ctx.show_text(&version_line);
    cursor_y += metrics.subtitle_font_size + metrics.subtitle_bottom_spacing;

    let nav_draw_style = NavDrawStyle {
        font_family: help_font_family.as_str(),
        subtitle_color: palette.subtitle,
        search_color: palette.search,
        nav_line_gap: metrics.nav_line_gap,
        nav_bottom_spacing: metrics.nav_bottom_spacing,
        extra_line_gap: metrics.extra_line_gap,
        extra_line_bottom_spacing: metrics.extra_line_bottom_spacing,
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
        body_font_size: metrics.body_font_size,
        heading_font_size: metrics.heading_font_size,
        heading_line_height: metrics.heading_line_height,
        heading_icon_size: metrics.heading_icon_size,
        heading_icon_gap: metrics.heading_icon_gap,
        row_line_height: metrics.row_line_height,
        row_gap_after_heading: metrics.row_gap_after_heading,
        key_desc_gap: metrics.key_desc_gap,
        badge_font_size: metrics.badge_font_size,
        badge_padding_x: metrics.badge_padding_x,
        badge_gap: metrics.badge_gap,
        badge_height: metrics.badge_height,
        badge_corner_radius: metrics.badge_corner_radius,
        badge_top_gap: metrics.badge_top_gap,
        section_card_padding: metrics.section_card_padding,
        section_card_radius: metrics.section_card_radius,
        row_gap: metrics.row_gap,
        column_gap: metrics.column_gap,
    };
    let grid_colors = GridColors {
        accent: palette.accent,
        heading_icon: palette.heading_icon,
        description: palette.description,
        highlight: palette.highlight,
        section_card_bg: palette.section_card_bg,
        section_card_border: palette.section_card_border,
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

    cursor_y = grid_start_y + grid_view_height + metrics.columns_bottom_spacing;

    // Note
    ctx.select_font_face(
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    ctx.set_font_size(metrics.note_font_size);
    ctx.set_source_rgba(palette.note[0], palette.note[1], palette.note[2], palette.note[3]);
    let note_x = inner_x + (inner_width - note_extents.width()) / 2.0;
    let note_baseline = cursor_y + metrics.note_font_size;
    ctx.move_to(note_x, note_baseline);
    let _ = ctx.show_text(note_text.as_str());
    cursor_y += metrics.note_font_size + note_to_close_gap;

    // Close hint
    ctx.set_source_rgba(palette.subtitle[0], palette.subtitle[1], palette.subtitle[2], 0.7);
    let close_x = inner_x + (inner_width - close_hint_extents.width()) / 2.0;
    let close_baseline = cursor_y + metrics.note_font_size;
    ctx.move_to(close_x, close_baseline);
    let _ = ctx.show_text(close_hint_text);

    scroll_max
}
