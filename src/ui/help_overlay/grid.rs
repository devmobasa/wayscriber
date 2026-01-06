use super::super::primitives::{draw_rounded_rect, text_extents_for};
use super::keycaps::{KeyComboStyle, draw_key_combo, draw_key_combo_highlight, measure_key_combo};
use super::layout::GridLayout;
use super::search::{HighlightStyle, draw_highlight, find_match_range};

pub(crate) struct GridStyle<'a> {
    pub(crate) help_font_family: &'a str,
    pub(crate) body_font_size: f64,
    pub(crate) heading_font_size: f64,
    pub(crate) heading_line_height: f64,
    pub(crate) heading_icon_size: f64,
    pub(crate) heading_icon_gap: f64,
    pub(crate) row_line_height: f64,
    pub(crate) row_gap_after_heading: f64,
    pub(crate) key_desc_gap: f64,
    pub(crate) badge_font_size: f64,
    pub(crate) badge_padding_x: f64,
    pub(crate) badge_gap: f64,
    pub(crate) badge_height: f64,
    pub(crate) badge_corner_radius: f64,
    pub(crate) badge_top_gap: f64,
    pub(crate) section_card_padding: f64,
    pub(crate) section_card_radius: f64,
    pub(crate) row_gap: f64,
    pub(crate) column_gap: f64,
}

