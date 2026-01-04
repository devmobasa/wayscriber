use super::super::super::primitives::text_extents_for;
use super::super::fonts::resolve_help_font_family;
use super::super::layout::{GridLayout, build_grid, measure_sections};
use super::super::nav::{NavState, build_nav_state};
use super::super::sections::{build_section_sets, filter_sections_for_search};
use super::BULLET;
use super::metrics::RenderMetrics;
use super::palette::RenderPalette;

pub(super) struct OverlayLayout {
    pub(super) search_active: bool,
    pub(super) search_lower: String,
    pub(super) help_font_family: String,
    pub(super) metrics: RenderMetrics,
    pub(super) palette: RenderPalette,
    pub(super) nav_state: NavState,
    pub(super) grid: GridLayout,
    pub(super) note_text: String,
    pub(super) note_width: f64,
    pub(super) close_hint_width: f64,
    pub(super) box_width: f64,
    pub(super) box_height: f64,
    pub(super) box_x: f64,
    pub(super) box_y: f64,
    pub(super) grid_view_height: f64,
    pub(super) scroll_max: f64,
    pub(super) scroll_offset: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_overlay_layout(
    ctx: &cairo::Context,
    style: &crate::config::HelpOverlayStyle,
    screen_width: u32,
    screen_height: u32,
    frozen_enabled: bool,
    page_index: usize,
    page_prev_label: &str,
    page_next_label: &str,
    search_query: &str,
    context_filter: bool,
    board_enabled: bool,
    capture_enabled: bool,
    scroll_offset: f64,
    title_text: &str,
    version_line: &str,
    note_text_base: &str,
    close_hint_text: &str,
) -> OverlayLayout {
    let search_query = search_query.trim();
    let search_active = !search_query.is_empty();
    let search_lower = search_query.to_ascii_lowercase();
    let help_font_family = resolve_help_font_family(&style.font_family);

    let section_sets = build_section_sets(
        frozen_enabled,
        context_filter,
        board_enabled,
        capture_enabled,
        page_prev_label,
        page_next_label,
    );
    let page_count = if section_sets.page2.is_empty() { 1 } else { 2 };
    let page_index = page_index.min(page_count - 1);
    let nav_title = "Controls";

    let sections = if search_active {
        filter_sections_for_search(section_sets.all, &search_lower)
    } else if page_index == 0 {
        section_sets.page1
    } else {
        section_sets.page2
    };

    let metrics = RenderMetrics::from_style(style, screen_width, screen_height);
    let palette = RenderPalette::from_style(style);

    let max_search_width = (screen_width as f64 * 0.9 - metrics.padding * 2.0).max(0.0);
    let nav_state = build_nav_state(
        ctx,
        help_font_family.as_str(),
        nav_title,
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

    let max_content_width = (metrics.max_box_width - metrics.padding * 2.0).max(0.0);
    let grid = build_grid(
        measured_sections,
        screen_width,
        max_content_width,
        metrics.column_gap,
        metrics.row_gap,
    );

    let title_width = text_extents_for(
        ctx,
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        metrics.title_font_size,
        title_text,
    )
    .width();
    let subtitle_width = text_extents_for(
        ctx,
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        metrics.subtitle_font_size,
        version_line,
    )
    .width();
    let close_hint_width = text_extents_for(
        ctx,
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        metrics.note_font_size,
        close_hint_text,
    )
    .width();

    let note_to_close_gap = metrics.note_to_close_gap;
    let header_height = metrics.accent_line_height
        + metrics.accent_line_bottom_spacing
        + metrics.title_font_size
        + metrics.title_bottom_spacing
        + metrics.subtitle_font_size
        + metrics.subtitle_bottom_spacing
        + nav_state.nav_block_height;
    let footer_height = metrics.columns_bottom_spacing
        + metrics.note_font_size
        + note_to_close_gap
        + metrics.note_font_size;
    let content_height = header_height + grid.grid_height + footer_height;
    let max_inner_height = (metrics.max_box_height - metrics.padding * 2.0).max(0.0);
    let inner_height = content_height.min(max_inner_height);
    let grid_view_height = (inner_height - header_height - footer_height).max(0.0);
    let scroll_max = (grid.grid_height - grid_view_height).max(0.0);
    let scroll_offset = scroll_offset.clamp(0.0, scroll_max);
    let page_label = format!("Page {}/{}", page_index + 1, page_count.max(1));
    let note_text = if scroll_max > 0.0 {
        format!(
            "{}  {}  {}  {}  Scroll: Mouse wheel",
            note_text_base, BULLET, page_label, BULLET
        )
    } else {
        format!("{}  {}  {}", note_text_base, BULLET, page_label)
    };
    let note_width = text_extents_for(
        ctx,
        help_font_family.as_str(),
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        metrics.note_font_size,
        note_text.as_str(),
    )
    .width();

    let mut content_width = grid
        .grid_width
        .max(title_width)
        .max(subtitle_width)
        .max(nav_state.nav_primary_width)
        .max(nav_state.nav_secondary_width)
        .max(note_width)
        .max(close_hint_width);
    // Don't let search text expand the overlay - it will be clamped/elided
    if grid.rows.is_empty() {
        content_width = content_width.max(title_width).max(subtitle_width);
    }
    // Ensure minimum width for search box
    content_width = content_width.max(300.0);
    let box_width = content_width + metrics.padding * 2.0;
    let box_height = inner_height + metrics.padding * 2.0;

    let box_x = (screen_width as f64 - box_width) / 2.0;
    let box_y = (screen_height as f64 - box_height) / 2.0;

    OverlayLayout {
        search_active,
        search_lower,
        help_font_family,
        metrics,
        palette,
        nav_state,
        grid,
        note_text,
        note_width,
        close_hint_width,
        box_width,
        box_height,
        box_x,
        box_y,
        grid_view_height,
        scroll_max,
        scroll_offset,
    }
}
