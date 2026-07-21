use super::grid::{GridColors, GridStyle, draw_sections_grid};
use super::keycaps::KeyComboStyle;
use super::nav::{NavDrawStyle, draw_nav};
use super::sections::HelpOverlayBindings;

mod cache;
mod frame;
mod header;
mod hit;
mod metrics;
mod palette;
mod state;

use super::super::primitives::{draw_rounded_rect, text_extents_for};
use super::types::HelpRowHit;
use crate::config::{Action, action_label};
use crate::label_format::NOT_BOUND_LABEL;
use crate::ui_text::{UiTextStyle, draw_text_baseline};
use cache::get_or_build_overlay_layout;
use frame::draw_overlay_frame;
use header::{HeaderContent, HeaderHint, draw_hints, draw_version_pill};

pub use cache::invalidate_help_overlay_cache;
#[cfg(test)]
pub use hit::install_help_hit_map_for_test;
pub use hit::{HelpOverlayRegion, clear_help_overlay_hit_map, help_overlay_region_at};

const BULLET: &str = "\u{2022}";

/// Horizontal padding inside the "Replay tour" footer pill, between its border
/// and the icon/label content.
const REPLAY_FOOTER_PAD_X: f64 = 12.0;
/// Gap between the refresh icon and the "Replay tour" label inside the pill.
const REPLAY_FOOTER_ICON_GAP: f64 = 7.0;

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
    let palette_binding = bindings
        .labels_for(Action::ToggleCommandPalette)
        .and_then(|labels| labels.first())
        .map(|label| label.as_str())
        .unwrap_or(NOT_BOUND_LABEL);
    let config_binding = bindings
        .labels_for(Action::OpenConfigurator)
        .and_then(|labels| labels.first())
        .map(|label| label.as_str())
        .unwrap_or(NOT_BOUND_LABEL);
    let version_text = format!("v{}", crate::build_info::version());
    let header_hints = if quick_mode {
        vec![HeaderHint {
            keys: palette_binding,
            label: "Command Palette (search all)",
        }]
    } else {
        vec![
            HeaderHint {
                keys: palette_binding,
                label: "Command Palette",
            },
            HeaderHint {
                keys: config_binding,
                label: action_label(Action::OpenConfigurator),
            },
        ]
    };
    let header = HeaderContent {
        version: &version_text,
        intro: if quick_mode {
            Some("Essential shortcuts")
        } else {
            None
        },
        hints: &header_hints,
    };
    let help_binding = bindings
        .labels_for(Action::ToggleHelp)
        .and_then(|labels| labels.first())
        .map(|label| label.as_str())
        .unwrap_or("F1");
    let quick_help_binding = bindings
        .labels_for(Action::ToggleQuickHelp)
        .and_then(|labels| labels.first())
        .map(|label| label.as_str())
        .unwrap_or("Shift+F1");
    let note_text_base_owned;
    let note_text_base: &str = if quick_mode {
        note_text_base_owned = format!("{} for full help", help_binding);
        &note_text_base_owned
    } else {
        "Note: Each board has independent pages"
    };
    let close_hint_owned = if quick_mode {
        format!("{} / Esc to close", quick_help_binding)
    } else {
        format!("{} / Esc to close", help_binding)
    };
    let close_hint_text: &str = &close_hint_owned;

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
        &header,
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

    // Accent line — solid on the left, fading out to the right so it reads as a
    // deliberate flourish rather than a hard rule.
    let accent_gradient = cairo::LinearGradient::new(inner_x, 0.0, inner_x + inner_width, 0.0);
    accent_gradient.add_color_stop_rgba(
        0.0,
        palette.accent[0],
        palette.accent[1],
        palette.accent[2],
        palette.accent[3],
    );
    accent_gradient.add_color_stop_rgba(
        0.55,
        palette.accent[0],
        palette.accent[1],
        palette.accent[2],
        palette.accent[3] * 0.55,
    );
    accent_gradient.add_color_stop_rgba(
        1.0,
        palette.accent[0],
        palette.accent[1],
        palette.accent[2],
        0.0,
    );
    let _ = ctx.set_source(&accent_gradient);
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

    // Version pill, right-aligned on the title row.
    draw_version_pill(
        ctx,
        inner_x + inner_width,
        title_baseline,
        metrics.title_font_size,
        help_font_family,
        metrics.subtitle_font_size,
        header.version,
        palette.accent,
        palette.accent_muted,
    );
    cursor_y += metrics.title_font_size + metrics.title_bottom_spacing;

    // Subtitle hint line — keycap chips matching the grid rows below.
    let muted = [
        palette.subtitle[0],
        palette.subtitle[1],
        palette.subtitle[2],
        palette.subtitle[3] * 0.7,
    ];
    let subtitle_baseline = cursor_y + metrics.subtitle_font_size + header::KEYCAP_PAD_Y;
    draw_hints(
        ctx,
        inner_x,
        subtitle_baseline,
        help_font_family,
        metrics.subtitle_font_size,
        &header,
        &key_combo_style,
        palette.subtitle,
        muted,
    );
    cursor_y += metrics.subtitle_row_height + metrics.subtitle_bottom_spacing;

    let nav_draw_style = NavDrawStyle {
        font_family: help_font_family,
        subtitle_color: palette.subtitle,
        search_color: palette.search,
        nav_line_gap: metrics.nav_line_gap,
        nav_bottom_spacing: metrics.nav_bottom_spacing,
        extra_line_gap: metrics.extra_line_gap,
        extra_line_bottom_spacing: metrics.extra_line_bottom_spacing,
    };
    let nav_render = draw_nav(
        ctx,
        inner_x,
        cursor_y,
        inner_width,
        &layout.nav_state,
        &nav_draw_style,
    );
    cursor_y = nav_render.next_y;
    let search_rect = nav_render.search_rect;

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

    // Clickable rows collect their screen rects here as the grid draws, so the
    // pointer hit map tests the real layout rather than an approximation.
    let mut row_hits: Vec<HelpRowHit> = Vec::new();
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
        &mut row_hits,
    );

    cursor_y = grid_start_y + layout.grid_view_height + metrics.columns_bottom_spacing;

    // "Replay tour" footer entry: a centred, clickable pill that runs the
    // ReplayTour action (resolved through the registry, never a hardcoded
    // label). Registered in the hit map as a clickable row.
    let replay_hit = draw_replay_footer(
        ctx,
        inner_x,
        inner_width,
        cursor_y,
        metrics.footer_action_height,
        metrics.note_font_size,
        help_font_family,
        palette.accent,
        palette.accent_muted,
    );
    row_hits.push(replay_hit);
    cursor_y += metrics.footer_action_height + metrics.footer_action_gap;

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

    hit::store_help_hit_map(
        (
            layout.box_x,
            layout.box_y,
            layout.box_width,
            layout.box_height,
        ),
        Some(search_rect),
        &row_hits,
    );

    layout.scroll_max
}