pub(crate) struct GridColors {
    pub(crate) accent: [f64; 4],
    pub(crate) heading_icon: [f64; 4],
    pub(crate) description: [f64; 4],
    pub(crate) highlight: [f64; 4],
    pub(crate) section_card_bg: [f64; 4],
    pub(crate) section_card_border: [f64; 4],
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_sections_grid(
    ctx: &cairo::Context,
    grid: &GridLayout,
    grid_start_y: f64,
    inner_x: f64,
    inner_width: f64,
    grid_view_height: f64,
    scroll_offset: f64,
    search_active: bool,
    search_lower: &str,
    style: &GridStyle<'_>,
    colors: &GridColors,
    key_combo_style: &KeyComboStyle<'_>,
) {
    if grid_view_height <= 0.0 {
        return;
    }

    let _ = ctx.save();
    ctx.rectangle(inner_x, grid_start_y, inner_width, grid_view_height);
    ctx.clip();

    let mut row_y = grid_start_y - scroll_offset;
    for (row_index, row) in grid.rows.iter().enumerate() {
        let row_height = *grid.row_heights.get(row_index).unwrap_or(&0.0);
        let row_width = *grid.row_widths.get(row_index).unwrap_or(&inner_width);
        if row.is_empty() {
            row_y += row_height;
            if row_index + 1 < grid.rows.len() {
                row_y += style.row_gap;
            }
            continue;
        }

        let mut section_x = inner_x + (inner_width - row_width) / 2.0;
        for (section_index, measured) in row.iter().enumerate() {
            if section_index > 0 {
                section_x += style.column_gap;
            }

            let section = &measured.section;

            // Draw section card background
            draw_rounded_rect(
                ctx,
                section_x,
                row_y,
                measured.width,
                measured.height,
                style.section_card_radius,
            );
            ctx.set_source_rgba(
                colors.section_card_bg[0],
                colors.section_card_bg[1],
                colors.section_card_bg[2],
                colors.section_card_bg[3],
            );
            let _ = ctx.fill_preserve();
            ctx.set_source_rgba(
                colors.section_card_border[0],
                colors.section_card_border[1],
                colors.section_card_border[2],
                colors.section_card_border[3],
            );
            ctx.set_line_width(1.0);
            let _ = ctx.stroke();

            // Content starts inside card padding
            let content_x = section_x + style.section_card_padding;
            let mut section_y = row_y + style.section_card_padding;
            let desc_x = content_x + measured.key_column_width + style.key_desc_gap;

            ctx.select_font_face(
                style.help_font_family,
                cairo::FontSlant::Normal,
                cairo::FontWeight::Bold,
            );
            ctx.set_font_size(style.heading_font_size);
            ctx.set_source_rgba(
                colors.accent[0],
                colors.accent[1],
                colors.accent[2],
                colors.accent[3],
            );
            let mut heading_text_x = content_x;
            if let Some(icon) = section.icon {
                let icon_y =
                    section_y + (style.heading_line_height - style.heading_icon_size) * 0.5;
                let _ = ctx.save();
                ctx.set_source_rgba(
                    colors.heading_icon[0],
                    colors.heading_icon[1],
                    colors.heading_icon[2],
                    colors.heading_icon[3],
                );
                icon(ctx, content_x, icon_y, style.heading_icon_size);
                let _ = ctx.restore();
                heading_text_x += style.heading_icon_size + style.heading_icon_gap;
            }
            let heading_baseline = section_y + style.heading_font_size;
            ctx.move_to(heading_text_x, heading_baseline);
            let _ = ctx.show_text(section.title);
            section_y += style.heading_line_height;

            if !section.rows.is_empty() {
                section_y += style.row_gap_after_heading;
                for row_data in &section.rows {
                    let baseline = section_y + style.body_font_size;

                    let key_match =
                        search_active && find_match_range(&row_data.key, search_lower).is_some();
                    if key_match && !row_data.key.is_empty() {
                        let key_width = measure_key_combo(
                            ctx,
                            row_data.key.as_str(),
                            style.help_font_family,
                            style.body_font_size,
                        );
                        draw_key_combo_highlight(
                            ctx,
                            content_x,
                            baseline,
                            style.body_font_size,
                            key_width,
                            colors.highlight,
                        );
                    }
                    if search_active
                        && let Some(range) = find_match_range(row_data.action, search_lower)
                    {
                        let highlight_style = HighlightStyle {
                            font_family: style.help_font_family,
                            font_size: style.body_font_size,
                            font_weight: cairo::FontWeight::Normal,
                            color: colors.highlight,
                        };
                        draw_highlight(
                            ctx,
                            desc_x,
                            baseline,
                            row_data.action,
                            range,
                            &highlight_style,
                        );
                    }

                    // Draw key with keycap styling
                    let _ = draw_key_combo(
                        ctx,
                        content_x,
                        baseline,
                        row_data.key.as_str(),
                        key_combo_style,
                    );

                    // Draw action description
                    ctx.select_font_face(
                        style.help_font_family,
                        cairo::FontSlant::Normal,
                        cairo::FontWeight::Normal,
                    );
                    ctx.set_font_size(style.body_font_size);
                    ctx.set_source_rgba(
                        colors.description[0],
                        colors.description[1],
                        colors.description[2],
                        colors.description[3],
                    );
                    ctx.move_to(desc_x, baseline);
                    let _ = ctx.show_text(row_data.action);

                    section_y += style.row_line_height;
                }
            }

            if !section.badges.is_empty() {
                section_y += style.badge_top_gap;
                let mut badge_x = content_x;

                for (badge_index, badge) in section.badges.iter().enumerate() {
                    if badge_index > 0 {
                        badge_x += style.badge_gap;
                    }

                    ctx.new_path();
                    let badge_metrics = measured
                        .badge_text_metrics
                        .get(badge_index)
                        .map(|metrics| (metrics.width, metrics.height, metrics.y_bearing))
                        .unwrap_or_else(|| {
                            let extents = text_extents_for(
                                ctx,
                                style.help_font_family,
                                cairo::FontSlant::Normal,
                                cairo::FontWeight::Bold,
                                style.badge_font_size,
                                badge.label,
                            );
                            (extents.width(), extents.height(), extents.y_bearing())
                        });
                    let badge_width = badge_metrics.0 + style.badge_padding_x * 2.0;

                    draw_rounded_rect(
                        ctx,
                        badge_x,
                        section_y,
                        badge_width,
                        style.badge_height,
                        style.badge_corner_radius,
                    );
                    ctx.set_source_rgba(badge.color[0], badge.color[1], badge.color[2], 0.25);
                    let _ = ctx.fill_preserve();

                    ctx.set_source_rgba(badge.color[0], badge.color[1], badge.color[2], 0.85);
                    ctx.set_line_width(1.0);
                    let _ = ctx.stroke();

                    ctx.select_font_face(
                        style.help_font_family,
                        cairo::FontSlant::Normal,
                        cairo::FontWeight::Bold,
                    );
                    ctx.set_font_size(style.badge_font_size);
                    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.92);
                    let text_x = badge_x + style.badge_padding_x;
                    let text_y =
                        section_y + (style.badge_height - badge_metrics.1) / 2.0 - badge_metrics.2;
                    ctx.move_to(text_x, text_y);
                    let _ = ctx.show_text(badge.label);

                    badge_x += badge_width;
                }
            }

            section_x += measured.width;
        }

        row_y += row_height;
        if row_index + 1 < grid.rows.len() {
            row_y += style.row_gap;
        }
    }

    let _ = ctx.restore();
}
