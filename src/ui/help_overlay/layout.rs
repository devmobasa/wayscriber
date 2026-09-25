use super::super::primitives::text_extents_for_with_engine;
use super::grid::GridStyle;
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

/// Measure each section card. Rows read "label … keys": the label column is
/// as wide as the widest label, and the key chips start one gap past it.
pub(crate) fn measure_sections(
    engine: &crate::ui_text::UiTextEngine,
    ctx: &cairo::Context,
    sections: Vec<Section>,
    style: &GridStyle<'_>,
) -> Vec<MeasuredSection> {
    let text_width = |weight: cairo::FontWeight, size: f64, text: &str| {
        text_extents_for_with_engine(
            engine,
            ctx,
            style.help_font_family,
            cairo::FontSlant::Normal,
            weight,
            size,
            text,
        )
        .width()
    };

    let mut measured_sections = Vec::with_capacity(sections.len());
    for section in sections {
        let mut label_column_width: f64 = 0.0;
        let mut key_column_width: f64 = 0.0;
        for row in &section.rows {
            label_column_width = label_column_width.max(text_width(
                cairo::FontWeight::Normal,
                style.body_font_size,
                row.action,
            ));
            if !row.key.is_empty() {
                key_column_width = key_column_width.max(measure_key_combo(
                    engine,
                    ctx,
                    row.key.as_str(),
                    style.help_font_family,
                    style.key_font_size,
                ));
            }
        }

        let mut heading_width = text_width(
            cairo::FontWeight::Bold,
            style.heading_font_size,
            section.title,
        );
        if section.icon.is_some() {
            heading_width += style.heading_icon_size + style.heading_icon_gap;
        }
        let mut section_width = heading_width;
        let mut section_height = style.heading_line_height;

        if !section.rows.is_empty() {
            let key_span = if key_column_width > 0.0 {
                style.key_desc_gap + key_column_width
            } else {
                0.0
            };
            section_width = section_width.max(label_column_width + key_span);
            section_height +=
                style.row_gap_after_heading + style.row_line_height * section.rows.len() as f64;
        }

        let mut badge_text_metrics = Vec::with_capacity(section.badges.len());
        if !section.badges.is_empty() {
            let mut badges_width = 0.0;
            for (index, badge) in section.badges.iter().enumerate() {
                let badge_extents = text_extents_for_with_engine(
                    engine,
                    ctx,
                    style.help_font_family,
                    cairo::FontSlant::Normal,
                    cairo::FontWeight::Bold,
                    style.badge_font_size,
                    badge.label.as_str(),
                );
                badge_text_metrics.push(BadgeTextMetrics {
                    width: badge_extents.width(),
                    height: badge_extents.height(),
                    y_bearing: badge_extents.y_bearing(),
                });
                if index > 0 {
                    badges_width += style.badge_gap;
                }
                badges_width += badge_extents.width() + style.badge_padding_x * 2.0;
            }

            section_width = section_width.max(badges_width);
            section_height += style.badge_top_gap + style.badge_height;
        }

        measured_sections.push(MeasuredSection {
            section,
            width: section_width + style.section_card_padding * 2.0,
            height: section_height + style.section_card_padding * 2.0,
            label_column_width,
            badge_text_metrics,
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
    // M6: at most two columns (the plan caps the reference card at two so it
    // never sprawls into a wall-to-wall sheet on ultrawide displays).
    let max_columns = if screen_width < 1200 { 1 } else { 2 };
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