/// Draw the "Replay tour" footer pill centred on the overlay and return its
/// clickable rect (tagged with [`Action::ReplayTour`]).
#[allow(clippy::too_many_arguments)]
fn draw_replay_footer(
    ctx: &cairo::Context,
    inner_x: f64,
    inner_width: f64,
    top_y: f64,
    pill_height: f64,
    font_size: f64,
    font_family: &str,
    accent: [f64; 4],
    accent_muted: [f64; 4],
) -> HelpRowHit {
    let label = action_label(Action::ReplayTour);
    let icon_size = font_size;
    let icon_gap = REPLAY_FOOTER_ICON_GAP;
    let pad_x = REPLAY_FOOTER_PAD_X;

    let label_width = text_extents_for(
        ctx,
        font_family,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        font_size,
        label,
    )
    .width();
    let content_width = icon_size + icon_gap + label_width;
    let pill_width = content_width + pad_x * 2.0;
    let pill_x = inner_x + (inner_width - pill_width) / 2.0;

    draw_rounded_rect(
        ctx,
        pill_x,
        top_y,
        pill_width,
        pill_height,
        header::PILL_RADIUS,
    );
    ctx.set_source_rgba(accent[0], accent[1], accent[2], 0.14);
    let _ = ctx.fill();
    draw_rounded_rect(
        ctx,
        pill_x,
        top_y,
        pill_width,
        pill_height,
        header::PILL_RADIUS,
    );
    ctx.set_source_rgba(accent[0], accent[1], accent[2], 0.38);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    let content_x = pill_x + pad_x;
    let icon_y = top_y + (pill_height - icon_size) / 2.0;
    let _ = ctx.save();
    ctx.set_source_rgba(
        accent_muted[0],
        accent_muted[1],
        accent_muted[2],
        accent_muted[3],
    );
    crate::toolbar_icons::draw_icon_refresh(ctx, content_x, icon_y, icon_size);
    let _ = ctx.restore();

    let label_style = UiTextStyle {
        family: font_family,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: font_size,
    };
    let label_baseline = top_y + pill_height / 2.0 + font_size * 0.35;
    ctx.set_source_rgba(
        accent_muted[0],
        accent_muted[1],
        accent_muted[2],
        accent_muted[3],
    );
    draw_text_baseline(
        ctx,
        label_style,
        label,
        content_x + icon_size + icon_gap,
        label_baseline,
        None,
    );

    HelpRowHit {
        x: pill_x,
        y: top_y,
        w: pill_width,
        h: pill_height,
        action: Action::ReplayTour,
    }
}
