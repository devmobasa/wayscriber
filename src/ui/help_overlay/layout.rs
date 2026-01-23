use super::super::primitives::text_extents_for;
use super::keycaps::measure_key_combo;
use super::types::{BadgeTextMetrics, MeasuredSection, Section};

#[derive(Clone)]
pub(crate) struct GridLayout {
    pub(crate) rows: Vec<Vec<MeasuredSection>>,
    pub(crate) row_widths: Vec<f64>,
    pub(crate) row_heights: Vec<f64>,
    pub(crate) grid_width: f64,
    pub(crate) grid_height: f64,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn measure_sections(
    ctx: &cairo::Context,
    sections: Vec<Section>,
    help_font_family: &str,
    body_font_size: f64,
    heading_font_size: f64,
    heading_line_height: f64,
    heading_icon_size: f64,
    heading_icon_gap: f64,
    row_line_height: f64,
    row_gap_after_heading: f64,
    key_desc_gap: f64,
    badge_font_size: f64,
    badge_padding_x: f64,
    badge_gap: f64,
    badge_height: f64,
    badge_top_gap: f64,
    section_card_padding: f64,
) -> Vec<MeasuredSection> {
    let mut measured_sections = Vec::with_capacity(sections.len());
    for section in sections {
        let mut key_max_width: f64 = 0.0;
        for row in &section.rows {
            if row.key.is_empty() {
                continue;
            }
            // Measure with keycap styling padding
            let key_width =
                measure_key_combo(ctx, row.key.as_str(), help_font_family, body_font_size);
            key_max_width = key_max_width.max(key_width);
        }

        let mut section_width: f64 = 0.0;
        let mut section_height: f64 = 0.0;

        let heading_extents = text_extents_for(
            ctx,
            help_font_family,
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
                    help_font_family,
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
            let mut badge_text_metrics = Vec::with_capacity(section.badges.len());

            for (index, badge) in section.badges.iter().enumerate() {
                let badge_extents = text_extents_for(
                    ctx,
                    help_font_family,
                    cairo::FontSlant::Normal,
                    cairo::FontWeight::Bold,
                    badge_font_size,
                    badge.label.as_str(),
                );
                badge_text_metrics.push(BadgeTextMetrics {
                    width: badge_extents.width(),
                    height: badge_extents.height(),
                    y_bearing: badge_extents.y_bearing(),
                });
                let badge_width = badge_extents.width() + badge_padding_x * 2.0;
                if index > 0 {
                    badges_width += badge_gap;
                }
                badges_width += badge_width;
            }

            section_width = section_width.max(badges_width);
            section_height += badge_height;

            measured_sections.push(MeasuredSection {
                section,
                width: section_width + section_card_padding * 2.0,
                height: section_height + section_card_padding * 2.0,
                key_column_width: key_max_width,
                badge_text_metrics,
            });
            continue;
        }
        measured_sections.push(MeasuredSection {
            section,
            width: section_width + section_card_padding * 2.0,
            height: section_height + section_card_padding * 2.0,
            key_column_width: key_max_width,
            badge_text_metrics: Vec::new(),
        });
    }

    measured_sections
}

pub(crate) fn build_grid(
    measured_sections: Vec<MeasuredSection>,
    screen_width: u32,
    max_content_width: f64,
    column_gap: f64,
    row_gap: f64,
) -> GridLayout {
    let max_columns = if screen_width < 1200 {
        1
    } else if screen_width > 1920 {
        3
    } else {
        2
    };
    let max_columns = max_columns.min(measured_sections.len().max(1));

    let mut rows: Vec<Vec<MeasuredSection>> = Vec::new();
    if measured_sections.is_empty() {
        rows.push(Vec::new());
    } else {
        let mut current_row = Vec::new();
        let mut current_width = 0.0;
        for section in measured_sections {
            let next_width = if current_row.is_empty() {
                section.width
            } else {
                current_width + column_gap + section.width
            };
            if (!current_row.is_empty() && next_width > max_content_width)
                || current_row.len() >= max_columns
            {
                rows.push(current_row);
                current_row = Vec::new();
                current_width = 0.0;
            }
            if current_row.is_empty() {
                current_width = section.width;
            } else {
                current_width += column_gap + section.width;
            }
            current_row.push(section);
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

    GridLayout {
        rows,
        row_widths,
        row_heights,
        grid_width,
        grid_height,
    }
}
