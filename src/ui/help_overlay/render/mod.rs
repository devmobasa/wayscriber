use super::grid::{GridColors, GridStyle, draw_sections_grid};
use super::keycaps::KeyComboStyle;
use super::nav::{NavDrawStyle, draw_nav};
use super::sections::HelpOverlayBindings;

mod cache;
mod frame;
mod metrics;
mod palette;
mod state;

use crate::config::{Action, action_label};
use crate::label_format::NOT_BOUND_LABEL;
use crate::ui_text::{UiTextStyle, draw_text_baseline};
use cache::get_or_build_overlay_layout;
use frame::draw_overlay_frame;

pub use cache::invalidate_help_overlay_cache;

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
    page_index: usize,
    bindings: &HelpOverlayBindings,
    search_query: &str,
    context_filter: bool,
    board_enabled: bool,
    capture_enabled: bool,
    scroll_offset: f64,
    quick_mode: bool,
) -> f64 {
    let title_text = if quick_mode {
        "Quick Reference"
    } else {
        "Wayscriber Controls"
    };
    let commit_hash = option_env!("WAYSCRIBER_GIT_HASH").unwrap_or("unknown");
    let config_binding = bindings
        .labels_for(Action::OpenConfigurator)
        .and_then(|labels| labels.first())
        .map(|label| label.as_str())
        .unwrap_or(NOT_BOUND_LABEL);
    let version_line = if quick_mode {
        "Essential shortcuts at a glance".to_string()
    } else {
        format!(
            "Wayscriber {} ({})  {}  {} {} {}",
            env!("CARGO_PKG_VERSION"),
            commit_hash,
            BULLET,
            config_binding,
            ARROW,
            action_label(Action::OpenConfigurator)
        )
    };
    let note_text_base = if quick_mode {
        "F1 for full help"
    } else {
        "Note: Each board has independent pages"
    };
    let close_hint_text = if quick_mode {
        "Shift+F1 / Esc to close"
    } else {
        "F1 / Esc to close"
    };

    let layout = get_or_build_overlay_layout(
        ctx,
        style,
        screen_width,
        screen_height,
        frozen_enabled,
        page_index,
        bindings,
        search_query,
        context_filter,
        board_enabled,
        capture_enabled,
        scroll_offset,
        title_text,
        &version_line,
        note_text_base,
        close_hint_text,
        quick_mode,
    );
    let help_font_family = layout.help_font_family.as_str();
    let metrics = layout.metrics;
    let palette = layout.palette;
    let key_combo_style = KeyComboStyle {
        font_family: help_font_family,
        font_size: metrics.body_font_size,
        text_color: palette.accent_muted,
        separator_color: palette.subtitle,
    };

    draw_overlay_frame(
        ctx,
        style,
        &palette,
        screen_width,
        screen_height,
        layout.box_x,
        layout.box_y,
        layout.box_width,
        layout.box_height,
    );

    let padding = metrics.padding;
    let inner_x = layout.box_x + padding;
    let mut cursor_y = layout.box_y + padding;
    let inner_width = layout.box_width - padding * 2.0;

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
    let title_style = UiTextStyle {
        family: help_font_family,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: metrics.title_font_size,
    };
    ctx.set_source_rgba(
        palette.body_text[0],
        palette.body_text[1],
        palette.body_text[2],
        palette.body_text[3],
    );
    let title_baseline = cursor_y + metrics.title_font_size;
    draw_text_baseline(ctx, title_style, title_text, inner_x, title_baseline, None);
    cursor_y += metrics.title_font_size + metrics.title_bottom_spacing;

    // Subtitle / version line
    let subtitle_style = UiTextStyle {
        family: help_font_family,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: metrics.subtitle_font_size,
    };
    ctx.set_source_rgba(
        palette.subtitle[0],
        palette.subtitle[1],
        palette.subtitle[2],
        palette.subtitle[3],
    );
    let subtitle_baseline = cursor_y + metrics.subtitle_font_size;
    draw_text_baseline(
        ctx,
        subtitle_style,
        &version_line,
        inner_x,
        subtitle_baseline,
        None,
    );
    cursor_y += metrics.subtitle_font_size + metrics.subtitle_bottom_spacing;

    let nav_draw_style = NavDrawStyle {
        font_family: help_font_family,
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
        &layout.nav_state,
        &nav_draw_style,
    );

    let grid_start_y = cursor_y;

    let grid_style = GridStyle {
        help_font_family,
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
        &layout.grid,
        grid_start_y,
        inner_x,
        inner_width,
        layout.grid_view_height,
        layout.scroll_offset,
        layout.search_active,
        &layout.search_lower,
        &grid_style,
        &grid_colors,
        &key_combo_style,
    );

    cursor_y = grid_start_y + layout.grid_view_height + metrics.columns_bottom_spacing;

    // Note
    let note_style = UiTextStyle {
        family: help_font_family,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: metrics.note_font_size,
    };
    ctx.set_source_rgba(
        palette.note[0],
        palette.note[1],
        palette.note[2],
        palette.note[3],
    );
    let note_x = inner_x + (inner_width - layout.note_width) / 2.0;
    let note_baseline = cursor_y + metrics.note_font_size;
    draw_text_baseline(
        ctx,
        note_style,
        layout.note_text.as_str(),
        note_x,
        note_baseline,
        None,
    );
    cursor_y += metrics.note_font_size + metrics.note_to_close_gap;

    // Close hint
    let close_style = UiTextStyle {
        family: help_font_family,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: metrics.note_font_size,
    };
    ctx.set_source_rgba(
        palette.subtitle[0],
        palette.subtitle[1],
        palette.subtitle[2],
        0.7,
    );
    let close_x = inner_x + (inner_width - layout.close_hint_width) / 2.0;
    let close_baseline = cursor_y + metrics.note_font_size;
    draw_text_baseline(
        ctx,
        close_style,
        close_hint_text,
        close_x,
        close_baseline,
        None,
    );

    layout.scroll_max
}
