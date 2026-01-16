use crate::config::HelpOverlayStyle;

#[derive(Debug, Clone, Copy)]
pub(super) struct RenderMetrics {
    pub(super) body_font_size: f64,
    pub(super) heading_font_size: f64,
    pub(super) title_font_size: f64,
    pub(super) subtitle_font_size: f64,
    pub(super) row_line_height: f64,
    pub(super) heading_line_height: f64,
    pub(super) heading_icon_size: f64,
    pub(super) heading_icon_gap: f64,
    pub(super) row_gap_after_heading: f64,
    pub(super) key_desc_gap: f64,
    pub(super) row_gap: f64,
    pub(super) column_gap: f64,
    pub(super) section_card_padding: f64,
    pub(super) section_card_radius: f64,
    pub(super) badge_font_size: f64,
    pub(super) badge_padding_x: f64,
    pub(super) badge_gap: f64,
    pub(super) badge_height: f64,
    pub(super) badge_corner_radius: f64,
    pub(super) badge_top_gap: f64,
    pub(super) accent_line_height: f64,
    pub(super) accent_line_bottom_spacing: f64,
    pub(super) title_bottom_spacing: f64,
    pub(super) subtitle_bottom_spacing: f64,
    pub(super) nav_line_gap: f64,
    pub(super) nav_bottom_spacing: f64,
    pub(super) extra_line_gap: f64,
    pub(super) extra_line_bottom_spacing: f64,
    pub(super) columns_bottom_spacing: f64,
    pub(super) max_box_width: f64,
    pub(super) max_box_height: f64,
    pub(super) note_font_size: f64,
    pub(super) nav_font_size: f64,
    pub(super) note_to_close_gap: f64,
    pub(super) padding: f64,
}

impl RenderMetrics {
    pub(super) fn from_style(
        style: &HelpOverlayStyle,
        screen_width: u32,
        screen_height: u32,
    ) -> Self {
        let scale = 0.8;
        let body_font_size = style.font_size * scale;
        let heading_font_size = body_font_size + 6.0 * scale;
        let title_font_size = heading_font_size + 6.0 * scale;
        let subtitle_font_size = body_font_size;
        let row_extra_gap = 4.0 * scale;
        let line_height = style.line_height * scale;
        let row_line_height = line_height.max(body_font_size + 8.0 * scale) + row_extra_gap;
        let heading_line_height = heading_font_size + 10.0 * scale;
        let heading_icon_size = heading_font_size * 0.9;
        let heading_icon_gap = 10.0 * scale;
        let row_gap_after_heading = 10.0 * scale;
        let key_desc_gap = 24.0 * scale;
        let row_gap = 36.0 * scale;
        let column_gap = 56.0 * scale;
        let section_card_padding = 14.0 * scale;
        let section_card_radius = 10.0 * scale;
        let min_font_size = 12.0 * scale;
        let badge_font_size = (body_font_size - 2.0 * scale).max(min_font_size);
        let badge_padding_x = 12.0 * scale;
        let badge_padding_y = 6.0 * scale;
        let badge_gap = 12.0 * scale;
        let badge_height = badge_font_size + badge_padding_y * 2.0;
        let badge_corner_radius = 10.0 * scale;
        let badge_top_gap = 10.0 * scale;
        let accent_line_height = 2.0 * scale;
        let accent_line_bottom_spacing = 16.0 * scale;
        let title_bottom_spacing = 8.0 * scale;
        let subtitle_bottom_spacing = 28.0 * scale;
        let nav_line_gap = 6.0 * scale;
        let nav_bottom_spacing = 18.0 * scale;
        let extra_line_gap = 30.0 * scale;
        let extra_line_bottom_spacing = 18.0 * scale;
        let columns_bottom_spacing = 28.0 * scale;
        let max_box_width = screen_width as f64 * 0.92;
        let max_box_height = screen_height as f64 * 0.92;
        let note_font_size = (body_font_size - 2.0 * scale).max(min_font_size);
        let nav_font_size = (body_font_size - 1.0 * scale).max(min_font_size);
        let note_to_close_gap = 12.0 * scale;
        let padding = style.padding * scale;

        Self {
            body_font_size,
            heading_font_size,
            title_font_size,
            subtitle_font_size,
            row_line_height,
            heading_line_height,
            heading_icon_size,
            heading_icon_gap,
            row_gap_after_heading,
            key_desc_gap,
            row_gap,
            column_gap,
            section_card_padding,
            section_card_radius,
            badge_font_size,
            badge_padding_x,
            badge_gap,
            badge_height,
            badge_corner_radius,
            badge_top_gap,
            accent_line_height,
            accent_line_bottom_spacing,
            title_bottom_spacing,
            subtitle_bottom_spacing,
            nav_line_gap,
            nav_bottom_spacing,
            extra_line_gap,
            extra_line_bottom_spacing,
            columns_bottom_spacing,
            max_box_width,
            max_box_height,
            note_font_size,
            nav_font_size,
            note_to_close_gap,
            padding,
        }
    }
}
